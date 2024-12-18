use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine as _};
use murkdown::types::{ExecArtifact, ExecInput, URI};

use super::command::{Command, GraphType};
use super::types::{AppError, Source};

type Id = Arc<str>;

#[derive(Debug, Eq, PartialEq, Hash, Clone, Copy, Ord, PartialOrd)]
pub enum Op {
    Gather,
    Exec,
    Load,
    Tangle,
    Parse,
    Preprocess,
    Compile,
    CompilePlaintext,
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
            Exec { .. } => Op::Exec,
            Load { .. } => Op::Load,
            Tangle { .. } => Op::Tangle,
            Parse { .. } => Op::Parse,
            Preprocess { .. } => Op::Preprocess,
            Compile { .. } => Op::Compile,
            CompilePlaintext { .. } => Op::CompilePlaintext,
            Write { .. } => Op::Write,
            Copy { .. } => Op::Copy,
            Graph { .. } => Op::Graph,
            Finish => Op::Finish,
        }
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(Ord, PartialOrd, Eq, PartialEq))]
pub enum Operation {
    Gather {
        cmd: Command,
        sources: Vec<Source>,
        #[allow(dead_code)]
        splits: Option<Vec<String>>,
    },
    Exec {
        id: Id,
        cmd: String,
        input: Option<ExecInput>,
        artifact: ExecArtifact,
    },
    Load {
        id: Id,
        source: Source,
    },
    Tangle {
        id: Id,
        target: PathBuf,
    },
    Parse {
        id: Id,
    },
    Preprocess {
        id: Id,
        headers: Option<String>,
    },
    Compile {
        id: Id,
    },
    CompilePlaintext {
        id: Id,
        source_uri: URI,
    },
    Write {
        id: Id,
    },
    Copy {
        id: Id,
        source: Source,
    },
    Finish,
    Graph {
        graph_type: GraphType,
    },
}

impl Operation {
    pub fn uri(&self) -> URI {
        OpId::from(self).uri()
    }

    pub fn uri_path(&self) -> URI {
        OpId::from(self).uri_path()
    }
}

impl Display for Operation {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Operation::Gather { .. } => write!(f, "Gather"),
            Operation::Exec { id, .. } => write!(f, "Exec {}", id),
            Operation::Load { id, .. } => write!(f, "Load {}", id),
            Operation::Tangle { id, .. } => write!(f, "Tangle {}", id),
            Operation::Parse { id, .. } => write!(f, "Parse {}", id),
            Operation::Preprocess { id, .. } => write!(f, "Preprocess {}", id),
            Operation::CompilePlaintext { id, .. } => write!(f, "Compile plaintext {}", id),
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

    pub fn exec(id: impl Into<Arc<str>>) -> Self {
        Self(Op::Exec, id.into())
    }

    #[cfg(test)]
    pub fn load(id: impl Into<Arc<str>>) -> Self {
        Self(Op::Load, id.into())
    }

    #[cfg(test)]
    pub fn parse(id: impl Into<Arc<str>>) -> Self {
        Self(Op::Parse, id.into())
    }

    #[cfg(test)]
    pub fn preprocess(id: impl Into<Arc<str>>) -> Self {
        Self(Op::Preprocess, id.into())
    }

    #[cfg(test)]
    pub fn compile(id: impl Into<Arc<str>>) -> Self {
        Self(Op::Compile, id.into())
    }

    #[cfg(test)]
    pub fn copy(id: impl Into<Arc<str>>) -> Self {
        Self(Op::Copy, id.into())
    }

    pub fn write(id: impl Into<Arc<str>>) -> Self {
        Self(Op::Write, id.into())
    }

    pub fn finish() -> Self {
        Self(Op::Finish, Arc::from("Finish"))
    }

    pub fn graph(id: impl Into<Arc<str>>) -> Self {
        Self(Op::Graph, id.into())
    }

    pub fn uri(&self) -> URI {
        match self.0 {
            Op::Gather => String::from("gather:"),
            Op::Exec => format!("exec:{}", self.1),
            Op::Load => format!("file:{}", self.1),
            Op::Tangle => format!("tangle:{}", self.1),
            Op::Parse => format!("ast:{}", self.1),
            Op::Preprocess => format!("parse:{}", self.1),
            Op::CompilePlaintext => format!("compileplain:{}", self.1),
            Op::Compile => format!("compile:{}", self.1),
            Op::Write => format!("write:{}", self.1),
            Op::Copy => format!("copy:{}", self.1),
            Op::Graph => format!("graph:{}", self.1),
            Op::Finish => String::from("finish:"),
        }
    }

    pub fn uri_path(&self) -> String {
        match self.0 {
            Op::Finish | Op::Gather => String::new(),
            _ => self.1.to_string(),
        }
    }

    pub fn uid(&self) -> String {
        STANDARD_NO_PAD.encode(self.uri())
    }

    pub fn is_hidden(&self) -> bool {
        matches!(self.0, Op::Graph)
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
            | Tangle { id, .. }
            | Exec { id, .. }
            | Parse { id, .. }
            | Preprocess { id, .. }
            | Compile { id, .. }
            | CompilePlaintext { id, .. }
            | Write { id }
            | Copy { id, .. } => OpId(other.into(), id.clone()),
            Graph { graph_type } => OpId::graph(graph_type.to_string()),
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
            "exec" => Op::Exec,
            "copy" => Op::Copy,
            _ => return Err(AppError::unknown_schema(schema)),
        };
        Ok(Self(op, Arc::from(path)))
    }
}
