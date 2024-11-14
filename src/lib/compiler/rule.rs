use std::collections::HashMap;
use std::fmt::Display;
use std::sync::OnceLock;
use std::{borrow::Cow, sync::Arc};

use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;
use regex::Regex;

use crate::compiler::rule_argument::Arg;
use crate::types::{LibError, RuleMap};

#[derive(Parser)]
#[grammar = "lib/compiler/rule_grammar.pest"]
struct RawRuleParser;

/// Compiler rule
#[derive(Debug, Clone)]
pub(crate) struct LangRule {
    #[allow(unused)]
    path: String,
    regex: Regex,
    pub instructions: Vec<LangInstr>,
    pub settings: LangSettings,
}

impl LangRule {
    pub fn matches(&self, path: &str) -> bool {
        self.regex.is_match(path)
    }
}

/// Language rule instruction
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

/// Language rule settings
#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub struct LangSettings {
    #[allow(unused)]
    pub is_composable: bool,
    pub is_paragraphable: bool,
    pub is_unescaped_value: bool,
    pub default_src: Option<&'static str>,
    pub default_ref: Option<&'static str>,
}

/// Context for evaluating a rule
#[derive(Debug, Clone, Default)]
pub struct Context<'a> {
    pub stacks: HashMap<Arc<str>, Vec<Cow<'a, str>>>,
}

/// Parse input to rules
pub fn parse(input: &str) -> Result<(String, String, RuleMap), LibError> {
    RawRuleParser::parse(Rule::Root, input)
        .map_err(|e| LibError::from(Box::new(e)))
        .and_then(parse_root)
}

fn parse_root<'a>(
    mut pairs: impl Iterator<Item = Pair<'a, Rule>> + 'a,
) -> Result<(String, String, RuleMap), LibError> {
    let mut name = String::new();
    let mut media_type = String::new();
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
        } else if pair.as_rule() == Rule::Preamble {
            let mut pairs = pair.into_inner();
            name = pairs.next().unwrap().as_str().to_string();
            media_type = pairs.next().unwrap().as_str().to_string();
        }
    }

    let rules = HashMap::from([("COMPILE", compile_rules), ("PREPROCESS", preprocess_rules)]);
    Ok((name, media_type, rules))
}

/// Walk pairs
fn parse_recursive<'a>(
    pairs: impl Iterator<Item = Pair<'a, Rule>>,
) -> Result<Vec<LangRule>, LibError> {
    let mut result = Vec::new();

    static DEFAULT_SCHEMAS: OnceLock<Regex> = OnceLock::new();
    let re = DEFAULT_SCHEMAS.get_or_init(|| Regex::new(r"(\w+)-BY-(\w+)").unwrap());

    for pair in pairs {
        if pair.as_rule() == Rule::Rule {
            let mut pairs = pair.into_inner().peekable();
            let path = pairs.next().unwrap().as_str().to_string();
            let settings = match pairs.peek().unwrap().as_rule() {
                Rule::Settings => {
                    let settings = pairs.next().unwrap().as_str();
                    let mut default_src = None;
                    let mut default_ref = None;

                    re.captures_iter(settings).for_each(|c| {
                        match (c.get(1).map(|k| k.as_str()), c.get(2).map(|v| v.as_str())) {
                            (Some("SRC"), Some("EXEC")) => default_src = Some("exec"),
                            (Some("SRC"), Some("COPY")) => default_src = Some("copy"),
                            (Some("REF"), Some("COPY")) => default_ref = Some("copy"),
                            (Some(key), Some(value)) => panic!("unknown default {key} {value}"),
                            (_, _) => panic!("unknown default"),
                        }
                    });
                    LangSettings {
                        is_composable: settings.contains("COMPOSABLE"),
                        is_paragraphable: settings.contains("PARAGRAPHABLE"),
                        is_unescaped_value: settings.contains("UNESCAPED_VALUE"),
                        default_src,
                        default_ref,
                    }
                }
                _ => LangSettings::default(),
            };

            let regex = Regex::new(
                &path
                    .replace('[', r"\[ ?")
                    .replace(']', r" ?\]")
                    .replace("...", r"[^]]*"),
            )?;

            let mut instructions = Vec::new();
            for mut pairs in pairs.map(Pair::into_inner) {
                let op = pairs.next().unwrap().as_str().to_string();
                let args = pairs.map(Arg::try_from).collect::<Result<_, _>>()?;
                instructions.push(LangInstr { op, args });
            }
            result.push(LangRule { path, regex, instructions, settings });
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
            RULES FOR test PRODUCE text/plain
            COMPILE RULES:
            [rule...]
              IS COMPOSABLE PARAGRAPHABLE REF-BY-COPY SRC-BY-EXEC
              PUSH foo "bar"
              WRITE "this=\"that\"\n"
            "#
        };
        let lang = Lang::new(input).unwrap();

        let compile_rules = lang.rules.get("COMPILE").unwrap();
        assert_eq!(compile_rules.len(), 1);
        let rule = &compile_rules[0];
        let expected = LangRule {
            path: "[rule...]".to_string(),
            regex: Regex::new(r#"\[ ?rule[^]]* ?\]"#).unwrap(),
            instructions: vec![
                LangInstr {
                    op: "PUSH".into(),
                    args: vec![StackRef("foo".into()), Str("bar".into())],
                },
                LangInstr {
                    op: "WRITE".into(),
                    args: vec![Str(r#"this=\"that\"\n"#.into())],
                },
            ],
            settings: LangSettings {
                is_composable: true,
                is_paragraphable: true,
                is_unescaped_value: false,
                default_src: Some("exec"),
                default_ref: Some("copy"),
            },
        };
        assert_eq!(rule.path, expected.path);
        assert_eq!(rule.instructions, expected.instructions);
        assert_eq!(rule.regex.as_str(), expected.regex.as_str());
        assert_eq!(rule.settings, expected.settings);
    }
}
