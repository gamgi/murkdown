use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use clap::error::Error as ClapError;
use data_url::{forgiving_base64::InvalidBase64, DataUrl, DataUrlError};
use murkdown::{
    compiler::Lang,
    types::{LibError, Location, URI},
};
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

/// Map from format to language rules
pub(crate) type LangMap = HashMap<String, Lang>;

/// Output target
#[derive(Debug, Default, Clone)]
pub enum Output {
    #[default]
    StdOut,
    StdOutLog,
    Path(PathBuf),
}

impl std::fmt::Display for Output {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Output::StdOut => write!(f, "stdout"),
            Output::StdOutLog => write!(f, "stdout"),
            Output::Path(path_buf) => write!(f, "{}", path_buf.display()),
        }
    }
}

/// Data Source
#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum Source {
    Path(PathBuf),
    Url(String),
}

impl Source {
    #[cfg(test)]
    pub fn empty() -> Self {
        Self::Path(PathBuf::new())
    }

    /// Source path or fragment
    pub fn path(&self) -> Result<String, AppError> {
        match self {
            Source::Path(path) => Ok(path.display().to_string()),
            Source::Url(pattern) => {
                let url = DataUrl::process(pattern)?;
                let fragment = url.decode_to_vec()?.1;
                match fragment {
                    Some(f) => Ok(f.to_percent_encoded()),
                    None => Err(AppError::bad_data_url_fragment(pattern)),
                }
            }
        }
    }
}

impl From<&String> for Source {
    fn from(value: &String) -> Self {
        match value {
            v if v.starts_with("data:") => Self::Url(v.to_string()),
            v => Self::Path(PathBuf::from(v)),
        }
    }
}

impl From<&str> for Source {
    fn from(value: &str) -> Self {
        match value {
            v if v.starts_with("data:") => Self::Url(v.to_string()),
            v => Self::Path(PathBuf::from(v)),
        }
    }
}

impl PartialEq<Path> for Source {
    fn eq(&self, other: &Path) -> bool {
        match self {
            Source::Path(path_buf) => path_buf == other,
            Source::Url(_) => false,
        }
    }
}

impl TryInto<PathBuf> for Source {
    type Error = std::io::Error;

    fn try_into(self) -> Result<PathBuf, Self::Error> {
        use std::io::{Error, ErrorKind};
        match self {
            Source::Path(path_buf) => Ok(path_buf),
            Source::Url(_) => Err(Error::new(
                ErrorKind::InvalidInput,
                "cannot build path fomr data url",
            )),
        }
    }
}

#[allow(clippy::from_over_into)]
impl Into<Location> for Source {
    fn into(self) -> Location {
        match self {
            Source::Path(path_buf) => Location::Path(path_buf),
            Source::Url(url) => Location::DataURL(url),
        }
    }
}

impl From<Location> for Source {
    fn from(value: Location) -> Self {
        match value {
            Location::Path(path_buf) => Self::Path(path_buf),
            Location::DataURL(url) => Self::Url(url),
        }
    }
}

impl From<PathBuf> for Source {
    fn from(value: PathBuf) -> Self {
        Self::Path(value)
    }
}

#[derive(Error, Debug, thiserror_ext::Box, thiserror_ext::Construct)]
#[thiserror_ext(newtype(name = AppError))]
pub enum AppErrorKind {
    #[error("could not parse command")]
    ClapError(#[from] ClapError),
    #[error("execution of `{program}` failed: {reason}")]
    ExecutionFailed { reason: String, program: String },
    #[error("execution of `{program}` failed: {source}")]
    ExecutionIoFailed {
        #[backtrace]
        source: std::io::Error,
        program: String,
    },
    #[error("execution of `{program}` exited with code: {code}")]
    ExecutionExited { program: String, code: i32 },
    #[error("invalid arguments for `{program}`: {args}")]
    BadExecArgs { program: String, args: String },
    #[error("file not found `{0}`")]
    FileNotFound(String),
    #[error("internal channel error")]
    SendError,
    #[error("invalid path `{0}`")]
    BadPath(PathBuf),
    #[error("invalid URI: {0}")]
    BadUri(String),
    #[error("invalid URL: {0}")]
    BadUrl(String),
    #[error("invalid data URL: {0}")]
    BadDataUrl(#[from] DataUrlError),
    #[error("missing data URL fragment: {0}")]
    BadDataUrlFragment(String),
    #[error("invalid data URL encoding: {0}")]
    BadDataUrlBase64(#[from] InvalidBase64),
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
    #[error("unknown language: {0}")]
    UnknownLanguage(String),
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
