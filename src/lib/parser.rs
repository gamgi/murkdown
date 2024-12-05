use std::{iter::Peekable, sync::Arc};

use pest::{
    error::Error as PestError,
    error::ErrorVariant,
    iterators::{Pair, Pairs},
    Parser, Position,
};
use pest_derive::Parser;

use crate::{
    ast::{Node, NodeBuilder, Props},
    types::LibError,
};

#[derive(Parser)]
#[grammar = "lib/parser_grammar.pest"]
struct RawParser;

#[allow(clippy::derivable_impls)]
impl Default for Rule {
    fn default() -> Self {
        Rule::RootA
    }
}

/// Parse input to AST
pub fn parse(input: &str) -> Result<Node, LibError> {
    RawParser::parse(Rule::RootB, input)
        .and_then(parse_root)
        .map_err(|e| LibError::from(Box::new(e)))
}

#[allow(clippy::result_large_err)]
fn parse_root<'a>(
    pairs: impl Iterator<Item = Pair<'a, Rule>> + 'a,
) -> Result<Node, PestError<Rule>> {
    match parse_recursive(pairs).next() {
        Some(r) => Ok(r),
        None => Err(PestError::new_from_pos(
            ErrorVariant::CustomError { message: "no root found".into() },
            Position::from_start(""),
        )),
    }
}

/// Walk pairs and build children
fn parse_recursive<'a>(
    pairs: impl Iterator<Item = Pair<'a, Rule>> + 'a,
) -> impl Iterator<Item = Node> + 'a {
    pairs.filter_map(|pair| match pair.as_rule() {
        Rule::EOI => None,
        Rule::RootA | Rule::RootB | Rule::Block => {
            let base = NodeBuilder::from(&pair);
            let mut pairs = pair.into_inner().peekable();
            let marker = take_marker(&mut pairs);
            let headers = take_headers(&mut pairs);
            let props = take_props(&mut pairs);
            let sections = parse_recursive(pairs);
            let node = base
                .marker(marker)
                .headers(headers)
                .add_children(sections)
                .try_props(props);
            Some(node.build().unwrap())
        }
        Rule::RootBlock
        | Rule::RootBlockB
        | Rule::LongBlock
        | Rule::LongBlockB
        | Rule::ShortBlock => {
            let base = NodeBuilder::new(Rule::Section);
            let mut pairs = pair.into_inner().peekable();
            let _ = take_marker(&mut pairs);
            let headers = take_headers(&mut pairs);
            let props = take_props(&mut pairs);
            let children = parse_recursive(pairs);
            let node = base
                .headers(headers)
                .add_children(children)
                .try_props(props);
            Some(node.build().unwrap())
        }
        _ => Some(Node::new(&pair)),
    })
}

fn take_marker(pairs: &mut Peekable<Pairs<'_, Rule>>) -> Option<Arc<str>> {
    pairs
        .next_if(|p| matches!(p.as_rule(), Rule::BLOCK_START | Rule::BLOCK_START_B))
        .map(|p| p.as_str().into())
}

fn take_headers(pairs: &mut Peekable<Pairs<'_, Rule>>) -> Option<Vec<Arc<str>>> {
    pairs
        .next_if(|p| matches!(p.as_rule(), Rule::BLOCK_HEADER | Rule::SECT_HEADER))
        .map(|p| p.as_str())
        .map(|p| p.split_ascii_whitespace().map(Arc::from).collect())
}

fn take_props(pairs: &mut Peekable<Pairs<'_, Rule>>) -> Result<Option<Props>, ()> {
    pairs
        .next_if(|p| matches!(p.as_rule(), Rule::BLOCK_PROPS))
        .map(|p| RawParser::parse(Rule::BlockProps, p.as_str()))
        .map(|p| match p {
            Ok(pairs) => {
                let props = pairs
                    .map(|pair| {
                        let mut tokens = pair.into_inner();
                        let key = Arc::from(tokens.next().unwrap().as_str());
                        let val = Arc::from(tokens.next().unwrap().as_str());
                        (key, val)
                    })
                    .collect();
                Ok(props)
            }
            _ => Err(()),
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_parse_short_block() {
        let input = indoc! {
            r#"
            foo
            > [!IMAGE](src="foo.png") bar
            baz
            "#
        };
        let expected = NodeBuilder::root()
            .add_section(vec![
                Node::line("foo"),
                NodeBuilder::block(">")
                    .add_prop(("src".into(), "foo.png".into()))
                    .headers(Some(vec!["IMAGE".into()]))
                    .add_section(vec![Node::line("bar")])
                    .done(),
                Node::line("baz"),
            ])
            .done();
        let ast = parse(input).unwrap();

        assert_eq!(ast, expected);
    }

    #[test]
    fn test_parse_long_block() {
        let input = indoc! {
            r#"
            foo
            > [!IMAGE](src="foo.png")
            > bar
            > baz
            qux
            "#
        };
        let expected = NodeBuilder::root()
            .add_section(vec![
                Node::line("foo"),
                NodeBuilder::block(">")
                    .add_prop(("src".into(), "foo.png".into()))
                    .headers(Some(vec!["IMAGE".into()]))
                    .add_section(vec![Node::line("bar"), Node::line("baz")])
                    .done(),
                Node::line("qux"),
            ])
            .done();
        let ast = parse(input).unwrap();

        assert_eq!(ast, expected);
    }
}
