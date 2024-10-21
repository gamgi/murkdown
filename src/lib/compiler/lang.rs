use std::{borrow::Cow, sync::Arc};

use super::{
    rule::{self, Context, LangInstr, LangRule},
    rule_argument::Arg,
};
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
        ctx: &'b mut Context<'a>,
        node: &'a Node,
    ) -> Result<String, LibError> {
        let mut out = String::new();
        for inst in rules {
            use Arg::*;
            match (inst.op.as_str(), inst.args.as_slice()) {
                ("PUSH", [Ref(target), Str(value)]) => {
                    let value = replace(value, ctx, node);
                    ctx.stacks
                        .entry(Arc::from(target.as_str()))
                        .or_default()
                        .push(value);
                }
                ("PUSH", [Ref(target), Ref(prop)]) => {
                    let new_value = match node.find_prop(prop) {
                        Some(value) => Some(value.to_string().into()),
                        None => ctx
                            .stacks
                            .get(prop.as_str())
                            .and_then(|v| v.last().cloned()),
                    };
                    if let Some(value) = new_value {
                        ctx.stacks
                            .raw_entry_mut()
                            .from_key(target.as_str())
                            .or_insert(Arc::from(target.as_str()), vec![])
                            .1
                            .push(value);
                    }
                }
                ("WRITE", [Str(value)]) => out.push_str(&replace(value, ctx, node)),
                _ => return Err(LibError::invalid_rule(inst.to_string())),
            }
        }
        Ok(out)
    }
}

fn replace<'a>(template: &'a str, ctx: &Context, node: &'a Node) -> Cow<'a, str> {
    if !template.contains("\\") && !template.contains("$") {
        return Cow::Borrowed(template);
    }
    let result = template
        .replace(r#"\v"#, node.value.as_deref().unwrap_or_default())
        .replace(r#"\n"#, "\n");

    Cow::Owned(result)
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

        let mut instructions = lang.get_instructions("[rule]");
        let mut ctx = Context::default();

        let value = lang.evaluate(&mut instructions, &mut ctx, &node).unwrap();
        assert_eq!(ctx.stacks.get("indent").unwrap(), &["hello", "world"]);
        assert_eq!(&value, &"ok");
    }
}
