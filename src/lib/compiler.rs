pub(crate) mod lang;
pub(crate) mod rule;
pub(crate) mod rule_argument;

use std::collections::HashSet;

pub use lang::Lang;
use rule::Context;
pub(crate) use rule::Rule;

use crate::ast::Node;
use crate::parser;
use crate::types::{Dependency, LibError, Pointer};

/// Compile AST to string
pub fn compile(node: &mut Node, lang: Option<&Lang>) -> Result<String, LibError> {
    let mut ignored_deps = HashSet::new();
    compile_recusive(
        std::slice::from_mut(&mut *node),
        &mut Context::default(),
        &mut ignored_deps,
        lang.expect("language"),
        "",
    )
}

fn compile_recusive<'a>(
    nodes: &mut [Node],
    ctx: &mut Context<'a>,
    deps: &mut HashSet<Dependency>,
    lang: &'a Lang,
    base_path: &str,
) -> Result<String, LibError> {
    let mut out = String::new();
    let mut nodes = nodes.iter_mut().peekable();

    while let Some(node) = nodes.next() {
        let path = node.build_path(base_path);

        let mut instructions = lang.get_instructions("COMPILE", &path);

        // Evaluate pre-yield
        let value = lang.evaluate(&mut instructions, &mut *ctx, deps, node)?;
        out.push_str(&value);

        if let Some(Pointer(weak)) = &node.pointer {
            let mutex = weak.upgrade().unwrap();
            let mut node = mutex.lock().unwrap();
            if let Some(children) = node.children.as_mut() {
                out.push_str(&compile_recusive(children, ctx, deps, lang, &path)?);
            }
        } else if let Some(children) = node.children.as_mut() {
            out.push_str(&compile_recusive(children, ctx, deps, lang, &path)?);
        }

        // Evaluate post-yield
        let value = lang.evaluate(&mut instructions, &mut *ctx, deps, node)?;
        out.push_str(&value);

        if nodes.peek().is_some() || node.rule == parser::Rule::Root {
            if let Some(joins) = ctx.stacks.get("join") {
                if let Some(last) = joins.last() {
                    out.push_str(last);
                }
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use indoc::indoc;

    use super::*;
    use crate::ast::NodeBuilder;

    #[test]
    fn test_compile() {
        let lang = Some(Lang::default());
        let mut node = NodeBuilder::root()
            .add_section(vec![NodeBuilder::block(">")
                .add_prop(("src".into(), "bar".into()))
                .add_section(vec![Node::new_line("foo")])
                .done()])
            .done();
        let result = compile(&mut node, lang.as_ref()).unwrap();

        assert_eq!(&result, "> foo\n");
    }

    #[test]
    fn test_compile_nested() {
        let lang = Some(Lang::default());
        let mut node = NodeBuilder::root()
            .add_section(vec![NodeBuilder::block(">")
                .add_prop(("src".into(), "bar".into()))
                .add_section(vec![
                    Node::new_line("foo"),
                    NodeBuilder::block(">")
                        .add_prop(("src".into(), "bar".into()))
                        .add_section(vec![Node::new_line("bar")])
                        .done(),
                    Node::new_line("baz"),
                ])
                .done()])
            .done();
        let result = compile(&mut node, lang.as_ref()).unwrap();
        assert_eq!(
            result,
            indoc! {
            r#"
            > foo
            > > bar
            > baz
            "#
            }
        );
    }
}
