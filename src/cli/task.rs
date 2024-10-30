use std::{
    collections::hash_map::Entry,
    fmt::Write,
    path::PathBuf,
    str::FromStr,
    sync::{Arc, Mutex},
};

use murkdown::{
    ast::{Node, NodeBuilder},
    types::{Dependency, ExecArtifact, LibErrorPathCtx, LocationMap, URI},
};
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
use crate::cli::{
    artifact::Artifact,
    command::Command,
    utils::{into_uri_path_tuple, spawn_command, wait_command, write_command},
};

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

/// Execute a command
pub async fn exec(
    op: Operation,
    asts: Arc<Mutex<AstMap>>,
    artifacts: Arc<Mutex<ArtifactMap>>,
) -> Result<bool, AppError> {
    let uri = op.uri();
    let Operation::Exec { ref cmd, ref input, artifact, .. } = op else {
        unreachable!()
    };

    let (program, args) = cmd.split_once(' ').unwrap_or((cmd, ""));

    let mut child = spawn_command(program, args)?;
    write_command(&mut child, input.as_deref(), program).await?;
    let result = wait_command(child, program).await?;

    let stdout_artifact = match String::from_utf8(result.stdout) {
        Ok(v) => Artifact::Plaintext("text/plain".to_string(), v),
        Err(v) => Artifact::Binary("application/octet-stream".to_string(), v.into_bytes()),
    };
    let stderr_artifact = match String::from_utf8(result.stderr) {
        Ok(v) => Artifact::Plaintext("text/plain".to_string(), v),
        Err(v) => Artifact::Binary("application/octet-stream".to_string(), v.into_bytes()),
    };

    let main_artifact = match artifact {
        ExecArtifact::Stdout(_) => stdout_artifact.clone(),
        ExecArtifact::Path(path) => Artifact::Path(path.clone()),
    };

    // upsert nodes to ast
    let mut asts = asts.lock().expect("poisoned lock");

    if let Artifact::Plaintext(_, content) = &main_artifact {
        let node = NodeBuilder::root()
            .add_section(content.split('\n').map(Node::new_line).collect())
            .done();

        match asts.entry(uri.clone()) {
            Entry::Occupied(r) => {
                let mut mutex = r.get().lock().expect("poisoned lock");
                *mutex = node;
                &r.get().clone()
            }
            Entry::Vacant(r) => &*r.insert(Arc::new(Mutex::new(node))),
        };
    }

    if let Artifact::Plaintext(_, content) = &stdout_artifact {
        let node = NodeBuilder::root()
            .add_section(content.split('\n').map(Node::new_line).collect())
            .done();

        match asts.entry(uri.replacen("exec:", "exec:stdout:", 1)) {
            Entry::Occupied(r) => {
                let mut mutex = r.get().lock().expect("poisoned lock");
                *mutex = node;
                &r.get().clone()
            }
            Entry::Vacant(r) => &*r.insert(Arc::new(Mutex::new(node))),
        };
    }

    if let Artifact::Plaintext(_, content) = &stderr_artifact {
        let node = NodeBuilder::root()
            .add_section(content.split('\n').map(Node::new_line).collect())
            .done();

        match asts.entry(uri.replacen("exec:", "exec:stderr:", 1)) {
            Entry::Occupied(r) => {
                let mut mutex = r.get().lock().expect("poisoned lock");
                *mutex = node;
                &r.get().clone()
            }
            Entry::Vacant(r) => &*r.insert(Arc::new(Mutex::new(node))),
        };
    }

    // add artifact
    let mut artifacts = artifacts.lock().expect("poisoned lock");
    artifacts.insert(uri, main_artifact);

    Ok(false)
}

/// Load files
pub async fn load(
    op: Operation,
    asts: Arc<Mutex<AstMap>>,
    artifacts: Arc<Mutex<ArtifactMap>>,
) -> Result<bool, AppError> {
    let uri = op.uri();
    let Operation::Load { path, .. } = &op else {
        unreachable!()
    };
    let artifact = match tokio::fs::read_to_string(path).await {
        Ok(contents) => Artifact::Plaintext("text/plain".to_string(), contents),
        Err(_) => Artifact::Binary(
            "application/octet-stream".to_string(),
            std::fs::read(path).with_ctx(path)?,
        ),
    };

    // add node to ast
    let mut asts = asts.lock().expect("poisoned lock");
    asts.entry(uri.clone()).or_insert_with(|| {
        let node = match &artifact {
            Artifact::Plaintext(_, content) => NodeBuilder::root()
                .add_section(content.split('\n').map(Node::new_line).collect())
                .done(),
            _ => todo!(),
        };
        Arc::new(Mutex::new(node))
    });

    // add artifact
    let mut artifacts = artifacts.lock().expect("poisoned lock");
    artifacts.insert(uri, artifact);

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
        Artifact::Plaintext(_, content) => {
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
            let deps = preprocessor::preprocess(&mut node, &mut asts, &locs, id)?;

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

            let (uri_deps, exec_deps): (Vec<_>, Vec<_>) = deps
                .into_iter()
                .partition(|d| matches!(d, Dependency::URI(_, _)));

            // schedule dependent tasks
            for uri in uri_deps {
                let Dependency::URI(kind, ref uri) = uri else {
                    unreachable!()
                };
                let (schema, uri_path) = uri.split_once(':').expect("uri to have schema");
                let id = Arc::from(uri_path);

                if graph.get_uri(uri).is_some() {
                    continue;
                }

                match kind {
                    // means include
                    "src" => {
                        if schema == "exec" {
                            graph.add_dependency(OpId::from(&op), OpId::exec(id));
                            continue;
                        }
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
                                graph.insert_node_chain([
                                    Operation::Copy { id, path },
                                    Operation::Finish,
                                ]);
                            }
                            _ => return Err(AppError::unknown_schema(schema)),
                        }
                    }
                    // means copy or write
                    "ref" => {
                        let op = Operation::Write { id };
                        graph.add_dependency(OpId::from(&op), OpId::from_str(uri)?);
                        graph.insert_node_chain([op, Operation::Finish]);
                    }
                    _ => unreachable!(),
                }
            }

            for dep in exec_deps {
                let Dependency::Exec { cmd, id, input, artifact, .. } = dep else {
                    unreachable!()
                };
                let id: Arc<str> = id.into();

                graph.insert_node(Operation::Exec { id: id.clone(), cmd, input, artifact });
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
    artifacts.insert(
        op.uri(),
        Artifact::Plaintext("text/markdown".to_string(), result),
    );

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
            Artifact::Plaintext(_, content) => content.to_string(),
            Artifact::Binary(_, content) => String::from_utf8_lossy(content).to_string(),
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
