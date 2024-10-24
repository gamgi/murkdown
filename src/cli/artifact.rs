use std::path::PathBuf;

use murkdown::ast::Node;

/// Artifact produced by a task
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Artifact {
    Path(PathBuf),
    String(String),
    Binary(Vec<u8>),
    Ast(Node),
}
