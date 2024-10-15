use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use murkdown::types::LocationMap;
use walkdir::WalkDir;

use super::{
    graph::OpGraph,
    op::{OpId, Operation},
    types::AppError,
    utils::{is_file, is_visible},
};
use crate::cli::{command::Command, utils::into_uri_path_tuple};

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

/// Gather entry points from provided paths
pub async fn gather(op: Operation, operations: Arc<Mutex<OpGraph>>) -> Result<bool, AppError> {
    let Operation::Gather { cmd, paths } = &op else {
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

pub async fn load(op: Operation, operations: Arc<Mutex<OpGraph>>) -> Result<bool, AppError> {
    let Operation::Load { id, path } = &op else {
        unreachable!()
    };

    Ok(false)
}
