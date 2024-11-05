use std::{fmt::Write as FmtWrite, sync::Arc};

use derive_builder::Builder;
use pest::iterators::Pair;

use crate::{parser::Rule, types::Pointer};

pub(crate) type Props = Vec<(Arc<str>, Arc<str>)>;

/// AST Node
#[derive(Builder, Clone, Debug, Default, PartialEq, Eq)]
#[builder(pattern = "owned", default, derive(Clone, Debug, PartialEq, Eq))]
pub struct Node {
    #[builder(setter(strip_option))]
    pub rule: Rule,
    #[builder(setter(strip_option, into, each(name = "add_prop")))]
    pub props: Option<Props>,
    #[builder(setter(into))]
    pub value: Option<Arc<str>>,
    #[builder(setter(into))]
    pub marker: Option<Arc<str>>,
    pub headers: Option<Vec<Arc<str>>>,
    #[builder(setter)]
    pub pointer: Option<Pointer>,
    #[builder(setter(strip_option, each(name = "add_child")))]
    pub children: Option<Vec<Node>>,
    #[builder(setter(strip_option, each(name = "add_error")))]
    pub errors: Option<Vec<&'static str>>,
}

impl Node {
    pub fn new(pair: &Pair<Rule>) -> Self {
        NodeBuilder::from(pair).build().unwrap()
    }

    pub fn new_line(value: &str) -> Self {
        NodeBuilder::new(Rule::Line)
            .value(Arc::from(value))
            .build()
            .unwrap()
    }

    #[cfg(test)]
    pub fn paragraph(values: &[&str]) -> Self {
        NodeBuilder::new(Rule::Paragraph)
            .value(Arc::from(values.join("\n")))
            .children(values.iter().map(|v| Node::new_line(v)).collect())
            .build()
            .unwrap()
    }

    pub fn ellipsis() -> Self {
        NodeBuilder::new(Rule::Ellipsis).build().unwrap()
    }

    pub fn add_prop(&mut self, key: &str, value: Arc<str>) {
        let entry = (Arc::from(key), value);
        match self.props.as_mut() {
            Some(props) => props.push(entry),
            None => self.props = Some(vec![entry]),
        };
    }

    pub fn find_prop(&self, key: &str) -> Option<Arc<str>> {
        match self.props.as_ref() {
            Some(props) => props
                .iter()
                .find_map(|(k, v)| if &**k == key { Some(v.clone()) } else { None }),
            None => None,
        }
    }

    /// Build AST path
    pub fn build_path(&self, prefix: &str) -> String {
        let headers = self.headers.as_ref().map_or(String::new(), |h| h.join(" "));
        let mut result = match prefix.is_empty() {
            true => String::new(),
            false => format!("{prefix} "),
        };
        let _ = match self.rule {
            Rule::Root => write!(&mut result, "[{}]", headers),
            Rule::Block => write!(&mut result, "[{}]", headers),
            Rule::Line => write!(&mut result, "LINE"),
            Rule::Section if !headers.is_empty() => write!(&mut result, "[SEC {headers}]"),
            Rule::Section => write!(&mut result, "[SEC]"),
            Rule::Paragraph => write!(&mut result, "[PAR]"),
            _ => write!(&mut result, "?"),
        };
        result
    }
}

impl NodeBuilder {
    pub fn new(rule: Rule) -> Self {
        Self::default().rule(rule)
    }

    pub fn done(self) -> Node {
        self.build().unwrap()
    }

    pub fn root() -> Self {
        Self::new(Rule::Root)
    }

    pub fn block(marker: &'static str) -> Self {
        Self::new(Rule::Block).marker(Arc::from(marker))
    }

    pub fn add_section(self, children: Vec<Node>) -> Self {
        let node = NodeBuilder::new(Rule::Section)
            .add_children(children)
            .build()
            .unwrap();
        self.add_child(node)
    }

    pub fn add_children(self, children: impl IntoIterator<Item = Node>) -> Self {
        self.children(children.into_iter().collect())
    }

    pub fn try_props<T>(self, props: Result<Option<Props>, T>) -> Self {
        match props {
            Ok(Some(props)) => self.props(props),
            Ok(None) => self,
            Err(_) => self.add_error("invalid props"),
        }
    }
}

impl From<&Pair<'_, Rule>> for NodeBuilder {
    fn from(pair: &Pair<Rule>) -> Self {
        let rule = match pair.as_rule() {
            Rule::LineEOI => Rule::Line,
            r => r,
        };
        let is_line = matches!(rule, Rule::Line);
        let is_block = matches!(rule, Rule::Block | Rule::Root);
        match pair.as_span().as_str() {
            "" if !is_line => NodeBuilder::new(rule),
            value if is_block => NodeBuilder::new(rule).marker(Some(Arc::from(value))),
            value => NodeBuilder::new(rule).value(Arc::from(value)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::NodeBuilder;

    #[test]
    fn test_build_section_path_with_headers() {
        let node = NodeBuilder::new(Rule::Section)
            .headers(Some(vec![Arc::from("BAR")]))
            .done();
        assert_eq!(node.build_path("[FOO]"), "[FOO] [SEC BAR]");
    }

    #[test]
    fn test_build_section_path_without_headers() {
        let node = NodeBuilder::new(Rule::Section).done();
        assert_eq!(node.build_path("[FOO]"), "[FOO] [SEC]");
    }
}
