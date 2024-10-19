use std::{
    fmt::Write,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use murkdown::parser;
use murkdown::types::{LocationMap, URI};
use murkdown::{preprocessor, types::AstMap};
use walkdir::WalkDir;

use super::{
    graph::OpGraph,
    op::{OpId, Operation},
    types::{AppError, ArtifactMap, ErrorIdCtx, ErrorPathCtx},
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
            .filter(is_file);
        for entry in walker {
            locations.insert(
                entry.path().display().to_string(),
                entry.path().to_path_buf(),
            );
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

    // schedule dependent tasks
    for path in paths {
        let walker = WalkDir::new(path)
            .into_iter()
            .filter_entry(is_visible)
            .filter_map(Result::ok)
            .filter(is_file)
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
                    Operation::Finish,
                ]),
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
    let mut artifacts = artifacts.lock().expect("poisoned lock");
    artifacts.insert(op.uri(), artifact);
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

    match ast.clone() {
        Artifact::Ast(mut node) => {
            let mut asts = asts.lock().expect("poisoned lock");

            let deps = {
                let locs = locations.lock().expect("poisoned lock");
                preprocessor::preprocess(&mut node, &mut asts, &locs, id)
            };
            artifacts.insert(op.uri(), Artifact::Ast(node));

            // schedule dependent tasks
            for uri in deps {
                let (schema, path) = uri.split_once(':').expect("uri to have schema");

                if graph.get_uri(&uri).is_some() {
                    continue;
                }

                match schema {
                    "file" => {
                        graph.insert_node_chain([
                            Operation::Load {
                                id: path.into(),
                                path: PathBuf::from(path),
                            },
                            op.clone(),
                        ]);
                    }
                    "ast" => {
                        graph.insert_node_chain([
                            Operation::Load {
                                id: path.into(),
                                path: PathBuf::from(path),
                            },
                            Operation::Parse { id: path.into() },
                            op.clone(),
                        ]);
                    }
                    "parse" => {
                        graph.insert_node_chain([
                            Operation::Load {
                                id: path.into(),
                                path: PathBuf::from(path),
                            },
                            Operation::Parse { id: path.into() },
                            Operation::Preprocess { id: path.into() },
                            op.clone(),
                        ]);
                    }
                    _ => todo!("unknown schema"),
                }
            }
        }
        _ => panic!("preprocessing unknown artifact"),
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
