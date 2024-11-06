use std::{borrow::Cow, collections::HashSet, sync::Arc};

use htmlize::escape_text;

use super::{
    rule::{self, Context, LangInstr, LangSettings},
    rule_argument::Arg,
};
use crate::{
    ast::Node,
    types::{Dependency, ExecArtifact, LibError, RuleMap},
};

#[derive(Debug, Clone)]
pub struct Lang {
    pub(crate) rules: RuleMap,
}

/// A set of compiler rules
impl Lang {
    pub fn new(input: &str) -> Result<Lang, LibError> {
        let rules = rule::parse(input)?;

        Ok(Lang { rules })
    }

    /// Get instructions for an AST path
    pub(crate) fn get_instructions(
        &self,
        stage: &'static str,
        path: &str,
    ) -> (impl Iterator<Item = &LangInstr>, LangSettings) {
        let rules = self.rules.get(stage);
        match rules.unwrap().iter().find(|r| r.matches(path)) {
            Some(rule) => (rule.instructions.iter(), rule.settings),
            None => ((&[] as &[LangInstr]).iter(), LangSettings::default()),
        }
    }

    /// Evaluate instructions to apply mutations and produce output
    pub(crate) fn evaluate<'a, 'b, 'c>(
        &self,
        instructions: &'b mut impl Iterator<Item = &'a LangInstr>,
        ctx: &'b mut Context<'a>,
        deps: &mut HashSet<Dependency>,
        node: &'c mut Node,
        set: &LangSettings,
    ) -> Result<String, LibError> {
        let mut out = String::new();
        for inst in instructions {
            use Arg::*;
            match (inst.op.as_str(), inst.args.as_slice()) {
                ("EXEC", [Str(cmd), destination @ (MediaType(_) | File(_)), URIPath(id)]) => {
                    let cmd = replace(cmd, ctx, &*node, set).to_string();
                    let id = replace(id, ctx, &*node, set).to_string();
                    let artifact = match destination {
                        MediaType(t) => {
                            ExecArtifact::Stdout(replace(t, ctx, &*node, set).to_string())
                        }
                        File(p) => ExecArtifact::Path(replace(p, ctx, &*node, set).as_ref().into()),
                        _ => unreachable!(),
                    };
                    let input = node.children.as_ref().map(|children| {
                        children
                            .iter()
                            .filter_map(|n| n.value.clone())
                            .collect::<Vec<Arc<str>>>()
                            .join("\n")
                    });
                    deps.insert(Dependency::Exec { cmd, input, id, artifact });
                }
                ("POP", [StackRef(stack)]) => {
                    if let Some(stack) = ctx.stacks.get_mut(stack.as_str()) {
                        stack.pop();
                    }
                }
                ("POP", [PropRef(prop)]) => {
                    if let Some(props) = node.props.as_mut() {
                        if let Some(idx) = props.iter().position(|(k, _)| **k == *prop) {
                            props.remove(idx);
                        }
                    }
                }
                ("PUSH", [StackRef(target), Str(value)])
                    if ["src", "ref"].contains(&target.as_str()) =>
                {
                    let value = replace(value, ctx, &*node, set);
                    ctx.stacks
                        .entry(Arc::from(target.as_str()))
                        .or_default()
                        .push(value.clone());
                    node.add_prop(target.as_str(), Arc::from(value));
                }
                ("PUSH", [StackRef(target), Str(value)]) => {
                    let value = replace(value, ctx, &*node, set);
                    ctx.stacks
                        .entry(Arc::from(target.as_str()))
                        .or_default()
                        .push(value);
                }
                ("PUSH", [StackRef(target), StackRef(source)]) => {
                    let value = ctx
                        .stacks
                        .get(source.as_str())
                        .and_then(|v| v.last().cloned());
                    if let Some(value) = value {
                        ctx.stacks
                            .raw_entry_mut()
                            .from_key(target.as_str())
                            .or_insert(Arc::from(target.as_str()), vec![])
                            .1
                            .push(value);
                    }
                }
                ("PUSH", [StackRef(target), PropRef(prop)]) => {
                    if let Some(value) = node.find_prop(prop) {
                        ctx.stacks
                            .raw_entry_mut()
                            .from_key(target.as_str())
                            .or_insert(Arc::from(target.as_str()), vec![])
                            .1
                            .push(value.to_string().into());
                    }
                }
                ("SET", [StackRef(target), Str(value)]) => {
                    let value = replace(value, ctx, node, set);
                    let v = ctx
                        .stacks
                        .raw_entry_mut()
                        .from_key(target.as_str())
                        .or_insert(Arc::from(target.as_str()), vec![])
                        .1;
                    v.pop();
                    v.push(Cow::Owned(value.to_string()));
                }
                ("WRITE", [StackRef(stack)]) => {
                    let stack = ctx.stacks.get(stack.as_str());
                    if let Some(value) = stack.and_then(|v| v.last()) {
                        out.push_str(value);
                    }
                }
                ("WRITE", [Str(value)]) => out.push_str(&replace(value, ctx, node, set)),
                ("WRITEALL", [StackRef(stack)]) => {
                    let stack = ctx.stacks.get(stack.as_str());
                    if let Some(stack) = stack {
                        stack.iter().for_each(|v| out.push_str(v));
                    }
                }
                ("YIELD", _) => {
                    break;
                }
                ("NOOP", _) => {}
                _ => return Err(LibError::invalid_rule(inst.to_string())),
            }
        }
        Ok(out)
    }
}

