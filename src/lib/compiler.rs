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
pub fn compile(node: &mut Node, lang: &Lang) -> Result<String, LibError> {
    let mut ignored_deps = HashSet::new();
    compile_recusive(
        std::slice::from_mut(&mut *node),
        &mut Context::default(),
        &mut ignored_deps,
        lang,
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
    let mut idx = 0;

    while let Some(node) = nodes.next() {
        let path = node.build_path(base_path);
        ctx.set_parent(node);
        ctx.set_index(idx);

        let rules = lang.get_rules("COMPILE", &path);
        let mut rules_stack = Vec::new();

        // Evaluate pre-yield
        for rule in rules {
            let mut instructions = rule.instructions.iter();
            let settings = rule.settings;
            let value = lang.evaluate(&mut instructions, &mut *ctx, deps, node, &settings)?;
            out.push_str(&value);
            rules_stack.push((instructions, settings));
        }

        if let Some(Pointer(weak)) = &node.pointer {
            let mutex = weak.upgrade().unwrap();
            if let parser::Rule::Ellipsis = node.rule {
                // NOTE: skip block node
                let mut block = mutex.lock().unwrap();
                if let Some(children) = block.children.as_mut() {
                    // NOTE: skip section node
                    for section in children {
                        assert_eq!(section.rule, parser::Rule::Section);
                        if let Some(children) = section.children.as_mut() {
                            // fall through Ellipsis and only render Section contents
                            out.push_str(&compile_recusive(children, ctx, deps, lang, base_path)?);
                            idx += 1;
                        }
                    }
                }
            } else {
                let mut node = mutex.lock().expect("poisoned or deadlack");
                if let Some(children) = node.children.as_mut() {
                    out.push_str(&compile_recusive(children, ctx, deps, lang, &path)?);
                    idx += 1;
                }
            }
        } else if let Some(children) = node.children.as_mut() {
            out.push_str(&compile_recusive(children, ctx, deps, lang, &path)?);
            idx += 1;
        }

        // Evaluate post-yield
        rules_stack.reverse();
        for (mut instructions, settings) in rules_stack {
            let value = lang.evaluate(&mut instructions, &mut *ctx, deps, node, &settings)?;
            out.push_str(&value);
        }

        if nodes.peek().is_some() || matches!(node.rule, parser::Rule::RootA | parser::Rule::RootB)
        {
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
        let lang = Lang::markdown();
        let mut node = NodeBuilder::root()
            .add_section(vec![NodeBuilder::block(">")
                .add_prop(("src".into(), "bar".into()))
                .add_section(vec![Node::line("foo")])
                .done()])
            .done();
        let result = compile(&mut node, &lang).unwrap();

        assert_eq!(&result, "> foo\n");
    }

    #[test]
    fn test_compile_nested() {
        let lang = Lang::markdown();
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
        let result = compile(&mut node, &lang).unwrap();
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
    fn test_compile_composable() {
        let lang = Lang::new(indoc! {
            r#"
            RULES FOR test PRODUCE text/plain
            COMPILE RULES:
            [INDENTED] [SEC] LINE$
              IS COMPOSABLE
              WRITE "  "
            [SEC...] LINE$
              IS COMPOSABLE
              WRITE "\v"
            LINE$
              WRITE "ish\n"
            "#
        })
        .unwrap();
        let mut node = NodeBuilder::root()
            .add_section(vec![NodeBuilder::block(">")
                .add_prop(("src".into(), "bar".into()))
                .add_section(vec![
                    Node::line("foo"),
                    NodeBuilder::block(">")
                        .headers(Some(vec!["INDENTED".into()]))
                        .add_section(vec![Node::line("bar")])
                        .done(),
                    Node::line("baz"),
                ])
                .done()])
            .done();
        let result = compile(&mut node, &lang).unwrap();
        assert_eq!(
            result,
            indoc! {
            r#"
            fooish
              barish
            bazish
            "#
            }
        );
    }

    #[test]
    fn test_compile_composable_with_yield() {
        let lang = Lang::new(indoc! {
            r#"
            RULES FOR test PRODUCE text/plain
            COMPILE RULES:
            [...INDENTED...] [SEC]$
              IS COMPOSABLE
              PUSH indent "  "
              YIELD
              POP indent
            [...DRAMATIC...] [SEC]$
              IS COMPOSABLE
              PUSH prefix "Wow! "
              PUSH suffix "!"
              YIELD
              POP prefix
              POP suffix
            LINE$
              WRITEALL indent
              WRITEALL prefix
              WRITE "\v"
              WRITEALL suffix
              WRITE "\n"
            "#
        })
        .unwrap();
        let mut node = NodeBuilder::root()
            .add_section(vec![NodeBuilder::block(">")
                .add_prop(("src".into(), "bar".into()))
                .add_section(vec![
                    Node::line("foo"),
                    NodeBuilder::block(">")
                        .headers(Some(vec!["INDENTED DRAMATIC".into()]))
                        .add_section(vec![Node::line("bar")])
                        .done(),
                    Node::line("baz"),
                ])
                .done()])
            .done();
        let result = compile(&mut node, &lang).unwrap();
        assert_eq!(
            result,
            indoc! {
            r#"
            foo
              Wow! bar!
            baz
            "#
            }
        );
    }

    #[test]
    fn test_compile_escapes_value_by_default() {
        let lang = Lang::new(indoc! {
            r#"
            RULES FOR test PRODUCE text/plain
            COMPILE RULES:
            [SEC...] LINE$
              WRITE "\v"
            "#
        })
        .unwrap();
        let unescaped_lang = Lang::new(indoc! {
            r#"
            RULES FOR test PRODUCE text/plain
            COMPILE RULES:
            [SEC...] LINE$
              IS UNESCAPED_VALUE
              WRITE "\v"
            "#
        })
        .unwrap();
        let mut node = NodeBuilder::root()
            .add_section(vec![NodeBuilder::block(">")
                .add_prop(("src".into(), "bar".into()))
                .add_section(vec![Node::line("<br />")])
                .done()])
            .done();

        let result = compile(&mut node, &unescaped_lang).unwrap();
        assert_eq!(&result, "<br />");

        let result = compile(&mut node, &lang).unwrap();
        assert_eq!(&result, "&lt;br /&gt;");
    }

    #[test]
    fn test_compile_skips_ellipsis_block_and_section_nodes() {
        let lang = Lang::new(indoc! {
            r#"
            RULES FOR test PRODUCE text/plain
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
        .unwrap();
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

        let result = compile(&mut node, &lang).unwrap();
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
