use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use murkdown::parser;
use murkdown::types::{LocationMap, URI};
use log::info;
use walkdir::WalkDir;

use super::{
    graph::OpGraph,
    op::{OpId, Operation},
    types::{AppError, ArtifactMap, ErrorPathCtx},
    utils::{is_file, is_visible},
};
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

// TODO rename schedule or coordintate or something that decidestasks
/// Gather entry points from provided paths
pub async fn gather(op: Operation, operations: Arc<Mutex<OpGraph>>) -> Result<bool, AppError> {
    let Operation::Gather { cmd, paths, splits: _ } = &op else {
        panic!()
    };
    let mut graph = operations.lock().expect("poisoned lock");
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
                Command::Build { .. } => graph.insert_node_chain([
                    Operation::Load { id: id.clone(), path },
                    Operation::Parse { id: id.clone() },
                    Operation::Finish,
                ]),
            }
        }
    }

    Ok(false)
}

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

pub async fn parse(
    op: Operation,
    dep: URI,
    artifacts: Arc<Mutex<ArtifactMap>>,
) -> Result<bool, AppError> {
    let Operation::Parse { ref id } = &op else {
        unreachable!()
    };
    let artifacts = artifacts.lock().expect("poisoned lock");
    let content = artifacts.get(&dep).expect("no parse dependency");

    match content {
        Artifact::String(content) => {
            let ast = parser::parse(content).map(Artifact::Ast)?;
        }
        _ => todo!(),
    }

    Ok(false)
}
