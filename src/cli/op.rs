use std::fmt::{self, Display, Formatter};
use std::hash::Hash;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use murkdown::types::URI;

use super::command::Command;
use super::types::AppError;

type Id = Arc<str>;

#[derive(Debug, Eq, PartialEq, Hash, Clone, Copy, Ord, PartialOrd)]
pub enum Op {
    Gather,
    Load,
    Parse,
    Finish,
}

impl From<&Operation> for Op {
    fn from(other: &Operation) -> Op {
        use Operation::*;
        match other {
            Gather { .. } => Op::Gather,
            Load { .. } => Op::Load,
            Parse { .. } => Op::Parse,
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
    Finish,
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

    pub fn finish() -> Self {
        Self(Op::Finish, Arc::from("Finish"))
    }

    pub fn uri(&self) -> URI {
        match self.0 {
            Op::Gather => String::from("gather:*"),
            Op::Load => format!("raw:{}", self.1),
            Op::Parse => format!("ast:{}", self.1),
            Op::Finish => unreachable!(),
        }
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
            Load { id, .. } | Parse { id, .. } => OpId(other.into(), id.clone()),
            Finish => OpId::finish(),
        }
    }
}

impl FromStr for OpId {
    type Err = crate::cli::types::AppError;

    fn from_str(other: &str) -> Result<Self, Self::Err> {
        let (schema, path) = other.split_once(':').ok_or(AppError::bad_uri(other))?;
        let op = match schema {
            "raw" => Op::Load,
            "raw-ast" => Op::Parse,
            _ => todo!(),
        };
        Ok(Self(op, Arc::from(path)))
    }
}
