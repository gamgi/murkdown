use super::rule::{self, Context, LangInstr, LangRule};
use crate::{ast::Node, types::LibError};

pub struct Lang {
    pub preprocess_rules: Vec<LangRule>,
    pub compile_rules: Vec<LangRule>,
}

/// A set of compiler rules
impl Lang {
    pub fn new(input: &'static str) -> Result<Lang, LibError> {
        let (preprocess_rules, compile_rules) = rule::parse(input)?;

        Ok(Lang { preprocess_rules, compile_rules })
    }

    /// Get instructions for an AST path
    pub(crate) fn get_instructions(&self, path: &str) -> impl Iterator<Item = &LangInstr> {
        match self.compile_rules.iter().find(|r| r.matches(path)) {
            Some(rule) => rule.instructions.iter(),
            None => (&[] as &[LangInstr]).iter(),
        }
    }

    pub(crate) fn evaluate<'a, 'b>(
        &self,
        rules: &'b mut impl Iterator<Item = &'a LangInstr>,
        ctx: &'b mut Context,
        node: &'a Node,
    ) -> String {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use indoc::indoc;

    use super::*;

    #[test]
    fn test_evaluate() {
        let input = indoc! {
            r#"
            [COMPILE]
            [rule]
              PUSH foo "hello"
              PUSH indent foo
              PUSH indent "world"
              WRITE "ok"
            "#
        };
        let lang = Lang::new(input).unwrap();
        let node = Node::default();

        let mut rules = lang.get_instructions("[rule]");
        let mut ctx = Context::default();

        let _ = lang.evaluate(&mut rules, &mut ctx, &node);
    }
}
