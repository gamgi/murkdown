pub(crate) mod lang;
pub(crate) mod rule;
pub(crate) mod rule_argument;

use lang::Lang;
use rule::Context;
pub(crate) use rule::Rule;

use crate::ast::Node;
use crate::parser;
use crate::types::LibError;

/// Compile AST to string
pub fn compile(node: &mut Node) -> Result<String, LibError> {
    let lang = Lang::new(include_str!("compiler/markdown.lang"))?;
    compile_recusive(
        std::slice::from_mut(&mut *node),
        &mut Context::default(),
        &lang,
        "",
    )
}

fn compile_recusive<'a, 'c>(
    nodes: &'c mut [Node],
    ctx: &mut Context<'a>,
    lang: &'a Lang,
    base_path: &str,
) -> Result<String, LibError> {
    let mut out = String::new();
    let mut nodes = nodes.iter_mut().peekable();

    while let Some(node) = nodes.next() {
        let path = node.build_path(base_path);

        let mut instructions = lang.get_instructions(&path);

        // Evaluate pre-yield
        let value = lang.evaluate(&mut instructions, &mut *ctx, node)?;
        out.push_str(&value);

        if let Some(children) = node.children.as_mut() {
            out.push_str(&compile_recusive(children, ctx, lang, &path)?);
        }

        // Evaluate post-yield
        let value = lang.evaluate(&mut instructions, &mut *ctx, node)?;
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
    use super::*;
    use crate::ast::NodeBuilder;

    #[test]
    fn test_compile() {
        let mut node = NodeBuilder::root()
            .children(vec![NodeBuilder::block(">")
                .add_prop(("src".into(), "bar".into()))
                .add_child(Node::new_line("foo"))
                .done()])
            .done();
        let result = compile(&mut node).unwrap();

        assert_eq!(&result, "> foo\n");
    }

    #[test]
    fn test_compile_nested() {
        let mut node = NodeBuilder::root()
            .children(vec![NodeBuilder::block(">")
                .add_prop(("src".into(), "bar".into()))
                .add_child(Node::new_line("foo"))
                .add_child(
                    NodeBuilder::block(">")
                        .add_prop(("src".into(), "bar".into()))
                        .add_child(Node::new_line("bar"))
                        .done(),
                )
                .add_child(Node::new_line("baz"))
                .done()])
            .done();
        let result = compile(&mut node).unwrap();

        assert_eq!(&result, "> foo\n>> bar\n> baz\n");
    }
}
