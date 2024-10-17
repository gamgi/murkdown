use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::{ast::Node, parser::Rule};

/// Uniform Resource Identifier (eg. load:foo.fd)
pub type URI = String;

/// Map from URI to AST node
pub type AstMap = HashMap<String, Arc<Mutex<Node>>>;

/// Map from Resource Name (eg. foo.fd) to location on disk
pub type LocationMap = HashMap<String, PathBuf>;

pub type ParseError = pest::error::Error<Rule>;
