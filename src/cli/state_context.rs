#[cfg(test)]
use std::path::PathBuf;
use std::{
    collections::{HashMap, HashSet},
    sync::{atomic::AtomicBool, Arc, Mutex},
};

use murkdown::types::{AstMap, LocationMap};

#[cfg(test)]
use super::artifact::Artifact;
use super::{
    graph::OpGraph,
    op::{OpId, Operation},
    types::ArtifactMap,
};

/// State container
#[derive(Debug, Clone)]
pub struct State {
    pub artifacts: Arc<Mutex<ArtifactMap>>,
    pub asts: Arc<Mutex<AstMap>>,
    pub locations: Arc<Mutex<LocationMap>>,
    pub operations: Arc<Mutex<OpGraph>>,
    pub operations_processed: Arc<Mutex<HashSet<OpId>>>,
    pub should_exit: Arc<AtomicBool>,
}

impl State {
    pub fn new() -> Self {
        Self {
            artifacts: Arc::new(Mutex::new(HashMap::new())),
            asts: Arc::new(Mutex::new(HashMap::new())),
            locations: Arc::new(Mutex::new(HashMap::new())),
            operations: Arc::new(Mutex::new(OpGraph::new())),
            operations_processed: Arc::new(Mutex::new(HashSet::new())),
            should_exit: Arc::new(AtomicBool::new(false)),
        }
    }

    #[cfg(test)]
    pub fn insert_artifact(&self, uri: &str, art: Artifact) {
        let mut arts = self.artifacts.lock().expect("poisoned lock");
        arts.insert(uri.to_string(), art);
    }

    #[cfg(test)]
    pub fn insert_location(&self, path: &str, location: impl Into<PathBuf>) {
        let mut locs = self.locations.lock().expect("poisoned lock");
        locs.insert(path.to_string(), location.into());
    }

    pub fn insert_op_chain<I>(&self, new_ops: I)
    where
        I: IntoIterator<Item = Operation>,
    {
        let mut ops = self.operations.lock().expect("poisoned lock");
        ops.insert_node_chain(new_ops)
    }

    pub fn mark_op_processed(&self, id: OpId) {
        let mut processed = self.operations_processed.lock().expect("poisoned lock");
        processed.insert(id);
    }

    pub fn is_op_processed(&self, id: &OpId) -> bool {
        let processed = self.operations_processed.lock().expect("poisoned lock");
        processed.contains(id)
    }

    /// Clear state
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        let mut ops = self.operations.lock().expect("poisoned lock");
        ops.clear();
        let mut processed = self.operations_processed.lock().expect("poisoned lock");
        processed.clear();
    }
}
