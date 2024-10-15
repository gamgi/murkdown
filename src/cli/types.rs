use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, Mutex},
};

use clap::error::Error as ClapError;
use murkdown::types::{LocationMap, URI};
use thiserror::Error;
use tokio::sync::mpsc::{self};

use super::{
    artifact::Artifact,
    command::Command,
    graph::OpGraph,
    op::{OpId, Operation},
};

pub type EventTx = mpsc::UnboundedSender<Event>;
pub type EventRx = mpsc::UnboundedReceiver<Event>;

#[derive(Debug)]
pub enum Event {
    Command(Result<Command, ClapError>),
    CommandOk,
    TaskOk,
    TaskError(AppError),
}

/// Map from URI (eg. load:foo.fd) to artefact
pub type ArtifactMap = HashMap<URI, Artifact>;

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

#[derive(Error, Debug, thiserror_ext::Box, thiserror_ext::Construct)]
#[thiserror_ext(newtype(name = AppError))]
pub enum AppErrorKind {
    #[error("could not parse command")]
    ClapError(#[from] ClapError),
    #[error("internal channel error")]
    SendError,
    #[error("invalid path `{0}`")]
    BadPath(PathBuf),
    #[error("could not read `{path}`")]
    ReadError {
        #[backtrace]
        source: std::io::Error,
        path: PathBuf,
    },
}

pub trait ErrorPathCtx<T> {
    fn with_ctx<P: Into<PathBuf>>(self, path: P) -> Result<T, AppError>;
}

impl<T> ErrorPathCtx<T> for Result<T, std::io::Error> {
    fn with_ctx<P: Into<PathBuf>>(self, path: P) -> Result<T, AppError> {
        self.map_err(|source| AppError::read_error(source, path))
    }
}
