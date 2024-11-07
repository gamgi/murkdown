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

        let (mut instructions, settings) = lang.get_instructions("COMPILE", &path);

        // Evaluate pre-yield
        let value = lang.evaluate(&mut instructions, &mut *ctx, deps, node, &settings)?;
        out.push_str(&value);

        if let Some(Pointer(weak)) = &node.pointer {
            let mutex = weak.upgrade().unwrap();
            if let parser::Rule::Ellipsis = node.rule {
                // NOTE: skip block node
                let mut block = mutex.lock().unwrap();
                assert_eq!(block.rule, parser::Rule::Block);
                if let Some(children) = block.children.as_mut() {
                    // NOTE: skip section node
                    for section in children {
                        assert_eq!(section.rule, parser::Rule::Section);
                        if let Some(children) = section.children.as_mut() {
                            // fall through Ellipsis and only render Section contents
                            out.push_str(&compile_recusive(children, ctx, deps, lang, base_path)?);
                        }
                    }
                }
            } else {
                let mut node = mutex.lock().expect("poisoned or deadlack");
                if let Some(children) = node.children.as_mut() {
                    out.push_str(&compile_recusive(children, ctx, deps, lang, &path)?);
                }
            }
        } else if let Some(children) = node.children.as_mut() {
            out.push_str(&compile_recusive(children, ctx, deps, lang, &path)?);
        }

        // Evaluate post-yield
        let value = lang.evaluate(&mut instructions, &mut *ctx, deps, node, &settings)?;
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
    use std::sync::{Arc, Mutex};

    use indoc::indoc;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::ast::NodeBuilder;

    #[test]
    fn test_compile() {
        let lang = Some(Lang::default());
        let mut node = NodeBuilder::root()
            .add_section(vec![NodeBuilder::block(">")
                .add_prop(("src".into(), "bar".into()))
                .add_section(vec![Node::line("foo")])
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
                    Node::line("foo"),
                    NodeBuilder::block(">")
                        .add_prop(("src".into(), "bar".into()))
                        .add_section(vec![Node::line("bar")])
                        .done(),
                    Node::line("baz"),
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

    #[test]
    fn test_compile_escapes_value_by_default() {
        let lang = Lang::new(indoc! {
            r#"
            COMPILE RULES:
            [SEC...] LINE$
              WRITE "\v"
            "#
        })
        .ok();
        let unescaped_lang = Lang::new(indoc! {
            r#"
            COMPILE RULES:
            [SEC...] LINE$
              IS UNESCAPED_VALUE
              WRITE "\v"
            "#
        })
        .ok();
        let mut node = NodeBuilder::root()
            .add_section(vec![NodeBuilder::block(">")
                .add_prop(("src".into(), "bar".into()))
                .add_section(vec![Node::line("<br />")])
                .done()])
            .done();

        let result = compile(&mut node, unescaped_lang.as_ref()).unwrap();
        assert_eq!(&result, "<br />");

        let result = compile(&mut node, lang.as_ref()).unwrap();
        assert_eq!(&result, "&lt;br /&gt;");
    }

    #[test]
    fn test_compile_skips_ellipsis_block_and_section_nodes() {
        let lang = Lang::new(indoc! {
            r#"
            COMPILE RULES:
            ^[]$
              NOOP
            ^[] [SEC]$
              NOOP
            [SEC]$
              WRITE "  section start\n"
              YIELD
              WRITE "  section end\n"
            [...]$
              WRITE "block start\n"
              YIELD
              WRITE "block end\n"
            [SEC] LINE$
              WRITE "    \v\n"
            "#
        })
        .ok();
        let mutex = Mutex::new(
            NodeBuilder::block(">")
                .add_prop(("id".into(), "includeme".into()))
                .add_section(vec![Node::line("hello")])
                .done(),
        );
        let arc = Arc::new(mutex);
        let pointer = Pointer(Arc::downgrade(&arc));
        let mut node = NodeBuilder::root()
            .add_section(vec![])
            .add_section(vec![NodeBuilder::block(">")
                .add_prop(("src".into(), "includeme".into()))
                .add_section(vec![Node::ellipsis(Some(pointer)), Node::line("world")])
                .done()])
            .done();

        let result = compile(&mut node, lang.as_ref()).unwrap();
        assert_eq!(
            &result,
            indoc! {r#"
            block start
              section start
                hello
                world
              section end
            block end
            "#
            }
        );
    }
}
