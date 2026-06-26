//! A2UI (Agent to UI) Protocol v1.0 — Core Types
//!
//! 本 crate 提供 A2UI 协议的完整 Rust 类型定义、消息反序列化、
//! Surface 状态机和 Data Model 操作。
//!
//! # 架构
//!
//! ```text
//! message/    — 消息类型定义（服务端→客户端 + 客户端→服务端）
//! component/  — 组件、Catalog、ChildList 类型
//! datamodel/  — DataModel 操作（JSON Pointer 路径解析）
//! schema/     — JSON Schema 验证（Catalog 合规性检查）
//! state/      — Surface 状态机
//! ```
//!
//! # 示例
//!
//! ```rust
//! use a2ui_core::prelude::*;
//!
//! // 解析服务端信封消息
//! let json = r#"{"version":"v1.0","createSurface":{"surfaceId":"s1","catalogId":"basic"}}"#;
//! let envelope = ServerEnvelope::from_json(json).unwrap();
//! match envelope {
//!     ServerEnvelope::V1_0(V1_0ServerMessage::CreateSurface(msg)) => {
//!         assert_eq!(msg.surface_id, "s1");
//!     }
//!     _ => panic!("wrong variant"),
//! }
//! ```
//!
//! [`ServerEnvelope`]: message::ServerEnvelope

mod error;
pub use error::{A2uiError, Result};
pub mod state;
pub mod component;
pub mod datamodel;
pub mod message;
pub mod schema;
pub use schema::CatalogValidator;
pub use message::{ClientEnvelope, ServerEnvelope};
pub use component::ComponentId;
pub use component::Catalog;
pub use datamodel::DataModel;
pub mod prelude;

#[cfg(feature = "embed-assets")]
pub fn load_basic_catalog() -> Result<Catalog> {
    let json = include_str!("assets/catalogs/basic/catalog.json");
    let catalog: Catalog = serde_json::from_str(json)?;
    catalog.validate()?;
    Ok(catalog)
}
