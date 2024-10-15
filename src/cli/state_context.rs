use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use murkdown::types::LocationMap;

use super::{
    graph::OpGraph,
    op::{OpId, Operation},
    types::ArtifactMap,
};

/// State container
#[derive(Debug, Clone)]
pub struct State {
    pub artifacts: Arc<Mutex<ArtifactMap>>,
    pub locations: Arc<Mutex<LocationMap>>,
    pub operations: Arc<Mutex<OpGraph>>,
    pub operations_processed: Arc<Mutex<HashSet<OpId>>>,
}

impl State {
    pub fn new() -> Self {
        Self {
            artifacts: Arc::new(Mutex::new(HashMap::new())),
            locations: Arc::new(Mutex::new(HashMap::new())),
            operations: Arc::new(Mutex::new(OpGraph::new())),
            operations_processed: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn insert_op(&self, op: Operation) -> OpId {
        let mut ops = self.operations.lock().expect("poisoned lock");
        ops.insert_node(op)
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
    pub fn clear(&mut self) {
        let mut ops = self.operations.lock().expect("poisoned lock");
        ops.clear();
        let mut processed = self.operations_processed.lock().expect("poisoned lock");
        processed.clear();
    }
}
