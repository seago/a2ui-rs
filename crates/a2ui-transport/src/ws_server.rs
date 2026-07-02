//! WebSocket 服务端 —— 监听端口、接受浏览器连接，并与之交换 A2UI 信封消息。
//!
//! 与 [`crate::WebSocketTransport`]（客户端，主动 connect 一个 URL）相对，
//! [`WebSocketServer`] 负责监听端口、接受连接。每个被接受的连接返回一个
//! [`WebSocketServerConnection`] 句柄，可用于：
//!
//! - **推送** [`ServerEnvelope`]（Agent → Renderer 方向，序列化为 JSON 文本帧）；
//! - **接收** [`ClientEnvelope`]（Renderer → Agent 方向，从文本帧反序列化）。
//!
//! 编解码风格与 [`crate::WebSocketTransport`] 保持一致（`Message::Text` ↔ 信封的
//! JSON 互转），错误使用 [`crate::WebSocketError`]。

use crate::error::TransportResult;
use crate::websocket::WebSocketError;
use a2ui_core::{ClientEnvelope, ServerEnvelope};
use futures_util::sink::SinkExt;
use futures_util::stream::StreamExt;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::WebSocketStream;

/// WebSocket 服务端：绑定地址并接受浏览器连接。
///
/// # 示例
///
/// ```no_run
/// use a2ui_transport::WebSocketServer;
///
/// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
/// // 绑定到随机端口
/// let server = WebSocketServer::bind("127.0.0.1:0").await?;
/// println!("listening on {}", server.local_addr()?);
///
/// // 接受下一个连接，得到一个可 push/receive 的句柄
/// let mut conn = server.accept().await?;
/// # let _ = &mut conn;
/// # Ok(())
/// # }
/// ```
pub struct WebSocketServer {
    listener: TcpListener,
}

impl WebSocketServer {
    /// 绑定到给定地址并开始监听。地址可为 `"127.0.0.1:0"` 以获取随机端口。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let server = a2ui_transport::WebSocketServer::bind("127.0.0.1:0").await?;
    /// # let _ = server;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn bind(addr: impl AsRef<str>) -> TransportResult<Self> {
        let listener = TcpListener::bind(addr.as_ref())
            .await
            .map_err(|e| WebSocketError::ConnectionError(format!("bind failed: {}", e)))?;
        tracing::info!(
            "WebSocket server listening on {:?}",
            listener.local_addr().ok()
        );
        Ok(Self { listener })
    }

    /// 返回实际监听的本地地址（绑定随机端口时用于获取分配到的端口）。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let server = a2ui_transport::WebSocketServer::bind("127.0.0.1:0").await?;
    /// let addr = server.local_addr()?;
    /// assert_eq!(addr.ip().to_string(), "127.0.0.1");
    /// # Ok(())
    /// # }
    /// ```
    pub fn local_addr(&self) -> TransportResult<SocketAddr> {
        self.listener.local_addr().map_err(|e| {
            WebSocketError::ConnectionError(format!("local_addr failed: {}", e)).into()
        })
    }

    /// 接受下一个客户端连接，完成 WebSocket 握手后返回连接句柄。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let server = a2ui_transport::WebSocketServer::bind("127.0.0.1:0").await?;
    /// let conn = server.accept().await?;
    /// # let _ = conn;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn accept(&self) -> TransportResult<WebSocketServerConnection> {
        let (stream, peer) = self
            .listener
            .accept()
            .await
            .map_err(|e| WebSocketError::ConnectionError(format!("accept failed: {}", e)))?;
        let ws = tokio_tungstenite::accept_async(stream)
            .await
            .map_err(|e| WebSocketError::ConnectionError(format!("ws handshake failed: {}", e)))?;
        tracing::info!("WebSocket connection accepted from {}", peer);
        Ok(WebSocketServerConnection { ws, peer })
    }
}

/// 一个已接受的 WebSocket 连接句柄。
///
/// 服务端视角的命名（与 [`crate::Transport`] 的传输通道视角相反）：
/// - [`push`](Self::push): 向客户端发送 [`ServerEnvelope`]（Agent → Renderer）；
/// - [`receive`](Self::receive): 从客户端接收 [`ClientEnvelope`]（Renderer → Agent）。
pub struct WebSocketServerConnection {
    ws: WebSocketStream<TcpStream>,
    peer: SocketAddr,
}

impl WebSocketServerConnection {
    /// 返回对端（客户端）地址。
    pub fn peer_addr(&self) -> SocketAddr {
        self.peer
    }

