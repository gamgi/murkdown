use super::rule::{self, LangRule};
use crate::types::LibError;

pub struct Lang {
    pub preprocess_rules: Vec<LangRule>,
    pub compile_rules: Vec<LangRule>,
}

/// A set of compiler rules
impl Lang {
    pub fn new(input: &'static str) -> Result<Lang, LibError> {
        let (preprocess_rules, compile_rules) = rule::parse(input)?;

        Ok(Lang { preprocess_rules, compile_rules })
    }
}
