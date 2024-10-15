#![feature(error_generic_member_access)]
mod cli;
use clap::Parser;
use cli::{
    command::{self, Config},
    logger::setup_logging,
    state,
    types::{AppError, Event},
};
use tokio::{sync, try_join};

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let config = Config::parse();
    let (tx, rx) = sync::mpsc::unbounded_channel::<Event>();
    setup_logging(&config);

    let handle_state = state::handle(tx.clone(), rx, &config);
    let handle_commands = command::handle(tx, &config);

    try_join!(handle_state, handle_commands).map(|_| ())
}
