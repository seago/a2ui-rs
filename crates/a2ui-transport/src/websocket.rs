use crate::{error::TransportResult, Transport, TransportError};
use a2ui_core::message::Capabilities;
use a2ui_core::{
    message::{V1_0ClientMessage, V1_0ServerMessage},
    ClientEnvelope, ServerEnvelope,
};
use async_trait::async_trait;
use futures_util::sink::SinkExt;
use futures_util::stream::StreamExt;
use thiserror::Error;
use tokio_tungstenite::tungstenite::protocol::Message;

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
    ws: Option<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    /// 自动重连配置（None 表示不自动重连）
    pub max_reconnect_attempts: Option<usize>,
    /// 重连间隔（毫秒）
    pub reconnect_delay_ms: u64,
}

impl WebSocketTransport {
    pub fn new(url: impl AsRef<str>) -> TransportResult<Self> {
        let url = url::Url::parse(url.as_ref())
            .map_err(|e| crate::TransportError::ConnectionError(format!("invalid URL: {}", e)))?;
        Ok(Self {
            url,
            ws: None,
            max_reconnect_attempts: Some(3),
            reconnect_delay_ms: 1000,
        })
    }

    /// 禁用自动重连
    pub fn without_reconnect(mut self) -> Self {
        self.max_reconnect_attempts = None;
        self
    }

    /// 设置重连参数
    pub fn with_reconnect(mut self, max_attempts: usize, delay_ms: u64) -> Self {
        self.max_reconnect_attempts = Some(max_attempts);
        self.reconnect_delay_ms = delay_ms;
        self
    }
}

#[async_trait]
impl Transport for WebSocketTransport {
    async fn connect(&mut self) -> TransportResult<()> {
        tracing::info!("connecting to websocket: {}", self.url);
        let (ws, _) = tokio_tungstenite::connect_async(&self.url)
            .await
            .map_err(|e| WebSocketError::ConnectionError(format!("{}", e)))?;
        self.ws = Some(ws);
        tracing::info!("websocket connected: {}", self.url);
        Ok(())
    }

    async fn handshake(&mut self, capabilities: Capabilities) -> TransportResult<Capabilities> {
        // 发送客户端能力描述
        let client_msg = V1_0ClientMessage::Capabilities(capabilities);
        self.send(ClientEnvelope::v1_0(client_msg)).await?;

        // 接收服务端能力描述
        let envelope = self.receive().await?;
        match envelope {
            ServerEnvelope::V1_0(V1_0ServerMessage::Capabilities(server_caps)) => Ok(server_caps),
            _ => Err(TransportError::ConnectionError(
                "expected capabilities message during handshake".to_string(),
            )),
        }
    }

    async fn send(&mut self, envelope: ClientEnvelope) -> TransportResult<()> {
        let ws = self
            .ws
            .as_mut()
            .ok_or_else(|| WebSocketError::SendError("not connected".into()))?;
        let json = envelope
            .to_json()
            .map_err(|e| WebSocketError::SendError(format!("serialization: {}", e)))?;
        ws.send(tokio_tungstenite::tungstenite::protocol::Message::Text(
            json,
        ))
        .await
        .map_err(|e| WebSocketError::SendError(format!("{}", e)))?;
        Ok(())
    }

    async fn receive(&mut self) -> TransportResult<ServerEnvelope> {
        // 自动重连：ws 为 None 时尝试重新连接
        if self.ws.is_none() {
            if let Some(max_attempts) = self.max_reconnect_attempts {
                for attempt in 1..=max_attempts {
                    tracing::warn!("WS reconnecting (attempt {}/{})", attempt, max_attempts);
                    tokio::time::sleep(std::time::Duration::from_millis(self.reconnect_delay_ms))
                        .await;
                    if self.connect().await.is_ok() {
                        tracing::info!("WS reconnected");
                        break;
                    }
                }
            }
        }

        loop {
            let ws = self
                .ws
                .as_mut()
                .ok_or_else(|| WebSocketError::ReceiveError("not connected".into()))?;
            let msg = ws
                .next()
                .await
                .ok_or_else(|| {
                    // 连接关闭时清除 ws，触发下次自动重连
                    self.ws = None;
                    WebSocketError::ReceiveError("connection closed".into())
                })?
                .map_err(|e| WebSocketError::ReceiveError(format!("{}", e)))?;

            // 支持 Text 和 Binary 帧（Binary 帧也尝试按 UTF-8 JSON 解析）
            let text = match msg {
                Message::Text(s) => s,
                Message::Binary(data) => String::from_utf8(data).map_err(|e| {
                    WebSocketError::ReceiveError(format!(
                        "binary frame contains non-UTF-8 data: {}",
                        e
                    ))
                })?,
                Message::Ping(_) | Message::Pong(_) => {
                    // tungstenite 会自动回复 Pong，但 Ping/Pong 帧仍会从
                    // Stream 中产出——跳过继续等待数据帧（服务端心跳很常见）
                    continue;
                }
                Message::Close(_) => {
                    return Err(
                        WebSocketError::ReceiveError("peer closed connection".into()).into(),
                    );
                }
                _ => {
                    return Err(WebSocketError::ReceiveError("unexpected frame type".into()).into());
                }
            };

            let envelope = ServerEnvelope::from_json(&text)
                .map_err(|e| WebSocketError::ReceiveError(format!("deserialization: {}", e)))?;
            return Ok(envelope);
        }
    }

