use std::path::PathBuf;

use futures::stream::FuturesUnordered;
use futures::{future::BoxFuture, FutureExt};
use log::{error, info, warn};
use tokio::task::yield_now;
use tokio_stream::StreamExt;

use super::command::Command;
use super::graph_sorter::grouped_topological_sort;
use super::op::Operation;
use super::task;
use super::types::State;
use super::utils::parents;
use super::{
    command::Config,
    types::{AppError, Event, EventRx, EventTx},
};

pub async fn handle(
    _event_tx: EventTx,
    event_rx: EventRx,
    config: &Config,
) -> Result<(), AppError> {
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
    let done = |tasks: &FuturesUnordered<_>| tasks.is_empty() && !config.interactive;

    loop {
        // Allow other tasks to run
        yield_now().await;

        if let Ok(e) = event_rx.try_recv() {
            process_event(e, config, &mut tasks, &state).or_else(handle_error)?;
        } else if let Some(e) = tasks.next().await {
            process_result(e, config, &mut tasks, &state).or_else(handle_error)?;
        } else if tasks.is_empty() {
            process_graph(&mut tasks, &state);

            if done(&tasks) {
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
    match event {
        Event::Command(Ok(cmd)) => match cmd {
            Command::Load { ref paths } => {
                let paths_parents = parents(paths.iter().map(PathBuf::from))?;
                let paths: Vec<_> = paths_parents.into_iter().collect();

                tasks.push(task::index(paths.clone(), state.locations.clone()).boxed());
                state.add_op_chain([Operation::Gather { cmd, paths }, Operation::Finish]);
            }
        },
        Event::Command(Err(_)) => todo!(),
        Event::CommandOk => todo!(),
        Event::TaskOk => {}
        Event::TaskError(_) => todo!(),
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
    tasks: &mut FuturesUnordered<BoxFuture<'static, Result<bool, AppError>>>,
    state: &State,
) {
    let graph = state.operations.lock().expect("poisoned lock");
    let sorted = grouped_topological_sort(&*graph).unwrap();

    let next_tasks = sorted
        .into_iter()
        .find(|group| group.iter().any(|id| !state.is_op_processed(id)))
        .unwrap_or_default();

    for id in next_tasks {
        let vertex = graph.get(&id).unwrap();
        let ops = state.operations.clone();
        let op = vertex.clone();

        match vertex {
            Operation::Gather { .. } => tasks.push(task::gather(op, ops).boxed()),
            Operation::Load { .. } => tasks.push(task::load(op, ops).boxed()),
            Operation::Finish { .. } => {}
            _ => todo!(),
        }
        state.mark_op_processed(id.clone());
    }
}
