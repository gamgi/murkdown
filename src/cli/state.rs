use std::path::PathBuf;

use futures::stream::FuturesUnordered;
use futures::{future::BoxFuture, FutureExt};
use tokio::task::yield_now;

use super::command::Command;
use super::task;
use super::types::State;
use super::utils::parents;
use super::{
    command::Config,
    types::{AppError, Event, EventRx, EventTx},
};

pub async fn handle(event_tx: EventTx, event_rx: EventRx, config: &Config) -> Result<(), AppError> {
    let state = State::new();

    handle_state(event_tx, event_rx, config, state).await
}

pub async fn handle_state(
    event_tx: EventTx,
    mut event_rx: EventRx,
    config: &Config,
    state: State,
) -> Result<(), AppError> {
    let mut tasks = FuturesUnordered::<BoxFuture<Result<bool, _>>>::new();

    loop {
        // Allow other tasks to run
        yield_now().await;

        if let Ok(e) = event_rx.try_recv() {
            process_event(e, config, &mut tasks, &state)?;
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
        Event::Command { cmd } => match cmd {
            Ok(Command::Load { paths }) => {
                let paths = paths.iter().map(PathBuf::from);
                let paths_to_index = parents(paths)?.into_iter().collect();
                tasks.push(task::index(paths_to_index, state.locations.clone()).boxed());
            }
            Err(_) => todo!(),
        },
    }
    Ok(())
}
