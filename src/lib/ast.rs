use std::collections::HashMap;

use derive_builder::Builder;
use pest::iterators::Pair;

use crate::parser::Rule;
pub type Props = HashMap<String, String>;

/// AST Node
#[derive(Builder, Clone, Debug, Default, PartialEq, Eq)]
#[builder(pattern = "owned", default, derive(Clone, Debug, PartialEq, Eq))]
pub struct Node {
    #[builder(setter(strip_option))]
    pub rule: Rule,
    #[builder(setter(into))]
    pub value: Option<String>,
    #[builder(setter(strip_option, each(name = "add_child")))]
    pub children: Option<Vec<Node>>,
}

impl Node {
    pub fn new(pair: &Pair<Rule>) -> Self {
        NodeBuilder::from(pair).build().unwrap()
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

    pub fn add_children(self, children: impl IntoIterator<Item = Node>) -> Self {
        self.children(children.into_iter().collect())
    }
}

impl From<&Pair<'_, Rule>> for NodeBuilder {
    fn from(pair: &Pair<Rule>) -> Self {
        let rule = pair.as_rule();
        NodeBuilder::new(rule)
    }
}
