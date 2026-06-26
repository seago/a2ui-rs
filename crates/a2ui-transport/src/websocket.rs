use crate::{error::TransportResult, Transport};
use a2ui_core::{ClientEnvelope, ServerEnvelope};
use async_trait::async_trait;
use futures_util::stream::StreamExt;
use futures_util::sink::SinkExt;
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
    ws: Option<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>,
}

impl WebSocketTransport {
    pub fn new(url: impl AsRef<str>) -> TransportResult<Self> {
        let url = url::Url::parse(url.as_ref())
            .map_err(|e| crate::TransportError::ConnectionError(format!("invalid URL: {}", e)))?;
        Ok(Self {
            url,
            ws: None,
        })
    }
}

#[async_trait]
impl Transport for WebSocketTransport {
    async fn connect(&mut self) -> TransportResult<()> {
        let (ws, _) = tokio_tungstenite::connect_async(&self.url)
            .await
            .map_err(|e| WebSocketError::ConnectionError(format!("{}", e)))?;
        self.ws = Some(ws);
        Ok(())
    }

    async fn send(&mut self, envelope: ClientEnvelope) -> TransportResult<()> {
        let ws = self.ws.as_mut().ok_or_else(|| {
            WebSocketError::SendError("not connected".into())
        })?;
        let json = serde_json::to_string(&envelope)
            .map_err(|e| WebSocketError::SendError(format!("serialization: {}", e)))?;
        ws.send(tokio_tungstenite::tungstenite::protocol::Message::Text(json))
            .await
            .map_err(|e| WebSocketError::SendError(format!("{}", e)))?;
        Ok(())
    }

    async fn receive(&mut self) -> TransportResult<ServerEnvelope> {
        let ws = self.ws.as_mut().ok_or_else(|| {
            WebSocketError::ReceiveError("not connected".into())
        })?;
        let msg = ws.next().await.ok_or_else(|| {
            WebSocketError::ReceiveError("connection closed".into())
        })?.map_err(|e| {
            WebSocketError::ReceiveError(format!("{}", e))
        })?;
        let text = msg.into_text().map_err(|e| {
            WebSocketError::ReceiveError(format!("expected text frame: {}", e))
        })?;
        let envelope: ServerEnvelope = serde_json::from_str(&text)
            .map_err(|e| WebSocketError::ReceiveError(format!("deserialization: {}", e)))?;
        Ok(envelope)
    }

    async fn close(&mut self) -> TransportResult<()> {
        if let Some(mut ws) = self.ws.take() {
            let _ = ws.close(None).await;
        }
        Ok(())
    }
}

#[cfg(test)]
impl WebSocketTransport {
    /// 检查 WebSocket 是否已连接（仅用于测试）
    fn is_connected(&self) -> bool {
        self.ws.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use a2ui_core::message::{ActionMessage, V1_0ClientMessage};
    use futures_util::stream::StreamExt;

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

    #[test]
    fn test_websocket_send_without_connect_fails() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let mut transport = WebSocketTransport::new("ws://localhost:8080/a2ui").unwrap();
        let envelope = ClientEnvelope::V1_0(V1_0ClientMessage::Action(ActionMessage::event(
            "click", "s1",
        )));
        let result = rt.block_on(async { transport.send(envelope).await });
        assert!(result.is_err());
    }

    #[test]
    fn test_websocket_receive_without_connect_fails() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let mut transport = WebSocketTransport::new("ws://localhost:8080/a2ui").unwrap();
        let result = rt.block_on(async { transport.receive().await });
        assert!(result.is_err());
    }

    #[test]
    fn test_websocket_close_without_connect_succeeds() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let mut transport = WebSocketTransport::new("ws://localhost:8080/a2ui").unwrap();
        let result = rt.block_on(async { transport.close().await });
        assert!(result.is_ok());
    }

    #[test]
    fn test_websocket_connect_stores_state() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // 启动一个简单的 WebSocket 回显服务器用于测试
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("failed to bind test server");
            let addr = listener.local_addr().expect("failed to get local addr");

            let server_handle = tokio::spawn(async move {
                if let Ok((stream, _)) = listener.accept().await {
                    if let Ok(mut ws) = tokio_tungstenite::accept_async(stream).await {
                        // 保持连接打开，直到客户端断开
                        while let Some(Ok(_)) = ws.next().await {}
                    }
                }
            });

            // 给服务器一点时间启动
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

            let url = format!("ws://{}/a2ui", addr);
            let mut transport =
                WebSocketTransport::new(&url).expect("failed to create transport");

            // 连接前，ws 应为 None
            assert!(
                !transport.is_connected(),
                "ws should be None before connect"
            );

            // 执行 connect
            let connect_result = transport.connect().await;
            assert!(connect_result.is_ok(), "connect failed: {:?}", connect_result);

            // 连接后，ws 应为 Some
            assert!(
                transport.is_connected(),
                "ws should be Some after connect"
            );

            // 清理服务器
            server_handle.abort();
            let _ = server_handle.await;
        });
    }
}
