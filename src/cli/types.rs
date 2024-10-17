use std::{collections::HashMap, path::PathBuf};

use clap::error::Error as ClapError;
use murkdown::types::{ParseError, URI};
use thiserror::Error;
use tokio::sync::mpsc::{self};

use super::{artifact::Artifact, command::Command};

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

#[derive(Error, Debug, thiserror_ext::Box, thiserror_ext::Construct)]
#[thiserror_ext(newtype(name = AppError))]
pub enum AppErrorKind {
    #[error("could not parse command")]
    ClapError(#[from] ClapError),
    #[error("internal channel error")]
    SendError,
    #[error("invalid path `{0}`")]
    BadPath(PathBuf),
    #[error("invalid URI: {0}")]
    BadUri(String),
    #[error("could not read `{path}`")]
    ReadError {
        #[backtrace]
        source: std::io::Error,
        path: PathBuf,
    },
    #[error(transparent)]
    Parse(#[from] Box<ParseError>),
}

pub trait ErrorPathCtx<T> {
    fn with_ctx<P: Into<PathBuf>>(self, path: P) -> Result<T, AppError>;
}

impl<T> ErrorPathCtx<T> for Result<T, std::io::Error> {
    fn with_ctx<P: Into<PathBuf>>(self, path: P) -> Result<T, AppError> {
        self.map_err(|source| AppError::read_error(source, path))
    }
}
