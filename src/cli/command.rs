use clap::Parser;

#[derive(Parser, Debug, Default)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Config {
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
