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
use crate::cli::utils::into_uri_path_tuple;

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
            let load = graph.add_node(Operation::Load { id, path });
            graph.add_dependency(OpId::finish(), load);
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
