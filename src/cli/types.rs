use clap::error::Error as ClapError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("could not parse command")]
    ClapError(#[from] ClapError),
}
