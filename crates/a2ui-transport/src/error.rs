use thiserror::Error;

/// Transport layer errors.
#[derive(Debug, Error)]
pub enum TransportError {
    #[error("connection error: {0}")]
    ConnectionError(String),

    #[error("send error: {0}")]
    SendError(String),

    #[error("receive error: {0}")]
    ReceiveError(String),
}
