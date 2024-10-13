use std::pin::Pin;
use std::result::Result;
use std::task::{Context, Poll};

use clap::{self, Error as ClapError, Parser};
use futures::future::Either;
use futures::{Stream, StreamExt, TryStreamExt};
use tokio::io::{stdin, AsyncBufReadExt, BufReader, Result as IOResult};
use tokio_stream::wrappers::LinesStream;

use super::command::{Command, Config};

/// Reads commmands from a stream
pub(crate) struct Reader {
    inner: Box<dyn Stream<Item = Result<Command, ClapError>> + Unpin + Send>,
}

impl From<&Config> for Reader {
    fn from(config: &Config) -> Self {
        let command = match config.command.as_ref() {
            Some(command) => Either::Left(tokio_stream::once(Ok(command.clone()))),
            None => Either::Right(tokio_stream::empty()),
        };
        let stream = match config.interactive {
            true => Either::Left(command.chain(stdin_stream())),
            false => Either::Right(command),
        };
        Self { inner: Box::new(stream) }
    }
}

impl Stream for Reader {
    type Item = Result<Command, ClapError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx)
    }
}

/// Parse commands stream from stdin
fn stdin_stream() -> Box<dyn Stream<Item = Result<Command, ClapError>> + Unpin + Send> {
    let stream = LinesStream::new(BufReader::new(stdin()).lines());

    let fix_prefix = |line: String| format!("command {}", line);
    let fix_shell = |line: String| shlex::split(&line).unwrap_or_else(Vec::new);
    let parse_cmd = |args: IOResult<_>| match args {
        Ok(args) => Command::try_parse_from(args),
        Err(_) => Err(ClapError::new(clap::error::ErrorKind::MissingSubcommand)),
    };

    Box::new(stream.map_ok(fix_prefix).map_ok(fix_shell).map(parse_cmd))
}
