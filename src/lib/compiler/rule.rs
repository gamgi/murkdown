use std::collections::HashMap;
use std::{borrow::Cow, sync::Arc};

use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;
use regex::Regex;

use crate::ast::Node;
use crate::compiler::rule_argument::Arg;
use crate::types::LibError;

#[derive(Parser)]
#[grammar = "lib/compiler/rule_grammar.pest"]
struct RawRuleParser;

/// A compiler rule
#[derive(Debug)]
pub(crate) struct LangRule {
    instructions: Vec<RuleInstruction>,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub(crate) struct RuleInstruction {
    op: String,
    args: Vec<Arg>,
}

/// Context for evaluating a rule
#[derive(Debug, Clone, Default)]
pub struct Context<'a> {
    pub stacks: HashMap<Arc<str>, Vec<Cow<'a, str>>>,
}

/// Parse input to rules
pub fn parse(input: &str) -> Result<(Vec<LangRule>, Vec<LangRule>), LibError> {
    RawRuleParser::parse(Rule::Root, input)
        .map_err(|e| LibError::from(Box::new(e)))
        .and_then(parse_root)
}

fn parse_root<'a>(
    mut pairs: impl Iterator<Item = Pair<'a, Rule>> + 'a,
) -> Result<(Vec<LangRule>, Vec<LangRule>), LibError> {
    todo!()
}

impl LangRule {
    pub(crate) fn evaluate<'a, 'b, I>(
        &self,
        rules: &mut I,
        ctx: &'b mut Context,
        node: &'a Node,
    ) -> String
    where
        I: Iterator<Item = &'a RuleInstruction>,
    {
        todo!()
    }
}
