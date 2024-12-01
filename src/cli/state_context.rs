use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc, Mutex, OnceLock},
};

use murkdown::compiler::Lang;
use murkdown::types::{AstMap, LocationMap};

#[cfg(test)]
use super::artifact::Artifact;
use super::{
    graph::OpGraph,
    op::{OpId, Operation},
    types::{AppError, AppErrorPathCtx, ArtifactMap, LangMap},
};

/// State container
#[derive(Debug, Clone)]
pub(crate) struct State {
    pub artifacts: Arc<Mutex<ArtifactMap>>,
    pub asts: Arc<Mutex<AstMap>>,
    pub locations: Arc<Mutex<LocationMap>>,
    pub languages: Arc<OnceLock<LangMap>>,
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
            languages: Arc::new(OnceLock::new()),
            operations: Arc::new(Mutex::new(OpGraph::new())),
            operations_processed: Arc::new(Mutex::new(HashSet::new())),
            should_exit: Arc::new(AtomicBool::new(false)),
        }
    }

    #[cfg(test)]
    pub fn new_loaded(format: &str) -> Self {
        let ctx = Self::new();
        ctx.load_languages(format).expect("valid format");
        ctx
    }

    #[cfg(test)]
    pub fn insert_op(&self, op: Operation) -> OpId {
        let mut ops = self.operations.lock().expect("poisoned lock");
        ops.insert_node(op)
    }

    #[cfg(test)]
    pub fn insert_artifact(&self, uri: &str, art: Artifact) {
        let mut arts = self.artifacts.lock().expect("poisoned lock");
        arts.insert(uri.to_string(), art);
    }

    #[cfg(test)]
    pub fn insert_location(&self, path: &str, location: impl Into<murkdown::types::Location>) {
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

    pub fn load_languages(&self, format: &str) -> Result<(), AppError> {
        if self.languages.get().is_none() {
            // builtin
            let markdown = include_str!("../lib/compiler/markdown.lang");
            let html = include_str!("../lib/compiler/html.lang");
            let plaintext = include_str!("../lib/compiler/plaintext.lang");

            let mut languages = HashMap::from([
                ("markdown".to_string(), Lang::new(markdown)?),
                ("html".to_string(), Lang::new(html)?),
                ("plaintext".to_string(), Lang::new(plaintext)?),
            ]);

            // custom
            if !languages.contains_key(format) {
                let path = PathBuf::from(format).with_extension("lang");
                let custom = std::fs::read_to_string(&path).with_ctx(path)?;
                languages.insert(format.to_string(), Lang::new(&custom)?);
            }

            self.languages.set(languages).expect("languages are loaded");
        }
        Ok(())
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
