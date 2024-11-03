use std::io::{self, Write};

use env_logger::fmt::Formatter;
use env_logger::{Builder, Target};
use log::{kv::Key, LevelFilter, Record};

use super::command::Config;

pub fn setup_logging(config: &Config) {
    let level = match config.verbosity {
        0 => LevelFilter::Info,
        1 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };
    let formatter = match config.log_format {
        "plain" => plain_formatter,
        "html" => html_formatter,
        _ => default_formatter,
    };
    Builder::new()
        .format(formatter)
        .filter_level(level)
        .target(Target::Stdout)
        .init();
}

fn default_formatter(buf: &mut Formatter, record: &Record) -> io::Result<()> {
    let style = buf.default_level_style(record.level());
    let reset = style.render_reset();
    writeln!(buf, "[{style}{}{reset}] {}", record.level(), record.args())
}

fn html_formatter(_: &mut Formatter, record: &Record) -> io::Result<()> {
    match record.key_values().get(Key::from_str("target")) {
        // NOTE: write! does not seem to preserve the NULL byte
        Some(target) => print!("<div id=\"{target}\">{}</div>\n\0", record.args()),
        None => eprintln!("{}", record.args()),
    }
    Ok(())
}

fn plain_formatter(buf: &mut Formatter, record: &Record) -> io::Result<()> {
    let style: clap::builder::styling::Style = match record.level() {
        log::Level::Info => env_logger::fmt::style::Style::new(),
        level => buf.default_level_style(level),
    };
    let reset = style.render_reset();
    writeln!(buf, "{style}{}{reset}", record.args())
}
