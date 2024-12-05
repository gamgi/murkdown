use std::sync::atomic::Ordering;
use std::sync::{Arc, MutexGuard};

use futures::stream::FuturesUnordered;
use futures::{future::BoxFuture, FutureExt};
use log::{error, info};
use murkdown::types::{ExecArtifact, ExecInput, LocationMap};
use tokio::task::yield_now;
use tokio_stream::StreamExt;

use super::command::Command;
use super::graph_sorter::grouped_topological_sort;
use super::op::{OpId, Operation};
use super::state_context::State;
use super::task;
use super::types::Source;
use super::utils::parents;
use super::{
    command::Config,
    types::{AppError, Event, EventRx},
};

pub async fn handle(event_rx: EventRx, config: &Config) -> Result<(), AppError> {
    let state = State::new();
    state.load_languages(config.format.as_ref().expect("format"))?;

    handle_state(event_rx, config, state).await
}

pub async fn handle_state(
    mut event_rx: EventRx,
    config: &Config,
    state: State,
) -> Result<(), AppError> {
    let mut tasks = FuturesUnordered::<BoxFuture<Result<bool, _>>>::new();
    let handle_error = |e| process_error(e, config);
    let done = |tasks: &FuturesUnordered<_>, state: &State| {
        tasks.is_empty() && (!config.interactive || state.should_exit.load(Ordering::Relaxed))
    };

    loop {
        // Allow other tasks to run
        yield_now().await;

        if let Ok(e) = event_rx.try_recv() {
            process_event(e, config, &mut tasks, &state).or_else(handle_error)?;
        } else if let Some(e) = tasks.next().await {
            process_result(e, config, &mut tasks, &state).or_else(handle_error)?;
        } else if tasks.is_empty() {
            process_graph(config, &mut tasks, &state);

            if done(&tasks, &state) {
                break Ok(());
            }
        }
    }
}

fn process_event(
    event: Event,
    _config: &Config,
    tasks: &mut FuturesUnordered<BoxFuture<'static, Result<bool, AppError>>>,
    state: &State,
) -> Result<(), AppError> {
    let get_sources_and_parents = |paths: &[String], locs: MutexGuard<LocationMap>| {
        let sources = paths
            .iter()
            .map(|path| {
                // NOTE: replace sources that have been indexed previously
                if let Some(loc) = locs.get(path) {
                    Source::from(loc.clone())
                } else {
                    Source::from(path)
                }
            })
            .collect::<Vec<_>>();
        let sources_paths = sources
            .clone()
            .into_iter()
            .filter_map(|s| s.try_into().ok());
        let paths_parents = parents(sources_paths)?.into_iter().collect::<Vec<_>>();
        Ok::<_, AppError>((sources, paths_parents))
    };
    match event {
        Event::Command(Ok(cmd)) => match cmd {
            Command::Graph { ref paths, graph_type, .. } => {
                info!(target = "status"; "Building {} sources and {} graph", paths.len(), graph_type);
                let (sources, paths_parents) = {
                    let locs = state.locations.lock().expect("poisoned lock");
                    get_sources_and_parents(paths, locs)?
                };
                let splits = None;

                tasks.push(task::index(paths_parents, state.locations.clone()).boxed());
                let id: Arc<str> = Arc::from(format!("{graph_type}_graph.png"));
                let input = Some(ExecInput::URI(format!("graph:{graph_type}")));

                // NOTE: tasks are scheduled here because there should only be one graph task
                state.insert_op_chain([
                    Operation::Gather { cmd, sources, splits },
                    Operation::Finish,
                    Operation::Graph { graph_type },
                    Operation::Exec {
                        id: id.clone(),
                        cmd: "plantuml -pipe -tpng".to_string(),
                        input,
                        artifact: ExecArtifact::Stdout("image/png".to_string()),
                    },
                    Operation::Write { id },
                ]);
            }
            Command::Index { ref paths, .. } => {
                info!(target = "status"; "Indexing {} sources", paths.len());
                let mut locs = state.locations.lock().expect("poisoned lock");
                let sources = paths.iter().map(Source::from).collect::<Vec<_>>();
                for source in sources {
                    locs.insert(source.path()?, source.into());
                }
            }
            Command::Build { ref paths, ref splits, .. } => {
                info!(target = "status"; "Building {} sources", paths.len());
                let (sources, parents) = {
                    let locs = state.locations.lock().expect("poisoned lock");
                    get_sources_and_parents(paths, locs)?
                };
                let splits = Some(splits.clone());

                tasks.push(task::index(parents, state.locations.clone()).boxed());
                state.insert_op_chain([
                    Operation::Gather { cmd, sources, splits },
                    Operation::Finish,
                ]);
            }
            Command::Exit => state.should_exit.store(true, Ordering::Relaxed),
        },
        Event::Command(Err(e)) => {
            error!(target = "status"; "Error {e}");
        }
        Event::CommandOk => {
            info!(target = "status"; "Done");
        }
        Event::TaskOk => {}
        Event::TaskError(e) => return Err(e),
    }
    Ok(())
}

