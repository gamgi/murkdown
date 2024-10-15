use std::path::PathBuf;

/// Artifact produced by a task
#[derive(Debug, Clone)]
pub enum Artifact {
    Path(PathBuf),
    String(String),
    Binary(Vec<u8>),
}
