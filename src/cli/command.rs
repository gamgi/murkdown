use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use futures::StreamExt;

use super::{
    reader::Reader,
    types::{AppError, Event, EventTx, Output},
};

#[derive(Parser, Debug, Default)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Config {
    /// Interactive mode
    #[clap(long, global = true)]
    pub interactive: bool,

    /// Output path or target
    ///
    /// [default: ./build, possible values: stdout, <PATH>]
    #[clap(
        short,
        long,
        default_value = "./build",
        value_parser = parse_out,
        hide_default_value = true
    )]
    #[clap(global = true)]
    pub output: Option<Output>,

    /// Progress output format
    ///
    /// [default: auto, possible values: auto, plain]
    #[clap(long, default_value = "auto", value_parser = parse_progress, hide_default_value = true)]
    #[clap(global = true)]
    pub progress: &'static str,

    /// Increase level of verbosity
    #[clap(short, action = clap::ArgAction::Count, global = true)]
    pub verbosity: u8,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Parser, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(Ord, PartialOrd))]
pub(crate) enum Command {
    /// Build a graph
    Graph {
        /// Graph type
        #[arg(value_enum, value_name = "TYPE")]
        graph_type: GraphType,

        /// Input paths
        #[clap(value_name = "PATH")]
        #[arg(default_values_t = [".".to_string()])]
        paths: Vec<String>,
    },
    /// Load content into memory
    Load {
        /// Input paths
        paths: Vec<String>,
    },
    /// Build sources
    Build {
        /// At what semantic level(s) should output be split to files
        #[clap(short, long = "split", value_name = "SPLIT")]
        #[arg(default_values_t = ["ROOT".to_string(), "DOCUMENT".to_string()])]
        splits: Vec<String>,

        /// Input paths
        #[clap(value_name = "PATH")]
        #[arg(default_values_t = [".".to_string()])]
        paths: Vec<String>,
    },
    /// Exit interactive mode
    #[clap(hide = true)]
    Exit,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub(crate) enum GraphType {
    /// Dependency graph
    Dependencies,
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

fn parse_out(arg: &str) -> Result<Output, &'static str> {
    match arg {
        uri if uri.starts_with("stdout") => Ok(Output::Stdout),
        uri if uri.starts_with("file://") || !uri.contains("://") => {
            let path: PathBuf = arg.trim_start_matches("file://").into();
            if path.is_file() {
                Err("Output must be a directory")
            } else {
                Ok(Output::Path(path))
            }
        }
        _ => Err("Unknown scheme"),
    }
}

fn parse_progress(arg: &str) -> Result<&'static str, &'static str> {
    match arg {
        "auto" => Ok("auto"),
        "plain" => Ok("plain"),
        _ => Err("Unknown progress"),
    }
}
