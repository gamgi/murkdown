use pest::{error::ErrorVariant, iterators::Pair, Parser as PestParser, Position};
use pest_derive::Parser;

use crate::{
    ast::{Node, NodeBuilder},
    types::ParseError,
};

#[derive(Parser)]
#[grammar = "lib/parser_grammar.pest"]
struct RawParser;

impl Default for Rule {
    fn default() -> Self {
        Rule::Root
    }
}

/// Parse input to AST
pub fn parse(input: &str) -> Result<Node, Box<ParseError>> {
    RawParser::parse(Rule::Root, input)
        .and_then(parse_root)
        .map_err(Box::new)
}

#[allow(clippy::result_large_err)]
fn parse_root<'a>(pairs: impl Iterator<Item = Pair<'a, Rule>> + 'a) -> Result<Node, ParseError> {
    match parse_pairs(pairs).next() {
        Some(r) => Ok(r),
        None => Err(ParseError::new_from_pos(
            ErrorVariant::CustomError { message: String::from("no root found") },
            Position::from_start(""),
        )),
    }
}

/// Walk pairs and build children
fn parse_pairs<'a>(
    pairs: impl Iterator<Item = Pair<'a, Rule>> + 'a,
) -> impl Iterator<Item = Node> + 'a {
    pairs.filter_map(|pair| match pair.as_rule() {
        Rule::EOI => None,
        Rule::Root => {
            let base = NodeBuilder::from(&pair);
            let pairs = pair.into_inner();
            let sections = parse_pairs(pairs);
            let node = base.add_children(sections);
            Some(node.build().unwrap())
        }
        _ => Some(Node::new(&pair)),
    })
}
