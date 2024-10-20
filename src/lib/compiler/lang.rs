use super::rule::{self, LangRule};

pub struct Lang {
    pub rules: Vec<LangRule>,
}

/// A set of compiler rules
impl Lang {
    pub fn new(input: &'static str) -> Lang {
        let rules = rule::parse(input.to_string());

        Lang { rules }
    }
}