    async fn close(&mut self) -> TransportResult<()> {
        if let Some(mut ws) = self.ws.take() {
            if let Err(e) = ws.close(None).await {
                tracing::warn!("websocket close error: {}", e);
            }
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
        let envelope = ClientEnvelope::v1_0(V1_0ClientMessage::Action(ActionMessage::event(
            "click", "s1", "btn",
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
            let mut transport = WebSocketTransport::new(&url).expect("failed to create transport");

            // 连接前，ws 应为 None
            assert!(
                !transport.is_connected(),
                "ws should be None before connect"
            );

            // 执行 connect
            let connect_result = transport.connect().await;
            assert!(
                connect_result.is_ok(),
                "connect failed: {:?}",
                connect_result
            );

            // 连接后，ws 应为 Some
            assert!(transport.is_connected(), "ws should be Some after connect");

            // 清理服务器
            server_handle.abort();
            let _ = server_handle.await;
        });
    }

    #[test]
    fn test_websocket_receive_skips_ping_frames() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // 服务端先发 Ping（keepalive 心跳，极为常见），再发正常消息。
            // receive() 必须跳过 Ping 而不是把它当作致命错误。
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("failed to bind test server");
            let addr = listener.local_addr().expect("failed to get local addr");

            let server_handle = tokio::spawn(async move {
                if let Ok((stream, _)) = listener.accept().await {
                    if let Ok(mut ws) = tokio_tungstenite::accept_async(stream).await {
                        let _ = ws
                            .send(tokio_tungstenite::tungstenite::protocol::Message::Ping(
                                vec![1, 2, 3],
                            ))
                            .await;
                        let response = r#"{"version":"v1.0","capabilities":{"version":"1.0","features":["basic"]}}"#;
                        let _ = ws
                            .send(tokio_tungstenite::tungstenite::protocol::Message::Text(
                                response.into(),
                            ))
                            .await;
                        // 保持连接直到客户端收完
                        while let Some(Ok(_)) = ws.next().await {}
                    }
                }
            });

            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

            let url = format!("ws://{}/a2ui", addr);
            let mut transport = WebSocketTransport::new(&url).expect("failed to create transport");
            transport.connect().await.unwrap();

            let envelope = transport
                .receive()
                .await
                .expect("Ping frame must be skipped, not treated as a fatal error");
            match envelope {
                ServerEnvelope::V1_0(V1_0ServerMessage::Capabilities(c)) => {
                    assert_eq!(c.features, vec!["basic"]);
                }
                _ => panic!("expected capabilities message after skipping ping"),
            }

            server_handle.abort();
            let _ = server_handle.await;
        });
    }

    #[test]
    fn test_websocket_handshake() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // 启动一个简单的 WebSocket 服务器，接收握手并回复 capabilities
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("failed to bind test server");
            let addr = listener.local_addr().expect("failed to get local addr");

            let server_handle = tokio::spawn(async move {
                if let Ok((stream, _)) = listener.accept().await {
                    if let Ok(mut ws) = tokio_tungstenite::accept_async(stream).await {
                        // 接收客户端发送的 capabilities 消息（只需处理第一条）
                        if let Some(Ok(msg)) = ws.next().await {
                            let _text = msg.into_text().unwrap();
                            // 回复服务端 capabilities
                            let response =
                                r#"{"version":"v1.0","capabilities":{"version":"1.0","features":["basic"]}}"#;
                            let _ = ws
                                .send(tokio_tungstenite::tungstenite::protocol::Message::Text(
                                    response.into(),
                                ))
                                .await;
                        }
                    }
                }
            });

            // 给服务器一点时间启动
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

            let url = format!("ws://{}/a2ui", addr);
            let mut transport =
                WebSocketTransport::new(&url).expect("failed to create transport");

            transport.connect().await.unwrap();

            let client_caps = Capabilities {
                version: "1.0".to_string(),
                features: vec!["tui".to_string()],
            };
            let server_caps = transport.handshake(client_caps).await.unwrap();

            assert_eq!(server_caps.version, "1.0");
            assert_eq!(server_caps.features, vec!["basic"]);

            // 清理服务器
            server_handle.abort();
            let _ = server_handle.await;
        });
    }
}