fn process_result(
    res: Result<bool, AppError>,
    config: &Config,
    tasks: &mut FuturesUnordered<BoxFuture<'static, Result<bool, AppError>>>,
    state: &State,
) -> Result<(), AppError> {
    match res {
        Ok(true) => process_event(Event::CommandOk, config, tasks, state),
        Ok(false) => process_event(Event::TaskOk, config, tasks, state),
        Err(e) => process_event(Event::TaskError(e), config, tasks, state),
    }
}

fn process_error(error: AppError, config: &Config) -> Result<(), AppError> {
    error!("{}", error);
    match config.interactive {
        true => Ok(()),
        false => Err(error),
    }
}

fn process_graph(
    config: &Config,
    tasks: &mut FuturesUnordered<BoxFuture<'static, Result<bool, AppError>>>,
    state: &State,
) {
    let operations = state.operations.lock().expect("poisoned lock");
    let sorted = grouped_topological_sort(&*operations).unwrap();

    let next_tasks = sorted
        .into_iter()
        .skip_while(|group| group.iter().all(|id| state.is_op_processed(id)));

    for mut batch in next_tasks {
        batch.retain(|id| !state.is_op_processed(id));

        // schedule tasks
        for opid in batch {
            let vertex = operations.get(&opid).unwrap();
            let op = vertex.clone();
            let dep = operations.get_first_node_dependency(&op).map(OpId::uri);
            let asts = state.asts.clone();
            let arts = state.artifacts.clone();
            let ops = state.operations.clone();
            let locs = state.locations.clone();
            let langs = state.languages.clone();
            let out = config.output.clone().expect("output");
            let fmt = config.format.clone().expect("format");

            use Operation::*;
            match vertex {
                Gather { .. } => tasks.push(task::gather(op, ops).boxed()),
                Exec { .. } => tasks.push(task::exec(op, asts, arts).boxed()),
                Load { .. } => tasks.push(task::load(op, asts, arts).boxed()),
                Tangle { .. } => tasks.push(task::tangle(op, dep.unwrap(), arts).boxed()),
                Parse { .. } => tasks.push(task::parse(op, dep.unwrap(), arts).boxed()),
                Preprocess { .. } => tasks.push(
                    task::preprocess(op, fmt, dep.unwrap(), asts, ops, arts, langs, locs).boxed(),
                ),
                Compile { .. } => {
                    tasks.push(task::compile(op, fmt, dep.unwrap(), arts, langs).boxed())
                }
                CompilePlaintext { source_uri, .. } => {
                    tasks.push(task::compile_plaintext(op, source_uri.clone(), arts, langs).boxed())
                }
                Write { .. } => tasks.push(task::write(op, dep.unwrap(), arts, out).boxed()),
                Copy { .. } => tasks.push(task::copy(op, out).boxed()),
                Graph { .. } => tasks.push(task::graph(op, ops, arts).boxed()),
                Finish => tasks.push(task::finish(op).boxed()),
            }
            state.mark_op_processed(opid.clone());
        }

        // keep scheduling if batch yelded no tasks (eg. `Operation::Finish`)
        if !tasks.is_empty() {
            break;
        }
    }
}
