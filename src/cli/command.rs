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

    /// Progress output format
    #[clap(long, default_value = "auto", value_parser = parse_progress)]
    #[clap(global = true)]
    pub progress: &'static str,

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
        /// Input paths
        paths: Vec<String>,
    },
    //// Build sources
    Build {
        /// Input paths
        #[clap(value_name = "PATH")]
        #[arg(default_values_t = [".".to_string()])]
        paths: Vec<String>,
    },
}

pub async fn handle(event_tx: EventTx, config: &Config) -> Result<(), AppError> {
    let mut reader = Reader::from(config);
    while let Some(cmd) = reader.next().await {
        event_tx
            .send(Event::Command(cmd))
            .map_err(|_| AppError::send_error())?;
    }
    Ok(())
}

fn parse_progress(arg: &str) -> Result<&'static str, &'static str> {
    match arg {
        "auto" => Ok("auto"),
        "plain" => Ok("plain"),
        _ => Err("Unknown progress"),
    }
}
