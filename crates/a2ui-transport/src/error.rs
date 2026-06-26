use a2ui_core::A2uiError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Send error: {0}")]
    SendError(String),

    #[error("Receive error: {0}")]
    ReceiveError(String),

    #[error("Core error: {0}")]
    CoreError(#[from] A2uiError),
}

pub type TransportResult<T> = Result<T, TransportError>;
