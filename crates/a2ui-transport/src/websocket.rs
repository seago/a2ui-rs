use crate::{error::TransportResult, Transport};
use a2ui_core::{ClientEnvelope, ServerEnvelope};
use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WebSocketError {
    #[error("connection error: {0}")]
    ConnectionError(String),

    #[error("send error: {0}")]
    SendError(String),

    #[error("receive error: {0}")]
    ReceiveError(String),
}

impl From<WebSocketError> for crate::error::TransportError {
    fn from(e: WebSocketError) -> Self {
        match e {
            WebSocketError::ConnectionError(msg) => Self::ConnectionError(msg),
            WebSocketError::SendError(msg) => Self::SendError(msg),
            WebSocketError::ReceiveError(msg) => Self::ReceiveError(msg),
        }
    }
}

pub struct WebSocketTransport {
    url: url::Url,
    // WebSocket 连接在 connect 时建立
}

impl WebSocketTransport {
    pub fn new(url: impl AsRef<str>) -> TransportResult<Self> {
        let url = url::Url::parse(url.as_ref())
            .map_err(|e| crate::TransportError::ConnectionError(format!("invalid URL: {}", e)))?;
        Ok(Self { url })
    }
}

#[async_trait]
impl Transport for WebSocketTransport {
    async fn connect(&mut self) -> TransportResult<()> {
        let (_ws, _) = tokio_tungstenite::connect_async(&self.url)
            .await
            .map_err(|e| WebSocketError::ConnectionError(format!("{}", e)))?;
        Ok(())
    }

    async fn send(&mut self, envelope: ClientEnvelope) -> TransportResult<()> {
        let json = serde_json::to_string(&envelope)
            .map_err(|e| WebSocketError::SendError(format!("serialization: {}", e)))?;
        // 发送 WebSocket 文本帧（需要连接状态管理，此处为基础实现）
        let _ = json;
        Ok(())
    }

    async fn receive(&mut self) -> TransportResult<ServerEnvelope> {
        // 接收 WebSocket 文本帧并反序列化（需要连接状态管理，此处为基础实现）
        Err(WebSocketError::ReceiveError("not implemented".into()).into())
    }

    async fn close(&mut self) -> TransportResult<()> {
        // 关闭 WebSocket 连接（需要连接状态管理，此处为基础实现）
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websocket_url_parse() {
        let transport = WebSocketTransport::new("ws://localhost:8080/a2ui");
        assert!(transport.is_ok());
    }

    #[test]
    fn test_websocket_invalid_url() {
        let result = WebSocketTransport::new("not-a-url");
        assert!(result.is_err());
    }
}