    /// 向客户端推送一个 [`ServerEnvelope`]，序列化为 JSON 文本帧。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// # use a2ui_core::message::{V1_0ServerMessage, capabilities::Capabilities};
    /// # use a2ui_core::ServerEnvelope;
    /// # async fn run(mut conn: a2ui_transport::WebSocketServerConnection) -> Result<(), Box<dyn std::error::Error>> {
    /// let env = ServerEnvelope::V1_0(V1_0ServerMessage::Capabilities(Capabilities {
    ///     version: "1.0".to_string(),
    ///     features: vec!["basic".to_string()],
    /// }));
    /// conn.push(env).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn push(&mut self, envelope: ServerEnvelope) -> TransportResult<()> {
        let json = serde_json::to_string(&envelope)
            .map_err(|e| WebSocketError::SendError(format!("serialization: {}", e)))?;
        self.ws
            .send(Message::Text(json))
            .await
            .map_err(|e| WebSocketError::SendError(format!("{}", e)))?;
        Ok(())
    }

    /// 从客户端接收下一个 [`ClientEnvelope`]（阻塞直到收到一个数据帧或连接关闭）。
    ///
    /// 支持 `Text` 与 `Binary` 帧（均按 UTF-8 JSON 解析）。控制帧（Ping/Pong）
    /// 由 tungstenite 自动处理并被跳过；收到 Close 帧或连接关闭时返回错误。
    pub async fn receive(&mut self) -> TransportResult<ClientEnvelope> {
        loop {
            let msg = self
                .ws
                .next()
                .await
                .ok_or_else(|| WebSocketError::ReceiveError("connection closed".into()))?
                .map_err(|e| WebSocketError::ReceiveError(format!("{}", e)))?;

            let text = match msg {
                Message::Text(s) => s,
                Message::Binary(data) => String::from_utf8(data).map_err(|e| {
                    WebSocketError::ReceiveError(format!(
                        "binary frame contains non-UTF-8 data: {}",
                        e
                    ))
                })?,
                // Ping/Pong 由 tungstenite 自动回应，跳过继续等待数据帧
                Message::Ping(_) | Message::Pong(_) => continue,
                Message::Close(_) => {
                    return Err(
                        WebSocketError::ReceiveError("peer closed connection".into()).into(),
                    );
                }
                _ => {
                    return Err(WebSocketError::ReceiveError("unexpected frame type".into()).into());
                }
            };

            let envelope: ClientEnvelope = serde_json::from_str(&text)
                .map_err(|e| WebSocketError::ReceiveError(format!("deserialization: {}", e)))?;
            return Ok(envelope);
        }
    }

    /// 关闭连接。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// # async fn run(mut conn: a2ui_transport::WebSocketServerConnection) -> Result<(), Box<dyn std::error::Error>> {
    /// conn.close().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn close(&mut self) -> TransportResult<()> {
        if let Err(e) = self.ws.close(None).await {
            tracing::warn!("websocket server connection close error: {}", e);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Transport, WebSocketTransport};
    use a2ui_core::message::{
        capabilities::Capabilities, ActionMessage, V1_0ClientMessage, V1_0ServerMessage,
    };

    /// 服务端 push 的 ServerEnvelope，客户端能收到；
    /// 客户端 send 的 ClientEnvelope，服务端能收到。
    #[tokio::test]
    async fn test_server_push_and_receive() {
        // 绑定随机端口，避免测试间端口冲突
        let server = WebSocketServer::bind("127.0.0.1:0")
            .await
            .expect("bind failed");
        let addr = server.local_addr().expect("local_addr failed");
        let url = format!("ws://{}/a2ui", addr);

        // 服务端任务：接受一个连接，push 一个 Capabilities，再接收一个 Action
        let server_task = tokio::spawn(async move {
            let mut conn = server.accept().await.expect("accept failed");

            let env = ServerEnvelope::V1_0(V1_0ServerMessage::Capabilities(Capabilities {
                version: "1.0".to_string(),
                features: vec!["basic".to_string()],
            }));
            conn.push(env).await.expect("push failed");

            // 接收客户端发来的 ClientEnvelope
            conn.receive().await.expect("server receive failed")
        });

        // 客户端：连上、收到服务端推送、再发一个 Action
        let mut client = WebSocketTransport::new(&url)
            .expect("create client")
            .without_reconnect();
        client.connect().await.expect("client connect failed");

        // 验证：服务端 push 的 ServerEnvelope，客户端能收到
        let received = client.receive().await.expect("client receive failed");
        match received {
            ServerEnvelope::V1_0(V1_0ServerMessage::Capabilities(caps)) => {
                assert_eq!(caps.features, vec!["basic".to_string()]);
            }
            other => panic!("unexpected envelope: {:?}", other),
        }

        // 客户端 send 一个 Action
        let action = ClientEnvelope::V1_0(V1_0ClientMessage::Action(ActionMessage::event(
            "submit", "s1",
        )));
        client.send(action).await.expect("client send failed");

        // 验证：客户端 send 的 ClientEnvelope，服务端能收到
        let server_got = server_task.await.expect("server task join failed");
        match server_got {
            ClientEnvelope::V1_0(V1_0ClientMessage::Action(a)) => {
                assert_eq!(a.name, "submit");
                assert_eq!(a.surface_id, "s1");
            }
            other => panic!("unexpected client envelope: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_bind_random_port_and_local_addr() {
        let server = WebSocketServer::bind("127.0.0.1:0")
            .await
            .expect("bind failed");
        let addr = server.local_addr().expect("local_addr failed");
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
        assert_ne!(addr.port(), 0, "should be assigned a concrete port");
    }
}
