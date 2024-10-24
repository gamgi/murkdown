use std::{collections::HashMap, path::PathBuf};

use clap::error::Error as ClapError;
use murkdown::types::{LibError, URI};
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

/// Output target
#[derive(Debug, Default, Clone)]
pub enum Output {
    #[default]
    Stdout,
    Path(PathBuf),
}

#[derive(Error, Debug, thiserror_ext::Box, thiserror_ext::Construct)]
#[thiserror_ext(newtype(name = AppError))]
pub enum AppErrorKind {
    #[error("interrupted by user")]
    Exit(u8),
    #[error("could not parse command")]
    ClapError(#[from] ClapError),
    #[error("internal channel error")]
    SendError,
    #[error("invalid path `{0}`")]
    BadPath(PathBuf),
    #[error("invalid URI: {0}")]
    BadUri(String),
    #[error("unknown URI schema `{0}`")]
    UnknownSchema(String),
    #[error("could not read `{path}`: {source}")]
    ReadError {
        #[backtrace]
        source: std::io::Error,
        path: PathBuf,
    },
    #[error("could not write `{path}`: {source}")]
    WriteError {
        #[backtrace]
        source: std::io::Error,
        path: PathBuf,
    },
    #[error("could not copy `{source_path}` to `{target_path}`: {source}")]
    CopyError {
        #[backtrace]
        source: std::io::Error,
        source_path: PathBuf,
        target_path: PathBuf,
    },
    #[error(transparent)]
    Lib(#[from] LibError),
}

pub trait AppErrorPathCtx<T> {
    fn with_ctx<P: Into<PathBuf>>(self, path: P) -> Result<T, AppError>;
}

impl<T> AppErrorPathCtx<T> for Result<T, std::io::Error> {
    fn with_ctx<P: Into<PathBuf>>(self, path: P) -> Result<T, AppError> {
        self.map_err(|source| AppError::read_error(source, path))
    }
}
