use murkdown::ast::NodeBuilder;
use murkdown::types::URI;

use crate::cli::task::preprocess;
use crate::cli::{
    artifact::Artifact,
    op::{OpId, Operation},
    state_context::State,
};

fn get_params() -> (Operation, URI, State) {
    let op = Operation::Preprocess { id: "foo".into() };
    let dep = op.uri();
    let ctx = State::new();
    (op, dep, ctx)
}

#[tokio::test]
async fn test_preprocess_adds_src_ops() {
    let node = NodeBuilder::root()
        .children(vec![NodeBuilder::block(">")
            .add_prop(("src".into(), "bar".into()))
            .done()])
        .done();
    let (op, dep, ctx) = get_params();
    ctx.insert_artifact(&dep, Artifact::Ast(node));

    preprocess(
        op,
        dep,
        ctx.asts,
        ctx.operations.clone(),
        ctx.artifacts,
        ctx.locations,
    )
    .await
    .unwrap();

    let graph = ctx.operations.lock().unwrap();
    let mut result = graph.iter().map(|(v, _, _)| v).collect::<Vec<_>>();
    result.sort();

    assert_eq!(
        result,
        [
            &OpId::load("bar"),
            &OpId::parse("bar"),
            &OpId::preprocess("bar"),
            &OpId::preprocess("foo"),
        ]
    );
}

#[tokio::test]
async fn test_preprocess_adds_ref_ops() {
    let node = NodeBuilder::root()
        .children(vec![NodeBuilder::block(">")
            .add_prop(("ref".into(), "bar".into()))
            .done()])
        .done();
    let (op, dep, ctx) = get_params();
    ctx.insert_artifact(&dep, Artifact::Ast(node));

    preprocess(
        op,
        dep,
        ctx.asts,
        ctx.operations.clone(),
        ctx.artifacts,
        ctx.locations,
    )
    .await
    .unwrap();

    let graph = ctx.operations.lock().unwrap();
    let mut result = graph.iter().map(|(v, _, _)| v).collect::<Vec<_>>();
    result.sort();

    assert_eq!(result, [&OpId::preprocess("foo"), &OpId::copy("bar"),]);
}
