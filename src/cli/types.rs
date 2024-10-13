use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use clap::error::Error as ClapError;
use murkdown::types::LocationMap;
use thiserror::Error;
use tokio::sync::mpsc::{self};

use super::{command::Command, graph::OpGraph, op::Operation};

pub type EventTx = mpsc::UnboundedSender<Event>;
pub type EventRx = mpsc::UnboundedReceiver<Event>;

#[derive(Debug)]
pub enum Event {
    Command(Result<Command, ClapError>),
    CommandOk,
    TaskOk,
    TaskError(AppError),
}

#[derive(Debug, Clone)]
pub struct State {
    pub locations: Arc<Mutex<LocationMap>>,
    pub operations: Arc<Mutex<OpGraph>>,
}

impl State {
    pub fn new() -> Self {
        Self {
            locations: Arc::new(Mutex::new(HashMap::new())),
            operations: Arc::new(Mutex::new(OpGraph::new())),
        }
    }

    pub fn add_op(&self, op: Operation) {
        let mut ops = self.operations.lock().expect("poisoned lock");
        ops.add_node(op);
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
    PathError(PathBuf),
}
