//! A2UI Transport Layer — 传输层抽象
//!
//! 定义 `Transport` trait 和基础实现（JSONL、WebSocket）。

pub mod error;
pub mod jsonl;
pub mod transport;

pub use error::TransportError;
pub use jsonl::{JsonlTransport, JsonlTransportReader, JsonlTransportWriter};
pub use transport::Transport;
