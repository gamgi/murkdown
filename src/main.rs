mod cli;
use clap::Parser;
use cli::{
    command::{self, Config},
    logger::setup_logging,
    types::Event,
};
use tokio::sync;
fn main() {
    let config = Config::parse();
    let (tx, rx) = sync::mpsc::unbounded_channel::<Event>();
    setup_logging(&config);

    let handle_commands = command::handle(tx, &config);
}
