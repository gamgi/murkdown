use clap::Parser;
use futures::StreamExt;

use super::{
    reader::Reader,
    types::{AppError, Event, EventTx},
};

#[derive(Parser, Debug, Default)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Config {
    /// Interactive mode
    #[clap(long, global = true)]
    pub interactive: bool,

    /// Increase level of verbosity
    #[clap(short, action = clap::ArgAction::Count, global = true)]
    pub verbosity: u8,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Parser, Debug, Clone, PartialEq, Eq)]
pub(crate) enum Command {
    //// Load content into memory
    Load {
        /// Path pattern or data URL
        pat: String,
    },
}

pub async fn handle(event_tx: EventTx, config: &Config) -> Result<(), AppError> {
    let mut reader = Reader::from(config);
    while let Some(cmd) = reader.next().await {
        event_tx.send(Event::Command { cmd })?;
    }
    Ok(())
}
