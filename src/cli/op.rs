use std::fmt::{self, Display, Formatter};
use std::hash::Hash;
use std::path::PathBuf;
use std::sync::Arc;

type Id = Arc<str>;

#[derive(Debug, Eq, PartialEq, Hash, Clone, Copy, Ord, PartialOrd)]
pub enum Op {
    Load,
    Finish,
}

#[derive(Debug, Clone)]
pub enum Operation {
    Load { id: Id, path: PathBuf },
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
            Operation::Load { id, path } => write!(f, "Load {}", id),
            Operation::Finish => write!(f, "Finish"),
        }
    }
}

#[derive(Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Clone)]
pub struct OpId(Op, Id);

impl OpId {
    pub fn load(id: impl Into<Arc<str>>) -> Self {
        Self(Op::Load, id.into())
    }

    pub fn finish() -> Self {
        Self(Op::Finish, Arc::from("Finish"))
    }

    pub fn uri(&self) -> String {
        match self.0 {
            Op::Load => format!("load:{}", self.1),
            Op::Finish => unreachable!(),
        }
    }
}

impl Default for OpId {
    fn default() -> Self {
        OpId::finish()
    }
}

impl From<&Operation> for Op {
    fn from(other: &Operation) -> Op {
        use Operation::*;
        match other {
            Load { .. } => Op::Load,
            Finish => Op::Finish,
        }
    }
}

impl From<&Operation> for OpId {
    fn from(other: &Operation) -> OpId {
        use Operation::*;
        match other {
            Load { id, .. } => OpId(other.into(), id.clone()),
            Finish => OpId::default(),
        }
    }
}