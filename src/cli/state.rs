use std::{path::PathBuf, sync::atomic::Ordering};

use futures::stream::FuturesUnordered;
use futures::{future::BoxFuture, FutureExt};
use log::error;
use tokio::task::yield_now;
use tokio_stream::StreamExt;

use super::command::Command;
use super::graph_sorter::grouped_topological_sort;
use super::op::{OpId, Operation};
use super::state_context::State;
use super::task;
use super::types::AppErrorKind;
use super::utils::parents;
use super::{
    command::Config,
    types::{AppError, Event, EventRx},
};

pub async fn handle(event_rx: EventRx, config: &Config) -> Result<(), AppError> {
    let state = State::new();

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
    let get_paths = |paths: &[String]| {
        let paths = paths.iter().map(PathBuf::from).collect::<Vec<_>>();
        let paths_parents = parents(paths.clone().into_iter())?
            .into_iter()
            .collect::<Vec<_>>();
        Ok::<_, AppError>((paths, paths_parents))
    };
    match event {
        Event::Command(Ok(cmd)) => match cmd {
            Command::Graph { ref paths, graph_type } => {
                let (paths, paths_parents) = get_paths(paths)?;
                let splits = None;

                tasks.push(task::index(paths_parents, state.locations.clone()).boxed());
                state.insert_op_chain([
                    Operation::Gather { cmd, paths, splits },
                    Operation::Finish,
                    Operation::Graph { graph_type },
                ]);
            }
            Command::Load { ref paths, .. } => {
                let (paths, paths_parents) = get_paths(paths)?;
                let splits = None;

                tasks.push(task::index(paths_parents, state.locations.clone()).boxed());
                state
                    .insert_op_chain([Operation::Gather { cmd, paths, splits }, Operation::Finish]);
            }
            Command::Build { ref paths, ref splits, .. } => {
                let (paths, paths_parents) = get_paths(paths)?;
                let splits = Some(splits.clone());

                tasks.push(task::index(paths_parents, state.locations.clone()).boxed());
                state
                    .insert_op_chain([Operation::Gather { cmd, paths, splits }, Operation::Finish]);
            }
            Command::Exit => state.should_exit.store(true, Ordering::Relaxed),
        },
        Event::Command(Err(_)) => todo!(),
        Event::CommandOk => todo!(),
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
    if let AppErrorKind::Exit(_) = error.inner() {
        Err(error)
    } else {
        error!("{}", error);
        match config.interactive {
            true => Ok(()),
            false => Err(error),
        }
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
            let out = config.output.clone().expect("output");

            match vertex {
                Operation::Gather { .. } => tasks.push(task::gather(op, ops).boxed()),
                Operation::Load { .. } => tasks.push(task::load(op, asts, arts).boxed()),
                Operation::Parse { .. } => tasks.push(task::parse(op, dep.unwrap(), arts).boxed()),
                Operation::Preprocess { .. } => {
                    tasks.push(task::preprocess(op, dep.unwrap(), asts, ops, arts, locs).boxed())
                }
                Operation::Compile { .. } => {
                    tasks.push(task::compile(op, dep.unwrap(), arts).boxed())
                }
                Operation::Write { .. } => {
                    tasks.push(task::write(op, dep.unwrap(), arts, out).boxed())
                }
                Operation::Copy { .. } => tasks.push(task::copy(op, out).boxed()),
                Operation::Graph { .. } => tasks.push(task::graph(op, ops).boxed()),
                Operation::Finish => {}
            }
            state.mark_op_processed(opid.clone());
        }

        // keep scheduling if batch yelded no tasks (eg. `Operation::Finish`)
        if !tasks.is_empty() {
            break;
        }
    }
}
