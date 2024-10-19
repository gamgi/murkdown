use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex, Weak},
};

use crate::{ast::Node, parser::Rule};

pub type ParseError = pest::error::Error<Rule>;

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
