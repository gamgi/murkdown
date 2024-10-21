use pest::iterators::Pair;

use super::Rule;
use crate::types::LibError;

/// Argument
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Arg {
    Str(String),
    Int(i64),
    Ref(String),
}

impl TryFrom<Pair<'_, Rule>> for Arg {
    type Error = LibError;

    fn try_from(pair: Pair<'_, Rule>) -> Result<Self, Self::Error> {
        match pair.as_rule() {
            Rule::Str => Ok(Arg::Str(pair.as_str().to_string())),
            Rule::Int => {
                Ok(Arg::Int(pair.as_str().parse::<i64>().map_err(|_| {
                    LibError::invalid_rule_argument(pair.as_str())
                })?))
            }
            Rule::Ref => Ok(Arg::Ref(pair.as_str().to_string())),
            _ => return Err(LibError::invalid_rule_argument(pair.as_str())),
        }
    }
}

impl PartialEq<&str> for Arg {
    fn eq(&self, other: &&str) -> bool {
        match self {
            Arg::Str(s) => s == *other,
            Arg::Int(s) => s.to_string() == *other,
            Arg::Ref(s) => s == *other,
        }
    }
}
