use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex, Weak},
};

use thiserror::Error;

use crate::{ast::Node, parser::Rule};

/// Uniform Resource Identifier (eg. load:foo.fd)
pub type URI = String;

/// Map from URI to AST node
pub type AstMap = HashMap<String, Arc<Mutex<Node>>>;

/// Map from Resource Name (eg. foo.fd) to location on disk
pub type LocationMap = HashMap<String, PathBuf>;

#[derive(Debug, Clone)]
pub struct Pointer(pub Weak<Mutex<Node>>);

/// Pointer equality is ignored
impl PartialEq for Pointer {
    fn eq(&self, _: &Pointer) -> bool {
        true
    }
}

impl Eq for Pointer {}

#[derive(Error, Debug)]
pub enum LibError {
    #[error(transparent)]
    ParseError(#[from] Box<pest::error::Error<Rule>>),
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
