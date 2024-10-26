use std::{
    path::PathBuf,
    sync::{Mutex, Weak},
};

use murkdown::ast::Node;

/// Artifact produced by a task
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Artifact {
    Path(PathBuf),
    String(String),
    Binary(Vec<u8>),
    Ast(Node),
    AstPointer(Weak<Mutex<Node>>),
}
