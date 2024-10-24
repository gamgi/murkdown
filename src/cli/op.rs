use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine as _};
use murkdown::types::URI;

use super::command::{Command, GraphType};
use super::types::AppError;

type Id = Arc<str>;

#[derive(Debug, Eq, PartialEq, Hash, Clone, Copy, Ord, PartialOrd)]
pub enum Op {
    Gather,
    Load,
    Parse,
    Preprocess,
    Compile,
    Write,
    Copy,
    Graph,
    Finish,
}

impl From<&Operation> for Op {
    fn from(other: &Operation) -> Op {
        use Operation::*;
        match other {
            Gather { .. } => Op::Gather,
            Load { .. } => Op::Load,
            Parse { .. } => Op::Parse,
            Preprocess { .. } => Op::Preprocess,
            Compile { .. } => Op::Compile,
            Write { .. } => Op::Write,
            Copy { .. } => Op::Copy,
            Graph { .. } => Op::Graph,
            Finish => Op::Finish,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Operation {
    Gather {
        cmd: Command,
        paths: Vec<PathBuf>,
        #[allow(dead_code)]
        splits: Option<Vec<String>>,
    },
    Load {
        id: Id,
        path: PathBuf,
    },
    Parse {
        id: Id,
    },
    Preprocess {
        id: Id,
    },
    Compile {
        id: Id,
    },
    Write {
        id: Id,
    },
    Copy {
        id: Id,
        path: PathBuf,
    },
    Finish,
    Graph {
        graph_type: GraphType,
    },
}

impl Operation {
    pub fn uri(&self) -> String {
        OpId::from(self).uri()
    }
}

impl Display for Operation {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Operation::Gather { .. } => write!(f, "Gather"),
            Operation::Load { id, .. } => write!(f, "Load {}", id),
            Operation::Parse { id, .. } => write!(f, "Parse {}", id),
            Operation::Preprocess { id, .. } => write!(f, "Preprocess {}", id),
            Operation::Compile { id, .. } => write!(f, "Compile {}", id),
            Operation::Write { id, .. } => write!(f, "Write {}", id),
            Operation::Copy { id, .. } => write!(f, "Copy {}", id),
            Operation::Graph { .. } => write!(f, "Graph"),
            Operation::Finish => write!(f, "Finish"),
        }
    }
}

#[derive(Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Clone)]
pub struct OpId(Op, Id);

impl OpId {
    pub fn gather() -> Self {
        Self(Op::Gather, Arc::from("Gather"))
    }

    pub fn load(id: impl Into<Arc<str>>) -> Self {
        Self(Op::Load, id.into())
    }

    pub fn parse(id: impl Into<Arc<str>>) -> Self {
        Self(Op::Parse, id.into())
    }

    pub fn preprocess(id: impl Into<Arc<str>>) -> Self {
        Self(Op::Preprocess, id.into())
    }

    pub fn copy(id: impl Into<Arc<str>>) -> Self {
        Self(Op::Copy, id.into())
    }

    pub fn finish() -> Self {
        Self(Op::Finish, Arc::from("Finish"))
    }

    pub fn graph() -> Self {
        Self(Op::Graph, Arc::from("Graph"))
    }

    pub fn uri(&self) -> URI {
        match self.0 {
            Op::Gather => String::from("gather:"),
            Op::Load => format!("file:{}", self.1),
            Op::Parse => format!("ast:{}", self.1),
            Op::Preprocess => format!("parse:{}", self.1),
            Op::Compile => format!("compile:{}", self.1),
            Op::Write => format!("write:{}", self.1),
            Op::Copy => format!("copy:{}", self.1),
            Op::Graph => String::from("graph:"),
            Op::Finish => String::from("finish:"),
        }
    }

    pub fn uid(&self) -> String {
        STANDARD_NO_PAD.encode(self.uri())
    }
}

impl Default for OpId {
    fn default() -> Self {
        OpId::finish()
    }
}

impl From<&Operation> for OpId {
    fn from(other: &Operation) -> OpId {
        use Operation::*;
        match other {
            Gather { .. } => OpId::gather(),
            Load { id, .. }
            | Parse { id, .. }
            | Preprocess { id }
            | Compile { id }
            | Write { id }
            | Copy { id, .. } => OpId(other.into(), id.clone()),
            Graph { .. } => OpId::graph(),
            Finish => OpId::finish(),
        }
    }
}

impl FromStr for OpId {
    type Err = crate::cli::types::AppError;

    fn from_str(other: &str) -> Result<Self, Self::Err> {
        let (schema, path) = other.split_once(':').ok_or(AppError::bad_uri(other))?;
        let op = match schema {
            "file" => Op::Load,
            "ast" => Op::Parse,
            "parse" => Op::Preprocess,
            _ => return Err(AppError::unknown_schema(schema)),
        };
        Ok(Self(op, Arc::from(path)))
    }
}
