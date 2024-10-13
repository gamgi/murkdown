use super::{
    command::Config,
    types::{AppError, EventRx, EventTx},
};

pub async fn handle(event_tx: EventTx, event_rx: EventRx, config: &Config) -> Result<(), AppError> {
    Ok(())
}
