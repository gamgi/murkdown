use std::{
    collections::hash_map::Entry,
    fmt::Write,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use murkdown::types::{LibErrorPathCtx, LocationMap, URI};
use murkdown::{compiler, parser};
use murkdown::{preprocessor, types::AstMap};
use tokio::fs;
use walkdir::{DirEntry, WalkDir};

use super::{
    graph::OpGraph,
    op::{OpId, Operation},
    types::{AppError, AppErrorPathCtx, ArtifactMap, Output},
    utils::{is_file, is_visible},
};
use crate::cli::command::GraphType;
use crate::cli::{artifact::Artifact, command::Command, utils::into_uri_path_tuple};

/// Index the contents of provided paths
pub async fn index(
    paths: Vec<PathBuf>,
    locations: Arc<Mutex<LocationMap>>,
) -> Result<bool, AppError> {
    let mut locations = locations.lock().expect("poisoned lock");
    for path in paths {
        let walker = WalkDir::new(path)
            .into_iter()
            .filter_entry(is_visible)
            .filter_map(Result::ok)
            .filter(is_file)
            .map(into_uri_path_tuple);
        for (id, path) in walker {
            locations.insert(id, path);
        }
    }

    Ok(false)
}

/// Gather entry points and schedule dependencies
pub async fn gather(op: Operation, operations: Arc<Mutex<OpGraph>>) -> Result<bool, AppError> {
    let Operation::Gather { cmd, paths, splits: _ } = &op else {
        panic!()
    };
    let mut graph = operations.lock().expect("poisoned lock");
    let is_source_or_explicitly_included = |e: &DirEntry| {
        let path = e.path();
        let path_is_md = path.extension().map(|s| s == "md").unwrap_or(false);
        path_is_md || paths.iter().any(|p| *p == path)
    };

    // schedule dependent tasks
    for path in paths {
        let walker = WalkDir::new(path)
            .into_iter()
            .filter_entry(is_visible)
            .filter_map(Result::ok)
            .filter(is_file)
            .filter(is_source_or_explicitly_included)
            .map(into_uri_path_tuple);

        for (id, path) in walker {
            let id: Arc<str> = Arc::from(id.as_ref());
            match cmd {
                Command::Load { .. } => {
                    if graph.get(&OpId::load(id.clone())).is_none() {
                        graph.insert_node_chain([Operation::Load { id, path }, Operation::Finish]);
                    } else {
                        // reload
                    }
                }
                Command::Build { .. } | Command::Graph { .. } => graph.insert_node_chain([
                    op.clone(),
                    Operation::Load { id: id.clone(), path },
                    Operation::Parse { id: id.clone() },
                    Operation::Preprocess { id: id.clone() },
                    Operation::Compile { id: id.clone() },
                    Operation::Write { id: id.clone() },
                    Operation::Finish,
                ]),
                _ => panic!("gather on bad command"),
            }
        }
    }

    Ok(false)
}

/// Load files
pub async fn load(op: Operation, artifacts: Arc<Mutex<ArtifactMap>>) -> Result<bool, AppError> {
    let Operation::Load { path, .. } = &op else {
        unreachable!()
    };
    let artifact = match tokio::fs::read_to_string(path).await {
        Ok(contents) => Artifact::String(contents),
        Err(_) => Artifact::Binary(std::fs::read(path).with_ctx(path)?),
    };
    let uri = op.uri();

    // add artifact
    let mut artifacts = artifacts.lock().expect("poisoned lock");
    artifacts.insert(uri.clone(), artifact);

    Ok(false)
}

/// Parse files to AST
pub async fn parse(
    op: Operation,
    dep: URI,
    artifacts: Arc<Mutex<ArtifactMap>>,
) -> Result<bool, AppError> {
    let Operation::Parse { id } = &op else {
        unreachable!()
    };
    let mut artifacts = artifacts.lock().expect("poisoned lock");
    let content = artifacts.get(&dep).expect("no parse dependency");

    match content {
        Artifact::String(content) => {
            let ast = parser::parse(content).with_path(id)?;
            artifacts.insert(op.uri(), Artifact::Ast(ast));
        }
        _ => todo!(),
    }

    Ok(false)
}

/// Preprocess AST and schedule dependencies
pub async fn preprocess(
    op: Operation,
    dep: URI,
    asts: Arc<Mutex<AstMap>>,
    operations: Arc<Mutex<OpGraph>>,
    artifacts: Arc<Mutex<ArtifactMap>>,
    locations: Arc<Mutex<LocationMap>>,
) -> Result<bool, AppError> {
    let Operation::Preprocess { ref id } = op else {
        unreachable!()
    };
    let mut artifacts = artifacts.lock().expect("poisoned lock");
    let ast = artifacts.get(&dep).expect("no preprocess dependency");
    let mut graph = operations.lock().expect("poisoned lock");

    // NOTE: clone to keep source AST intact
    match ast.clone() {
        Artifact::Ast(mut node) => {
            let mut asts = asts.lock().expect("poisoned lock");
            let uri = op.uri();

            let locs = locations.lock().expect("poisoned lock");
            let deps = preprocessor::preprocess(&mut node, &mut asts, &locs, id);

            // upsert preprocessed node to ast
            let arc = match asts.entry(uri.to_string()) {
                Entry::Occupied(r) => {
                    let mut mutex = r.get().lock().expect("poisoned lock");
                    *mutex = node;
                    &r.get().clone()
                }
                Entry::Vacant(r) => &*r.insert(Arc::new(Mutex::new(node))),
            };

            // add artifact
            let weak = Arc::downgrade(arc);
            artifacts.insert(uri, Artifact::AstPointer(weak));

            // schedule dependent tasks
            for uri in deps {
                let (schema, uri_path) = uri.split_once(':').expect("uri to have schema");

                if graph.get_uri(&uri).is_some() {
                    continue;
                }

                let id = Arc::from(uri_path);
                let path = locs
                    .get(uri_path)
                    .ok_or(AppError::file_not_found(uri_path))?
                    .clone();

                match schema {
                    "file" => {
                        graph.insert_node_chain([Operation::Load { id, path }, op.clone()]);
                    }
                    "ast" => {
                        graph.insert_node_chain([
                            Operation::Load { id: id.clone(), path },
                            Operation::Parse { id },
                            op.clone(),
                        ]);
                    }
                    "parse" => {
                        graph.insert_node_chain([
                            Operation::Load { id: id.clone(), path },
                            Operation::Parse { id: id.clone() },
                            Operation::Preprocess { id },
                            op.clone(),
                        ]);
                    }
                    "copy" => {
                        graph.insert_node_chain([Operation::Copy { id, path }, Operation::Finish]);
                    }
                    _ => return Err(AppError::unknown_schema(schema)),
                }
            }
        }
        _ => panic!("preprocessing unknown artifact"),
    }

    Ok(false)
}

/// Compile AST to string
pub async fn compile(
    op: Operation,
    dep: URI,
    artifacts: Arc<Mutex<ArtifactMap>>,
) -> Result<bool, AppError> {
    let Operation::Compile { .. } = op else {
        unreachable!()
    };
    let mut artifacts = artifacts.lock().expect("poisoned lock");
    let ast = artifacts.get(&dep).expect("no compile dependency");

    let result = match ast.clone() {
        Artifact::Ast(mut node) => compiler::compile(&mut node).unwrap(),
        Artifact::AstPointer(pointer) => {
            let mutex = pointer.upgrade().unwrap();
            let mut node = mutex.lock().unwrap();
            compiler::compile(&mut node).unwrap()
        }
        _ => panic!("compiling unknown artifact"),
    };
    artifacts.insert(op.uri(), Artifact::String(result));

    Ok(false)
}

/// Write artifact to target
pub async fn write(
    op: Operation,
    dep: URI,
    artifacts: Arc<Mutex<ArtifactMap>>,
    output: Output,
) -> Result<bool, AppError> {
    let Operation::Write { id } = op else {
        unreachable!()
    };

    let content = {
        let artifacts = artifacts.lock().expect("poisoned lock");
        let result = artifacts.get(&dep).expect("no write dependency");
        match result {
            Artifact::String(content) => content.clone(),
            _ => panic!("writing unknown artifact"),
        }
    };

    match output {
        Output::Stdout => println!("{content}"),
        Output::Path(root) => {
            let target = root.join(&*id);
            fs::write(&target, content)
                .await
                .map_err(|err| AppError::write_error(err, target))?;
        }
    }

    Ok(false)
}

/// Copy artifact to target
pub async fn copy(op: Operation, output: Output) -> Result<bool, AppError> {
    let Operation::Copy { id, path: source } = op else {
        unreachable!()
    };

    match output {
        Output::Stdout => {}
        Output::Path(root) => {
            let target = root.join(&*id);
            fs::copy(source.clone(), target.clone())
                .await
                .map_err(|err| AppError::copy_error(err, source, target))?;
        }
    }

    Ok(false)
}

/// Compile operations graph to PlantUML
pub async fn graph(op: Operation, operations: Arc<Mutex<OpGraph>>) -> Result<bool, AppError> {
    let Operation::Graph { graph_type: GraphType::Dependencies } = op else {
        unreachable!()
    };
    let graph = operations.lock().expect("poisoned lock");
    let mut cards = String::from("@startuml\nskinparam defaultTextAlignment center\n'nodes\n");
    let mut deps = String::from("'dependencies\n");
    let nodes = graph
        .iter()
        .filter(|(_, op, _)| !matches!(op, Operation::Graph { .. }));

    for (target, _, edges) in nodes {
        let uri = target.uri();
        let uid = target.uid();
        let (schema, path) = uri.split_once(':').expect("uri to have schema");

        cards.push_str("card ");
        if path.is_empty() {
            writeln!(&mut cards, "\"({schema})\" as {uid}").expect("write");
        } else {
            writeln!(&mut cards, "\"{path}\\n({schema})\" as {uid}").expect("write");
        }

        for source in edges.iter().filter(|&e| e != &OpId::graph()) {
            writeln!(&mut deps, "{} --> {}", source.uid(), target.uid()).expect("write");
        }
    }

    println!("{}{}@enduml", cards, deps);

    Ok(true)
}

#[cfg(test)]
mod tests;
