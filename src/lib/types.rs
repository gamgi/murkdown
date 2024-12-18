use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex, Weak},
};

use thiserror::Error;

use crate::{
    ast::Node,
    compiler::{self, rule::LangRule},
    parser,
};

/// Uniform Resource Identifier (eg. load:foo.fd)
pub type URI = String;

/// Map from URI to AST node
pub type AstMap = HashMap<String, Arc<Mutex<Node>>>;

/// Map from processing stage (eg. preprocess) to list of rules
pub(crate) type RuleMap = HashMap<&'static str, Vec<LangRule>>;

/// Map from Resource path (eg. foo.fd) to location on disk
pub type LocationMap = HashMap<String, Location>;

#[derive(Debug, Clone)]
pub struct Pointer(pub Weak<Mutex<Node>>);

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Location {
    Path(PathBuf),
    DataURL(String),
}

impl From<PathBuf> for Location {
    fn from(value: PathBuf) -> Self {
        Self::Path(value)
    }
}

/// Dependency discovered by the preprocessor or compiler
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Dependency {
    URI(&'static str, URI),
    Exec {
        id: String,
        cmd: String,
        input: Option<String>,
        artifact: ExecArtifact,
    },
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum ExecArtifact {
    Stdout(String),
    Path(PathBuf),
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum ExecInput {
    String(String),
    URI(URI),
}

/// Pointer equality is ignored
impl PartialEq for Pointer {
    fn eq(&self, _: &Pointer) -> bool {
        true
    }
}

impl Eq for Pointer {}

#[derive(Error, Debug, thiserror_ext::Construct)]
pub enum LibError {
    #[error(transparent)]
    ParseError(#[from] Box<pest::error::Error<parser::Rule>>),
    #[error(transparent)]
    ParseRuleError(#[from] Box<pest::error::Error<compiler::Rule>>),
    #[error(transparent)]
    BadRuleRegex(#[from] regex::Error),
    #[error("missing root")]
    MissingRoot,
    #[error("unknown rule section `{0}`")]
    UnknownRuleSection(String),
    #[error("invalid rule `{0}`")]
    InvalidRule(String),
    #[error("invalid argument `{0}`")]
    InvalidRuleArgument(String),
    #[error("invalid argument type `{0}` expected `{1}`")]
    InvalidRuleArgumentType(String, &'static str),
}

pub trait LibErrorPathCtx<T> {
    fn with_path(self, id: &str) -> Result<T, LibError>;
}

impl<T> LibErrorPathCtx<T> for Result<T, LibError> {
    fn with_path(self, path: &str) -> Result<T, LibError> {
        self.map_err(|e| match e {
            LibError::ParseError(error) => LibError::ParseError(Box::new(error.with_path(path))),
            _ => e,
        })
    }
}
