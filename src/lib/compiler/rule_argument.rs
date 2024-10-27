use std::fmt::Display;

use pest::iterators::Pair;

use super::Rule;
use crate::types::LibError;

/// Argument
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Arg {
    Memory,
    Int(i64),
    URIPath(String),
    PropRef(String),
    Str(String),
    StackRef(String),
}

impl Display for Arg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Arg::Memory => write!(f, "TO STRING"),
            Arg::Int(v) => write!(f, "{}", v),
            Arg::URIPath(v) => write!(f, "AS \"{}\"", v),
            Arg::PropRef(v) => write!(f, "PROP {}", v),
            Arg::Str(v) => write!(f, "\"{}\"", v),
            Arg::StackRef(v) => write!(f, "{}", v),
        }
    }
}

impl TryFrom<Pair<'_, Rule>> for Arg {
    type Error = LibError;

    fn try_from(pair: Pair<'_, Rule>) -> Result<Self, Self::Error> {
        match pair.as_rule() {
            Rule::Str => Ok(Arg::Str(pair.as_str().to_string())),
            Rule::Int => Ok(Arg::Int(pair.as_str().parse::<i64>().map_err(|_| {
                LibError::invalid_rule_argument_type(pair.as_str(), "int")
            })?)),
            Rule::StackRef => Ok(Arg::StackRef(pair.as_str().to_string())),
            Rule::PropRef => Ok(Arg::PropRef(pair.as_str().to_string())),
            Rule::URIPath => Ok(Arg::URIPath(pair.as_str().to_string())),
            Rule::Memory => Ok(Arg::Memory),
            _ => return Err(LibError::invalid_rule_argument(pair.as_str())),
        }
    }
}

impl PartialEq<&str> for Arg {
    fn eq(&self, other: &&str) -> bool {
        match self {
            Arg::URIPath(s) | Arg::Str(s) | Arg::PropRef(s) | Arg::StackRef(s) => s == *other,
            Arg::Memory => false,
            Arg::Int(s) => s.to_string() == *other,
        }
    }
}
