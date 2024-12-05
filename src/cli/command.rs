use std::{fmt::Display, path::PathBuf};

use clap::{Parser, ValueEnum};
use futures::StreamExt;

use super::{
    reader::Reader,
    types::{AppError, Event, EventTx, Output},
};

#[derive(Parser, Debug, Default)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Config {
    /// Output format
    #[clap(short, long = "format", default_value = "html", value_parser = clap::builder::NonEmptyStringValueParser::new(), global = true)]
    pub format: Option<String>,

    /// Interactive mode
    #[clap(long, global = true)]
    pub interactive: bool,

    /// Output path or target
    ///
    /// [default: ./build, possible values: stdout, <PATH>]
    #[clap(short, long, value_parser = parse_out, hide_default_value = true, global = true)]
    // NOTE: default value set in `Config::validate`
    pub output: Option<Output>,

    /// Log format
    ///
    /// [default: auto, possible values: auto, html, plain]
    #[clap(long = "log", default_value = "auto", value_parser = parse_log_format, require_equals = true, hide_default_value = true)]
    #[clap(global = true)]
    pub log_format: &'static str,

    /// Increase level of verbosity
    #[clap(short, action = clap::ArgAction::Count, global = true)]
    pub verbosity: u8,

    #[command(subcommand)]
    pub command: Option<Command>,
}

// NOTE: not using `Subcommand` since we need `Parser::try_parse_from`
#[derive(Parser, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(Ord, PartialOrd))]
pub(crate) enum Command {
    /// Build a graph
    Graph {
        /// Additional root block headers
        #[clap(long = "headers", value_name = "HEADERS")]
        headers: Option<String>,

        /// Graph type
        #[arg(value_enum, value_name = "TYPE")]
        graph_type: GraphType,

        /// Input paths or data URLs
        #[clap(value_name = "PATH")]
        #[arg(default_values_t = [".".to_string()])]
        paths: Vec<String>,
    },
    /// Index content into memory
    Index {
        /// Input paths or data URLs
        paths: Vec<String>,
    },
    /// Build sources
    Build {
        /// Additional root block headers
        #[clap(long = "as", value_name = "HEADERS")]
        headers: Option<String>,

        /// At what semantic level(s) should output be split to files
        #[clap(short, long = "split", value_name = "SPLIT")]
        #[arg(default_values_t = ["ROOT".to_string(), "DOCUMENT".to_string()])]
        splits: Vec<String>,

        /// Input paths or data URLs
        #[clap(value_name = "PATH")]
        #[arg(default_values_t = [".".to_string()])]
        paths: Vec<String>,
    },
    /// Exit interactive mode
    #[clap(hide = true)]
    Exit,
}

impl Config {
    pub fn defaults(mut self) -> Self {
        // workaround for `default_value_ifs` issue in clap
        match (self.log_format, self.output.as_ref()) {
            ("html", None) => self.output = Some(Output::StdOutLog),
            ("html", Some(Output::StdOut)) => self.output = Some(Output::StdOutLog),
            (_, None) => self.output = Some(Output::Path(PathBuf::from("./build"))),
            _ => {}
        };
        self
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, ValueEnum, PartialOrd, Ord)]
pub(crate) enum GraphType {
    /// Dependency graph
    Dependencies,
}

impl Display for GraphType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphType::Dependencies => write!(f, "dependencies"),
        }
    }
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
        uri if uri.starts_with("stdout") => Ok(Output::StdOut),
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

fn parse_log_format(arg: &str) -> Result<&'static str, &'static str> {
    match arg {
        "auto" => Ok("auto"),
        "html" => Ok("html"),
        "plain" => Ok("plain"),
        _ => Err("Unknown progress"),
    }
}