fn replace<'a>(
    template: &'a str,
    ctx: &Context,
    node: &Node,
    settings: &LangSettings,
) -> Cow<'a, str> {
    if !template.contains("\\") && !template.contains("$") {
        return Cow::Borrowed(template);
    }

    let value = match settings.is_unescaped_value {
        true => node.value.as_deref().map(Cow::Borrowed),
        false => node.value.as_deref().map(escape_text).to_owned(),
    };

    let mut result = template
        .replace(r#"\v"#, value.as_deref().unwrap_or_default())
        .replace(r#"\m"#, node.marker.as_deref().unwrap_or_default())
        .replace(r#"\n"#, "\n");

    // variables from props
    if template.contains("$") {
        for (key, value) in node.props.iter().flatten() {
            result = result.replace(&["$", key].concat(), value);
        }
    }

    // variables from stacks
    if result.contains("$") {
        for (key, stack) in ctx.stacks.iter() {
            if let Some(last) = stack.last() {
                result = result.replace(&["$", key].concat(), last);
            }
        }
    }
    Cow::Owned(result)
}

#[cfg(test)]
impl Default for Lang {
    fn default() -> Self {
        Self::new(include_str!("../../lib/compiler/markdown.lang"))
            .expect("builtin markdown to work")
    }
}

#[cfg(test)]
mod tests {
    use indoc::indoc;

    use super::*;
    use crate::ast::NodeBuilder;

    #[test]
    fn test_get_instructions() {
        let input = indoc! {
            r#"
            PREPROCESS RULES:
            [SEC...]$
              IS PARAGRAPHABLE
              NOOP
            "#
        };
        let lang = Lang::new(input).unwrap();

        let (instructions, settings) = lang.get_instructions("PREPROCESS", "[SEC]");
        let instructions = instructions.collect::<Vec<&LangInstr>>();
        assert_eq!(
            instructions,
            vec![&LangInstr { op: "NOOP".into(), args: vec![] }]
        );
        assert!(settings.is_paragraphable);
    }

    #[test]
    fn test_evaluate() {
        let input = indoc! {
            r#"
            COMPILE RULES:
            [rule]
              PUSH foo "hello"
              PUSH indent foo
              PUSH indent "world"
              WRITE "ok"
            "#
        };
        let mut deps = HashSet::new();
        let lang = Lang::new(input).unwrap();
        let mut node = Node::default();

        let (mut instructions, settings) = lang.get_instructions("COMPILE", "[rule]");
        let mut ctx = Context::default();

        let value = lang
            .evaluate(&mut instructions, &mut ctx, &mut deps, &mut node, &settings)
            .unwrap();
        assert_eq!(ctx.stacks.get("indent").unwrap(), &["hello", "world"]);
        assert_eq!(value, "ok");
    }

    #[test]
    fn test_evaluate_props() {
        let input = indoc! {
            r#"
            COMPILE RULES:
            [rule]
              WRITE "$word "
              POP PROP word
              WRITE "$word"
            "#
        };
        let mut deps = HashSet::new();
        let lang = Lang::new(input).unwrap();
        let mut node = NodeBuilder::root()
            .add_prop(("word".into(), "hello".into()))
            .add_prop(("word".into(), "world".into()))
            .done();

        let (mut instructions, settings) = lang.get_instructions("COMPILE", "[rule]");
        let mut ctx = Context::default();

        let value = lang
            .evaluate(&mut instructions, &mut ctx, &mut deps, &mut node, &settings)
            .unwrap();
        assert_eq!(value, "hello world");
    }
}
