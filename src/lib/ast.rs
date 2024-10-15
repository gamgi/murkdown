use std::collections::HashMap;

use derive_builder::Builder;

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
}
