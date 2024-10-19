use std::sync::Arc;

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
    #[builder(setter(strip_option))]
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

    pub fn block(_marker: &'static str) -> Self {
        Self::new(Rule::Block)
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
        let rule = pair.as_rule();
        NodeBuilder::new(rule)
    }
}
