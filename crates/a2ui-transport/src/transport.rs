use async_trait::async_trait;
use a2ui_core::{ClientEnvelope, ServerEnvelope};

/// Transport trait — 消息收发抽象。
#[async_trait]
pub trait Transport: Send + Sync {
    /// 打开传输连接并返回会话 ID。
    async fn connect(&mut self) -> Result<String, crate::TransportError>;

    /// 发送一个客户端消息 envelope。
    async fn send(&mut self, envelope: ClientEnvelope) -> Result<(), crate::TransportError>;

    /// 接收下一个服务端消息 envelope。
    async fn receive(&mut self) -> Result<ServerEnvelope, crate::TransportError>;

    /// 关闭连接。
    async fn close(&mut self) -> Result<(), crate::TransportError>;
}
