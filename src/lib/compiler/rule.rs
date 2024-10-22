use std::collections::HashMap;
use std::fmt::Display;
use std::{borrow::Cow, sync::Arc};

use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;
use regex::Regex;

use crate::compiler::rule_argument::Arg;
use crate::types::LibError;

#[derive(Parser)]
#[grammar = "lib/compiler/rule_grammar.pest"]
struct RawRuleParser;

/// Compiler rule
#[derive(Debug)]
pub(crate) struct LangRule {
    pub path: String,
    regex: Regex,
    pub instructions: Vec<LangInstr>,
    is_composable: bool,
}

impl LangRule {
    pub fn matches(&self, path: &str) -> bool {
        self.regex.is_match(path)
    }
}

/// Compiler instruction
#[derive(Debug, Eq, PartialEq, Clone)]
pub(crate) struct LangInstr {
    pub op: String,
    pub args: Vec<Arg>,
}

impl Display for LangInstr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.op)?;
        if !self.args.is_empty() {
            write!(f, " ")?;
        }
        let args = self.args.iter().map(|a| a.to_string()).collect::<Vec<_>>();
        write!(f, "{}", args.join(" "))
    }
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
    let mut compile_rules = Vec::new();
    let mut preprocess_rules = Vec::new();

    let pairs = pairs.next().unwrap().into_inner();
    for pair in pairs {
        if pair.as_rule() == Rule::Section {
            let mut pairs = pair.into_inner();
            match pairs.next().unwrap().as_str() {
                "COMPILE" => compile_rules.extend(parse_recursive(pairs)?),
                "PREPROCESS" => preprocess_rules.extend(parse_recursive(pairs)?),
                section => return Err(LibError::unknown_rule_section(section)),
            }
        }
    }

    Ok((preprocess_rules, compile_rules))
}

/// Walk pairs
fn parse_recursive<'a>(
    pairs: impl Iterator<Item = Pair<'a, Rule>>,
) -> Result<Vec<LangRule>, LibError> {
    let mut result = Vec::new();
    for pair in pairs {
        if pair.as_rule() == Rule::Rule {
            let mut pairs = pair.into_inner().peekable();
            let path = pairs.next().unwrap().as_str().to_string();
            let is_composable = match pairs.peek().unwrap().as_rule() {
                Rule::Settings => pairs.next().unwrap().as_str().contains("COMPOSABLE"),
                _ => false,
            };

            let regex = Regex::new(
                &path
                    .replace('[', r"\[")
                    .replace(']', r"\]")
                    .replace("...", r"[^]]*"),
            )?;

            let mut instructions = Vec::new();
            for mut pairs in pairs.map(Pair::into_inner) {
                let op = pairs.next().unwrap().as_str().to_string();
                let args = pairs.map(Arg::try_from).collect::<Result<_, _>>()?;
                instructions.push(LangInstr { op, args });
            }
            result.push(LangRule { path, regex, instructions, is_composable });
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use pretty_assertions::assert_eq;
    use Arg::*;

    use super::*;
    use crate::compiler::lang::Lang;

    #[test]
    fn test_parse_rule() {
        let input = indoc! {
            r#"
            [COMPILE]
            [rule...]
              IS COMPOSABLE
              PUSH foo "bar"
            "#
        };
        let lang = Lang::new(input).unwrap();

        assert_eq!(lang.compile_rules.len(), 1);
        let rule = &lang.compile_rules[0];
        let expected = LangRule {
            path: "[rule...]".to_string(),
            regex: Regex::new(r#"\[rule[^]]*\]"#).unwrap(),
            instructions: vec![LangInstr {
                op: "PUSH".into(),
                args: vec![StackRef("foo".into()), Str("bar".into())],
            }],
            is_composable: true,
        };
        assert_eq!(rule.path, expected.path);
        assert_eq!(rule.instructions, expected.instructions);
        assert_eq!(rule.regex.as_str(), expected.regex.as_str());
        assert_eq!(rule.is_composable, expected.is_composable);
    }
}
