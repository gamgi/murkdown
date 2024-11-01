use std::path::PathBuf;

use murkdown::ast::NodeBuilder;

use crate::cli::command::GraphType;
use crate::cli::task::{graph, index, preprocess};
use crate::cli::{
    artifact::Artifact,
    op::{OpId, Operation},
    state_context::State,
};

#[tokio::test]
async fn test_index_strips_relative_path_and_duplicates() {
    let ctx = State::new();

    index(
        vec![
            PathBuf::from("./src/cli/task/tests.rs"),
            PathBuf::from("src/cli/task/tests.rs"),
        ],
        ctx.locations.clone(),
    )
    .await
    .unwrap();

    let locs = ctx.locations.lock().unwrap();
    let result_keys = locs.keys().collect::<Vec<_>>();

    assert_eq!(result_keys, [&"src/cli/task/tests.rs".to_string()]);
}

#[tokio::test]
async fn test_preprocess_adds_src_operations() {
    let node = NodeBuilder::root()
        .children(vec![NodeBuilder::block(">")
            .add_prop(("src".into(), "bar".into()))
            .done()])
        .done();
    let op = Operation::Preprocess { id: "foo".into() };
    let dep = op.uri();
    let ctx = State::new();
    ctx.insert_location("bar", PathBuf::from("file.txt"));
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
    let mut result_keys = graph.iter().map(|(v, _, _)| v).collect::<Vec<_>>();
    result_keys.sort();

    assert_eq!(
        result_keys,
        [
            &OpId::load("bar"),
            &OpId::parse("bar"),
            &OpId::preprocess("bar"),
            &OpId::preprocess("foo"),
        ]
    );

    let mut result_ops = graph.iter().map(|(_, o, _)| o).collect::<Vec<_>>();
    result_ops.sort();

    assert_eq!(
        result_ops,
        [
            &Operation::Load {
                id: "bar".into(),
                path: PathBuf::from("file.txt")
            },
            &Operation::Parse { id: "bar".into() },
            &Operation::Preprocess { id: "bar".into() },
            &Operation::Preprocess { id: "foo".into() }
        ]
    );
}

#[tokio::test]
async fn test_preprocess_adds_ref_operations() {
    let node = NodeBuilder::root()
        .children(vec![NodeBuilder::block(">")
            .add_prop(("ref".into(), "bar".into()))
            .done()])
        .done();
    let op = Operation::Preprocess { id: "foo".into() };
    let dep = op.uri();
    let ctx = State::new();
    ctx.insert_location("bar", PathBuf::from("file.txt"));
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
    let mut result_keys = graph.iter().map(|(v, _, _)| v).collect::<Vec<_>>();
    result_keys.sort();

    assert_eq!(result_keys, [&OpId::copy("bar"), &OpId::finish()]);
}

#[tokio::test]
async fn test_preprocess_adds_exec_input_dependency() {
    let node = NodeBuilder::root()
        .add_section(vec![NodeBuilder::block(">")
            .headers(Some(vec!["EXEC".into()]))
            .done()])
        .done();
    let op = Operation::Preprocess { id: "foo".into() };
    let dep = op.uri();
    let ctx = State::new();
    ctx.insert_op(op.clone());
    ctx.insert_location("bar", PathBuf::from("file.txt"));
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

    let mut result = vec![];
    for (from, _vertex, edges) in graph.iter() {
        let deps = edges.iter().cloned().collect::<Vec<_>>();
        result.push((from, deps));
    }
    result.sort();
    assert_eq!(
        result,
        [(&OpId::preprocess("foo"), vec![OpId::exec("run")]),]
    );
}

#[tokio::test]
async fn test_graph() {
    let node = NodeBuilder::root()
        .children(vec![NodeBuilder::block(">")
            .add_prop(("ref".into(), "bar".into()))
            .done()])
        .done();
    let op = Operation::Graph { graph_type: GraphType::Dependencies };
    let ctx = State::new();
    ctx.insert_artifact(&op.uri(), Artifact::Ast(node));

    graph(op, ctx.operations, ctx.artifacts.clone())
        .await
        .unwrap();

    let artifacts = ctx.artifacts.lock().unwrap();
    let artifact = artifacts.get("graph:dependencies").unwrap();
    let Artifact::Plaintext(media_type, content) = artifact else {
        unreachable!()
    };

    assert_eq!(media_type, "text/plantuml");
    assert_eq!(
        content,
        "@startuml\nskinparam defaultTextAlignment center\n'nodes\n'dependencies\n@enduml"
    );
}
