use std::path::PathBuf;

use murkdown::ast::NodeBuilder;
use murkdown::types::ExecArtifact;

use crate::cli::command::GraphType;
use crate::cli::task::{exec, gather, graph, index, preprocess, Command};
use crate::cli::types::Source;
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
async fn test_gather_adds_operationss_for_command() {
    let paths = vec!["data:,Hello%20World!#foo".to_string()];
    let sources = paths.clone().into_iter().map(Source::Url).collect();
    let op = Operation::Gather {
        cmd: Command::Build { paths, splits: vec![] },
        sources,
        splits: Some(vec![]),
    };
    let ctx = State::new_loaded("markdown");
    gather(op, ctx.operations.clone()).await.unwrap();

    let graph = ctx.operations.lock().unwrap();
    let mut result_keys = graph.iter().map(|(v, _, _)| v).collect::<Vec<_>>();
    result_keys.sort();

    assert_eq!(
        result_keys,
        [
            &OpId::gather(),
            &OpId::load("foo"),
            &OpId::parse("foo"),
            &OpId::preprocess("foo"),
            &OpId::compile("foo"),
            &OpId::write("foo"),
            &OpId::finish()
        ]
    );
}

#[tokio::test]
async fn test_exec_returns_error_on_nonzero() {
    let op = Operation::Exec {
        id: "foo".into(),
        cmd: "test 1 = 0".to_string(),
        input: None,
        artifact: ExecArtifact::Stdout(String::new()),
    };
    let ctx = State::new();

    let result = exec(op, ctx.asts, ctx.artifacts).await;

    assert!(result.is_err());
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
    let ctx = State::new_loaded("markdown");
    ctx.insert_location("bar", PathBuf::from("file.txt"));
    ctx.insert_artifact(&dep, Artifact::Ast(node));

    preprocess(
        op,
        "markdown".to_string(),
        dep,
        ctx.asts,
        ctx.operations.clone(),
        ctx.artifacts,
        ctx.languages,
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
                source: Source::from("file.txt")
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
    let ctx = State::new_loaded("markdown");
    ctx.insert_location("bar", PathBuf::from("file.txt"));
    ctx.insert_artifact(&dep, Artifact::Ast(node));

    preprocess(
        op,
        "markdown".to_string(),
        dep,
        ctx.asts,
        ctx.operations.clone(),
        ctx.artifacts,
        ctx.languages,
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
            &OpId::compile("bar"),
            &OpId::write("bar"),
            &OpId::finish()
        ]
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
