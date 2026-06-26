use crate::error::TransportResult;
use a2ui_core::message::Capabilities;
use a2ui_core::{ClientEnvelope, ServerEnvelope};
use async_trait::async_trait;

/// Transport trait — 所有传输实现必须满足此契约
///
/// 命名约定（从传输通道视角）：
/// - `send`: 写入输出流（Renderer → Agent，即 ClientEnvelope）
/// - `receive`: 读取输入流（Agent → Renderer，即 ServerEnvelope）
#[async_trait]
pub trait Transport: Send {
    /// 建立连接
    async fn connect(&mut self) -> TransportResult<()>;

    /// 执行能力协商握手，交换客户端和服务端能力描述
    async fn handshake(&mut self, capabilities: Capabilities) -> TransportResult<Capabilities>;

    /// 发送客户端信封消息到 Agent（写入输出流）
    async fn send(&mut self, envelope: ClientEnvelope) -> TransportResult<()>;

    /// 从 Agent 接收服务端信封消息（从输入流读取）
    async fn receive(&mut self) -> TransportResult<ServerEnvelope>;

    /// 关闭连接
    async fn close(&mut self) -> TransportResult<()>;
}
