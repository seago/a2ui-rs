//! A2UI Transport Layer — 传输层抽象
//!
//! 定义 `Transport` trait 和基础实现（JSONL、WebSocket）。

pub mod error;
pub mod transport;
pub mod jsonl;

pub use error::TransportError;
pub use transport::Transport;
pub use jsonl::JsonlTransport;
