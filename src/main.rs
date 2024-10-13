mod cli;
use clap::Parser;
use cli::{
    command::{self, Config},
    types::Event,
};

use tokio::sync;
fn main() {
    let config = Config::parse();
    let (tx, rx) = sync::mpsc::unbounded_channel::<Event>();
    let handle_commands = command::handle(tx, &config);
}
