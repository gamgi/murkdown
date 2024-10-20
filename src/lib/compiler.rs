pub(crate) mod lang;
pub(crate) mod rule;

use lang::Lang;
use rule::Context;

use crate::ast::Node;
use crate::types::LibError;

/// Compile AST to string
pub fn compile(node: &mut Node) -> Result<String, LibError> {
    let lang = Lang::new(include_str!("compiler/markdown.lang"));
    compile_recusive(&[node.clone()], &mut Context::default(), &lang, "");
    todo!();
}

fn compile_recusive(nodes: &[Node], ctx: &mut Context, lang: &Lang, path: &str) -> String {
    todo!();
}
