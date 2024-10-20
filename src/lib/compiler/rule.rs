use std::collections::HashMap;
use std::{borrow::Cow, sync::Arc};

use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "lib/compiler/rule_grammar.pest"]
struct RawRuleParser;

/// A compiler rule
pub struct LangRule {}

/// Context for evaluating a rule
#[derive(Debug, Clone, Default)]
pub struct Context<'a> {
    pub stacks: HashMap<Arc<str>, Vec<Cow<'a, str>>>,
}

/// Parse input to rules
pub fn parse(input: String) -> Vec<LangRule> {
    let _ = RawRuleParser::parse(Rule::Root, &input);
    vec![]
}
