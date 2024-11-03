use std::{
    collections::hash_map::Entry,
    fmt::Write,
    path::PathBuf,
    str::FromStr,
    sync::{Arc, Mutex, OnceLock},
};

use log::{debug, info, trace, warn};
use murkdown::{
    ast::{Node, NodeBuilder},
    types::{Dependency, ExecArtifact, ExecInput, LibErrorPathCtx, LocationMap, URI},
};
use murkdown::{compiler, parser};
use murkdown::{preprocessor, types::AstMap};
use tokio::fs;
use walkdir::{DirEntry, WalkDir};

use super::{
    graph::OpGraph,
    op::{OpId, Operation},
    types::{AppError, AppErrorPathCtx, ArtifactMap, LangMap, Output},
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
    debug!("Indexing files");

    let mut count = 0;
    let mut locations = locations.lock().expect("poisoned lock");
    for path in paths {
        let walker = WalkDir::new(path)
            .into_iter()
            .filter_entry(is_visible)
            .filter_map(Result::ok)
            .filter(is_file)
            .map(into_uri_path_tuple);
        for (id, path) in walker {
            trace!("Indexed {}", id);
            count += 1;
            locations.insert(id, path);
        }
    }
    debug!("Indexed {count} files");

    Ok(false)
}

/// Gather entry points and schedule dependencies
pub async fn gather(op: Operation, operations: Arc<Mutex<OpGraph>>) -> Result<bool, AppError> {
    let Operation::Gather { cmd, paths, splits: _ } = &op else {
        panic!()
    };
    debug!("Gathering files");

    let mut count = 0;
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
            trace!("Gathered {}", id);

            count += 1;
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

    debug!("Gathered {count} files");

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

    let input = match input {
        Some(ExecInput::String(content)) => Some(Arc::from(content.as_ref())),
        Some(ExecInput::URI(uri)) => {
            let artifacts = artifacts.lock().expect("poisoned lock");

            match artifacts.get(uri) {
                Some(Artifact::Plaintext(_, content)) => Some(Arc::from(content.as_ref())),
                Some(_) => todo!(),
                None => {
                    warn!("Execution input {} not found", uri);
                    None
                }
            }
        }
        None => None,
    };

    if input.is_some() {
        debug!("Executing {cmd} with input");
    } else {
        debug!("Executing {cmd}");
    }

    let mut child = spawn_command(program, args)?;
    write_command(&mut child, input.as_deref(), program).await?;
    let result = wait_command(child, program).await?;

    if let Some(code) = result.status.code() {
        if code != 0 {
            return Err(AppError::execution_exited(program, code));
        }
    }

    let stdout_artifact = match String::from_utf8(result.stdout) {
        Ok(v) => Artifact::Plaintext("text/plain".to_string(), v),
        Err(v) => Artifact::Binary("application/octet-stream".to_string(), v.into_bytes()),
    };
    let stderr_artifact = match String::from_utf8(result.stderr) {
        Ok(v) => Artifact::Plaintext("text/plain".to_string(), v),
        Err(v) => {
            warn!("Execution {cmd} stderr is binary");
            Artifact::Binary("application/octet-stream".to_string(), v.into_bytes())
        }
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
    let Operation::Load { id, path } = &op else {
        unreachable!()
    };
    debug!("Loading {id}");
    let artifact = match tokio::fs::read_to_string(path).await {
        Ok(contents) => Artifact::Plaintext("text/plain".to_string(), contents),
        Err(_) => Artifact::Binary(
            "application/octet-stream".to_string(),
            std::fs::read(path).with_ctx(path)?,
        ),
    };

    let node = match &artifact {
        Artifact::Plaintext(_, content) => NodeBuilder::root()
            .add_section(content.split('\n').map(Node::new_line).collect())
            .done(),
        _ => todo!(),
    };

    // upsert node to ast
    let mut asts = asts.lock().expect("poisoned lock");
    match asts.entry(uri.clone()) {
        Entry::Occupied(r) => {
            let mut mutex = r.get().lock().expect("poisoned lock");
            *mutex = node;
            &r.get().clone()
        }
        Entry::Vacant(r) => &*r.insert(Arc::new(Mutex::new(node))),
    };

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
    debug!("Parsing {id}");
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
#[allow(clippy::too_many_arguments)]
pub async fn preprocess(
    op: Operation,
    format: String,
    dep: URI,
    asts: Arc<Mutex<AstMap>>,
    operations: Arc<Mutex<OpGraph>>,
    artifacts: Arc<Mutex<ArtifactMap>>,
    languages: Arc<OnceLock<LangMap>>,
    locations: Arc<Mutex<LocationMap>>,
) -> Result<bool, AppError> {
    let Operation::Preprocess { ref id } = op else {
        unreachable!()
    };
    debug!("Preprocessing {id}");
    let mut artifacts = artifacts.lock().expect("poisoned lock");
    let ast = artifacts.get(&dep).expect("no preprocess dependency");
    let mut graph = operations.lock().expect("poisoned lock");

    // NOTE: clone to keep source AST intact
    match ast.clone() {
        Artifact::Ast(mut node) => {
            let mut asts = asts.lock().expect("poisoned lock");
            let uri = op.uri();

            let locs = locations.lock().expect("poisoned lock");
            let lang = languages.get().expect("languages not loaded").get(&format);
            let deps = preprocessor::preprocess(&mut node, &mut asts, &locs, id, lang)?;

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

            debug!("Preprocessing {id} yielded {} dependencies", deps.len());
            let (uri_deps, exec_deps): (Vec<_>, Vec<_>) = deps
                .into_iter()
                .partition(|d| matches!(d, Dependency::URI(_, _)));

            // schedule dependent tasks
            for uri in uri_deps {
                let Dependency::URI(kind, ref uri) = uri else {
                    unreachable!()
                };
                let (schema, uri_path) = uri.split_once(':').expect("uri to have schema");

                let (uri_path, _) = uri_path.rsplit_once('#').unwrap_or((uri_path, ""));
                let id: Arc<str> = Arc::from(uri_path);
                let uri = format!("{schema}:{uri_path}");
                if graph.get_uri(&uri).is_some() {
                    continue;
                }

                match kind {
                    // means include
                    "src" => {
                        // NOTE: these are technically not correct, since the dependency to exec should be
                        // on the dependent node of current op, but we don't have a mapping for that now
                        if schema == "exec" {
                            graph.add_dependency(OpId::from(&op), OpId::exec(uri_path));
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
                    "ref" => match schema {
                        "exec" => {
                            let id: Arc<str> = uri_path.into();
                            graph.add_dependency(OpId::write(id.clone()), OpId::exec(uri_path));
                            graph.insert_node_chain([Operation::Write { id }, Operation::Finish]);
                        }
                        "copy" => {
                            let path = locs
                                .get(uri_path)
                                .ok_or(AppError::file_not_found(uri_path))?
                                .clone();
                            graph.insert_node_chain([
                                Operation::Copy { id, path },
                                Operation::Finish,
                            ]);
                        }
                        _ => {
                            let op = Operation::Write { id };
                            graph.add_dependency(OpId::from(&op), OpId::from_str(&uri)?);
                            graph.insert_node_chain([op, Operation::Finish]);
                        }
                    },
                    _ => unreachable!(),
                }
            }

            for dep in exec_deps {
                let Dependency::Exec { cmd, id, input, artifact, .. } = dep else {
                    unreachable!()
                };
                let input = input.map(ExecInput::String);
                graph.insert_node(Operation::Exec { id: id.into(), cmd, input, artifact });
            }
        }
        _ => panic!("preprocessing unknown artifact"),
    }

    Ok(false)
}

/// Compile AST to string
pub async fn compile(
    op: Operation,
    format: String,
    dep: URI,
    artifacts: Arc<Mutex<ArtifactMap>>,
    languages: Arc<OnceLock<LangMap>>,
) -> Result<bool, AppError> {
    let Operation::Compile { ref id } = op else {
        unreachable!()
    };
    debug!("Compiling {id}");
    let mut artifacts = artifacts.lock().expect("poisoned lock");
    let ast = artifacts.get(&dep).expect("no compile dependency");
    let lang = languages.get().expect("languages not loaded").get(&format);

    let result = match ast.clone() {
        Artifact::Ast(mut node) => compiler::compile(&mut node, lang).unwrap(),
        Artifact::AstPointer(pointer) => {
            let mutex = pointer.upgrade().unwrap();
            let mut node = mutex.lock().unwrap();
            compiler::compile(&mut node, lang).unwrap()
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
        if let Artifact::Plaintext(_, content) = result {
            match output {
                Output::StdOut => {
                    debug!("Writing {id} to stdout");
                    println!("{content}");
                    return Ok(false);
                }
                Output::StdOutLog => {
                    debug!("Writing {id} to stdout");
                    info!(target = *id; "{}", content);
                    return Ok(false);
                }
                _ => {}
            };
        }

        match result {
            Artifact::Plaintext(_, content) => content.as_bytes().to_owned(),
            Artifact::Binary(_, content) => content.to_owned(),
            _ => panic!("writing unknown artifact"),
        }
    };

    if let Output::Path(root) = output {
        let target = root.join(&*id);
        debug!("Writing {id} to {}", target.display());
        fs::write(&target, content)
            .await
            .map_err(|err| AppError::write_error(err, target))?;
    }

    Ok(false)
}

/// Copy artifact to target
// TODO should the output come in op instead?
pub async fn copy(op: Operation, output: Output) -> Result<bool, AppError> {
    let Operation::Copy { id, path: source } = op else {
        unreachable!()
    };

    match output {
        Output::StdOut | Output::StdOutLog => {
            debug!("Copying of {id} skipped");
        }
        Output::Path(root) => {
            let target = root.join(&*id);
            debug!("Copying {id} to {}", target.display());
            fs::copy(source.clone(), target.clone())
                .await
                .map_err(|err| AppError::copy_error(err, source, target))?;
        }
    }

    Ok(false)
}

/// Compile operations graph to PlantUML
pub async fn graph(
    op: Operation,
    operations: Arc<Mutex<OpGraph>>,
    artifacts: Arc<Mutex<ArtifactMap>>,
) -> Result<bool, AppError> {
    let Operation::Graph { graph_type: GraphType::Dependencies } = op else {
        unreachable!()
    };
    debug!("Graphing");
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

        for source in edges.iter().filter(|&e| !e.is_hidden()) {
            writeln!(&mut deps, "{} --> {}", source.uid(), target.uid()).expect("write");
        }
    }
    let result = format!("{}{}@enduml", cards, deps);

    let mut artifacts = artifacts.lock().expect("poisoned lock");
    artifacts.insert(
        op.uri(),
        Artifact::Plaintext("text/plantuml".to_string(), result),
    );

    Ok(false)
}

/// Finish operations
pub async fn finish(_: Operation) -> Result<bool, AppError> {
    Ok(true)
}

#[cfg(test)]
mod tests;
