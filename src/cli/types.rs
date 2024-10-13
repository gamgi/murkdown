use clap::error::Error as ClapError;
use thiserror::Error;
use tokio::sync::mpsc::{self, error::SendError};

use super::command::Command;

pub type EventTx = mpsc::UnboundedSender<Event>;
pub type EventRx = mpsc::UnboundedReceiver<Event>;

#[derive(Debug)]
pub enum Event {
    Command { cmd: Result<Command, ClapError> },
}

#[derive(Error, Debug)]
pub enum AppError {
    #[error("could not parse command")]
    ClapError(#[from] ClapError),
    #[error("internal channel error")]
    SendError(#[from] SendError<Event>),
}
