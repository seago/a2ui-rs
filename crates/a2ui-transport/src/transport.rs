use crate::error::TransportResult;
use a2ui_core::{ClientEnvelope, ServerEnvelope};
#[allow(unused_imports)]
use serde::{Deserialize, Serialize};

/// Transport trait — 所有传输实现必须满足此契约
#[async_trait::async_trait]
pub trait Transport: Send {
    /// 建立连接
    async fn connect(&mut self) -> TransportResult<()>;

    /// 发送服务端信封消息
    async fn send(&mut self, envelope: ServerEnvelope) -> TransportResult<()>;

    /// 接收客户端信封消息
    async fn receive(&mut self) -> TransportResult<ClientEnvelope>;

    /// 关闭连接
    async fn close(&mut self) -> TransportResult<()>;
}
