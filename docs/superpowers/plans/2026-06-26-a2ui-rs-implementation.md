# A2UI-RS 完整实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 根据 ARCHITECTURE.md 完整实现 A2UI (Agent to UI) Protocol v1.0 的 Rust 工作空间，包含 a2ui-core、a2ui-transport、a2ui-renderer、a2ui-renderer-tui 和 a2ui-cli 五个 crate，全部通过 TDD 模式开发。

**Architecture:** 严格遵循 ARCHITECTURE.md 的 4 阶段路线图。Phase 1 构建协议核心类型（a2ui-core），Phase 2 构建渲染器抽象层（a2ui-renderer），Phase 3 实现 TUI 渲染器（a2ui-renderer-tui），Phase 4 构建传输层和 CLI（a2ui-transport + a2ui-cli）。每个 Phase 可独立编译测试。

**Tech Stack:** Rust 2021 edition, serde + serde_json, thiserror, tokio (async), ratatui + crossterm (TUI), clap (CLI)

## Global Constraints

- **Rust Edition:** 2021
- **TDD 强制要求:** 红 → 绿 → 重构，不允许先写实现再补测试
- **错误处理:** 统一使用 thiserror，禁止业务逻辑中裸 panic!/unwrap()
- **异步接口:** 所有公共 API 使用 async fn
- **序列化:** serde + serde_json 统一处理
- **文档:** 所有公共 API 必须有 /// 文档注释 + 可运行示例
- **a2ui-core 唯一依赖 serde_json:** 下游 crate 只依赖 a2ui-core 的 Rust 类型
- **反序列化:** 协议消息使用 deny_unknown_fields
- **标识符校验:** ComponentId 等必须通过 Unicode UAX #31 校验
- **Surface 上限:** 最大并发 100，单 Surface 最大组件数 1000

## 已完成任务清单

以下任务已在前期执行中完成，**无需重复实施**。Phase 5 仅覆盖未完成的缺口。

| Phase | 任务 | 状态 |
|-------|------|------|
| Phase 1 | Task 1-14: Workspace + a2ui-core 协议核心 | ✅ 完成 |
| Phase 2 | Task 16-25: a2ui-renderer 渲染器抽象层 | ✅ 完成 |
| Phase 3 | Task 26-38: TUI crate + TuiRenderer 核心 + WidgetMapper | ✅ 完成 |
| Phase 3 | Task 40-45: InputHandler + FocusManager + 端到端测试 | ✅ 完成 |
| Phase 4 | Task 46-49: a2ui-transport crate + JsonlTransport | ✅ 完成 |
| Phase 4 | Task 50-55: a2ui-cli CLI 入口 + 消息处理循环 | ✅ 完成 |
| **缺口** | **Task 39: TUI render() 帧渲染（原实现为占位符）** | ❌ 待补全 |
| **缺口** | **Task 46 续: WebSocketTransport** | ❌ 待实现 |
| **缺口** | **formatString 插值解析** | ❌ 待实现 |
| **缺口** | **响应性渲染管线（DependencyGraph 接入 render）** | ❌ 待实现 |

---

# Phase 1: Workspace 搭建 + a2ui-core 协议核心

**目标:** 建立 Cargo workspace，完成 a2ui-core 中所有协议类型定义、消息反序列化、Surface 状态机和 DataModel。

## 文件结构

```
a2ui-rs/
├── Cargo.toml
├── crates/a2ui-core/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── error.rs
│       ├── message/{mod.rs, envelope.rs, server_to_client.rs, client_to_server.rs}
│       ├── component/{mod.rs, component.rs, catalog.rs, child_list.rs}
│       ├── datamodel/{mod.rs, model.rs}
│       ├── schema/{mod.rs, common_types.rs, server_to_client.rs, catalog_schema.rs}
│       └── state/{mod.rs, surface_state.rs}
│       └── assets/catalogs/basic/catalog.json
└── tests/integration/a2ui_core_tests.rs
```

---

## Task 1: 创建 Workspace Cargo.toml

**Files:**
- Create: `Cargo.toml`

**Interfaces:**
- Consumes: 无
- Produces: 工作空间根配置

- [ ] **Step 1: 创建 workspace Cargo.toml**

```toml
[workspace]
resolver = "2"
members = ["crates/a2ui-core"]
exclude = [
    "crates/a2ui-transport", "crates/a2ui-renderer",
    "crates/a2ui-renderer-tui", "crates/a2ui-renderer-gui",
    "crates/a2ui-renderer-web", "crates/a2ui-cli",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.75"
license = "MIT"
authors = ["yufeng108"]

[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "1.0"
tokio = { version = "1.35", features = ["full"] }
async-trait = "0.1"
```

- [ ] **Step 2: 验证 workspace 可以解析**

```bash
cargo metadata --no-deps --format-version 1
```

Expected: 输出包含 `a2ui-core` 的 JSON metadata

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "chore: create workspace Cargo.toml"
```

---

## Task 2: 创建 a2ui-core Crate 骨架

**Files:**
- Create: `crates/a2ui-core/Cargo.toml`
- Create: `crates/a2ui-core/src/lib.rs`

**Interfaces:**
- Consumes: workspace Cargo.toml
- Produces: a2ui-core crate 基础结构

- [ ] **Step 1: 创建 a2ui-core Cargo.toml**

```toml
[package]
name = "a2ui-core"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true
description = "A2UI Protocol v1.0 core types"
repository = "https://github.com/yufeng108/a2ui-rs"

[dependencies]
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
```

- [ ] **Step 2: 创建 lib.rs 骨架**

```rust
//! A2UI (Agent to UI) Protocol v1.0 — Core Types
//!
//! 本 crate 提供 A2UI 协议的完整 Rust 类型定义。

pub mod error;
pub mod message;
pub mod component;
pub mod datamodel;
pub mod schema;
pub mod state;

pub mod prelude {
    pub use crate::error::{A2uiError, Result};
    pub use crate::message::{ServerEnvelope, ClientEnvelope};
    pub use crate::component::{Component, Catalog, ComponentId, ChildList};
    pub use crate::component::component::{ComponentCommon, AccessibilityAttributes};
    pub use crate::datamodel::DataModel;
    pub use crate::state::{SurfaceState, StateMachine};
    pub use crate::schema::CatalogValidator;
}
```

- [ ] **Step 3: 验证编译**

```bash
cargo build -p a2ui-core
```

- [ ] **Step 4: Commit**

```bash
git add crates/a2ui-core/Cargo.toml crates/a2ui-core/src/lib.rs
git commit -m "feat: create a2ui-core crate skeleton"
```

---

## Task 3: 定义统一错误类型 (error.rs)

**Files:**
- Create: `crates/a2ui-core/src/error.rs`
- Modify: `crates/a2ui-core/src/lib.rs`

**Interfaces:**
- Consumes: 无
- Produces: `A2uiError` 枚举, `Result<T>` 类型别名

- [ ] **Step 1: 编写失败的测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_surface_not_found_display() {
        let err = A2uiError::SurfaceNotFound("surface-1".into());
        assert_eq!(err.to_string(), "Surface not found: surface-1");
    }

    #[test]
    fn test_surface_id_conflict_display() {
        let err = A2uiError::SurfaceIdConflict("s1".into());
        assert_eq!(err.to_string(), "Surface ID already exists: s1");
    }

    #[test]
    fn test_invalid_component_id_display() {
        let err = A2uiError::InvalidComponentId("bad id!".into());
        assert_eq!(err.to_string(), "Invalid component ID: bad id!");
    }

    #[test]
    fn test_component_not_found_display() {
        let err = A2uiError::ComponentNotFound("btn".into());
        assert_eq!(err.to_string(), "Component reference not found: btn");
    }

    #[test]
    fn test_catalog_not_found_display() {
        let err = A2uiError::CatalogNotFound("https://example.com/catalog".into());
        assert_eq!(err.to_string(), "Catalog not found: https://example.com/catalog");
    }

    #[test]
    fn test_function_not_available_display() {
        let err = A2uiError::FunctionNotAvailable("myFunc".into());
        assert_eq!(err.to_string(), "Function not available: myFunc");
    }

    #[test]
    fn test_deserialization_error_wraps_serde() {
        let serde_err: serde_json::Error =
            serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let err = A2uiError::Deserialization(serde_err);
        assert!(err.to_string().contains("Deserialization error"));
    }

    #[test]
    fn test_validation_error_display() {
        let err = A2uiError::ValidationError {
            message: "field required".into(),
            component_id: "email".into(),
            check_index: 0,
        };
        assert!(err.to_string().contains("field required"));
        assert!(err.to_string().contains("email"));
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

```bash
cargo test -p a2ui-core error::tests --no-run 2>&1
```

Expected: 编译失败

- [ ] **Step 3: 实现最小错误类型**

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum A2uiError {
    #[error("Surface not found: {0}")]
    SurfaceNotFound(String),
    #[error("Surface ID already exists: {0}")]
    SurfaceIdConflict(String),
    #[error("Invalid component ID: {0}")]
    InvalidComponentId(String),
    #[error("Component reference not found: {0}")]
    ComponentNotFound(String),
    #[error("Catalog not found: {0}")]
    CatalogNotFound(String),
    #[error("Function not available: {0}")]
    FunctionNotAvailable(String),
    #[error("Invalid state transition: current={current:?}, attempted={attempted:?}")]
    InvalidStateTransition { current: String, attempted: String },
    #[error("Deserialization error: {0}")]
    Deserialization(#[from] serde_json::Error),
    #[error("{message} (component: {component_id}, check: {check_index})")]
    ValidationError { message: String, component_id: String, check_index: usize },
}

pub type Result<T, E = A2uiError> = std::result::Result<T, E>;
```

- [ ] **Step 4: 运行测试确认通过**

```bash
cargo test -p a2ui-core error::tests
```

- [ ] **Step 5: 更新 lib.rs 导出**

在 `crates/a2ui-core/src/lib.rs` 开头添加 `pub use error::{A2uiError, Result};`

- [ ] **Step 6: Commit**

```bash
git add crates/a2ui-core/src/error.rs crates/a2ui-core/src/lib.rs
git commit -m "feat(a2ui-core): define unified error types"
```

---

## Task 4: 定义 Surface 状态机 (state/)

**Files:**
- Create: `crates/a2ui-core/src/state/mod.rs`
- Create: `crates/a2ui-core/src/state/surface_state.rs`
- Modify: `crates/a2ui-core/src/lib.rs`

**Interfaces:**
- Consumes: `error::A2uiError`
- Produces: `SurfaceState` 枚举, `StateOperation` 枚举, `StateMachine` trait

- [ ] **Step 1: 编写失败的测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::A2uiError;

    #[test]
    fn test_initial_state_is_pending() {
        let sm = StateMachine::new("s1".to_string());
        assert_eq!(sm.state(), SurfaceState::Pending);
    }

    #[test]
    fn test_pending_to_active() {
        let mut sm = StateMachine::new("s1".to_string());
        sm.create_surface().unwrap();
        assert_eq!(sm.state(), SurfaceState::Active);
    }

    #[test]
    fn test_active_can_update() {
        let mut sm = StateMachine::new("s1".to_string());
        sm.create_surface().unwrap();
        assert!(sm.update_components().is_ok());
        assert!(sm.update_data_model().is_ok());
    }

    #[test]
    fn test_active_to_deleted() {
        let mut sm = StateMachine::new("s1".to_string());
        sm.create_surface().unwrap();
        sm.delete_surface().unwrap();
        assert_eq!(sm.state(), SurfaceState::Deleted);
    }

    #[test]
    fn test_deleted_cannot_update() {
        let mut sm = StateMachine::new("s1".to_string());
        sm.create_surface().unwrap();
        sm.delete_surface().unwrap();
        assert!(sm.update_components().is_err());
    }

    #[test]
    fn test_deleted_cannot_recreate() {
        let mut sm = StateMachine::new("s1".to_string());
        sm.create_surface().unwrap();
        sm.delete_surface().unwrap();
        assert!(sm.create_surface().is_err());
    }

    #[test]
    fn test_pending_cannot_delete() {
        let mut sm = StateMachine::new("s1".to_string());
        assert!(sm.delete_surface().is_err());
    }

    #[test]
    fn test_display_variants() {
        assert_eq!(SurfaceState::Pending.as_str(), "Pending");
        assert_eq!(SurfaceState::Active.as_str(), "Active");
        assert_eq!(SurfaceState::Deleted.as_str(), "Deleted");
    }

    #[test]
    fn test_state_operation_str() {
        assert_eq!(StateOperation::CreateSurface.as_str(), "CreateSurface");
        assert_eq!(StateOperation::UpdateComponents.as_str(), "UpdateComponents");
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

```bash
cargo test -p a2ui-core state::tests --no-run 2>&1
```

- [ ] **Step 3: 实现状态机**

```rust
use crate::error::{A2uiError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SurfaceState {
    Pending, Active, Deleted,
}

impl SurfaceState {
    pub fn as_str(&self) -> &'static str {
        match self { Pending => "Pending", Active => "Active", Deleted => "Deleted" }
    }
}

impl std::fmt::Display for SurfaceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateOperation {
    CreateSurface, UpdateComponents, UpdateDataModel, DeleteSurface,
}

impl StateOperation {
    pub fn as_str(&self) -> &'static str {
        match self {
            StateOperation::CreateSurface => "CreateSurface",
            StateOperation::UpdateComponents => "UpdateComponents",
            StateOperation::UpdateDataModel => "UpdateDataModel",
            StateOperation::DeleteSurface => "DeleteSurface",
        }
    }
}

#[derive(Debug, Clone)]
pub struct StateMachine {
    surface_id: String,
    state: SurfaceState,
}

impl StateMachine {
    pub fn new(surface_id: String) -> Self {
        Self { surface_id, state: SurfaceState::Pending }
    }
    pub fn state(&self) -> SurfaceState { self.state }
    pub fn surface_id(&self) -> &str { &self.surface_id }

    pub fn create_surface(&mut self) -> Result<()> {
        match self.state {
            SurfaceState::Pending => { self.state = SurfaceState::Active; Ok(()) }
            _ => Err(A2uiError::InvalidStateTransition {
                current: self.state.as_str().to_string(),
                attempted: StateOperation::CreateSurface.as_str().to_string(),
            }),
        }
    }

    pub fn update_components(&self) -> Result<()> {
        match self.state {
            SurfaceState::Active => Ok(()),
            _ => Err(A2uiError::InvalidStateTransition {
                current: self.state.as_str().to_string(),
                attempted: StateOperation::UpdateComponents.as_str().to_string(),
            }),
        }
    }

    pub fn update_data_model(&self) -> Result<()> {
        match self.state {
            SurfaceState::Active => Ok(()),
            _ => Err(A2uiError::InvalidStateTransition {
                current: self.state.as_str().to_string(),
                attempted: StateOperation::UpdateDataModel.as_str().to_string(),
            }),
        }
    }

    pub fn delete_surface(&mut self) -> Result<()> {
        match self.state {
            SurfaceState::Active => { self.state = SurfaceState::Deleted; Ok(()) }
            _ => Err(A2uiError::InvalidStateTransition {
                current: self.state.as_str().to_string(),
                attempted: StateOperation::DeleteSurface.as_str().to_string(),
            }),
        }
    }
}
```

- [ ] **Step 4: 运行测试确认通过**

```bash
cargo test -p a2ui-core state::tests
```

- [ ] **Step 5: 更新 lib.rs 导出**

添加 `pub mod state;`

- [ ] **Step 6: Commit**

```bash
git add crates/a2ui-core/src/state/ crates/a2ui-core/src/lib.rs
git commit -m "feat(a2ui-core): implement Surface state machine"
```

---

## Task 5: 定义 ComponentId 和 DynamicValue (component/)

**Files:**
- Create: `crates/a2ui-core/src/component/mod.rs`
- Create: `crates/a2ui-core/src/component/component.rs`
- Modify: `crates/a2ui-core/src/lib.rs`

**Interfaces:**
- Consumes: `error::A2uiError`
- Produces: `ComponentId`, `DynamicValue<T>`, `ComponentCommon`, `AccessibilityAttributes`

- [ ] **Step 1: 编写失败的测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_component_id_valid() {
        let id = ComponentId::new("my_button").unwrap();
        assert_eq!(id.as_str(), "my_button");
    }

    #[test]
    fn test_component_id_invalid_starts_with_number() {
        assert!(ComponentId::new("123abc").is_err());
    }

    #[test]
    fn test_component_id_invalid_contains_space() {
        assert!(ComponentId::new("my button").is_err());
    }

    #[test]
    fn test_component_id_at_namespace_reserved() {
        assert!(ComponentId::new("@custom").is_err());
    }

    #[test]
    fn test_component_id_empty() {
        assert!(ComponentId::new("").is_err());
    }

    #[test]
    fn test_component_id_display() {
        let id = ComponentId::new("root").unwrap();
        assert_eq!(format!("{}", id), "root");
    }

    #[test]
    fn test_dynamic_value_literal_string() {
        let dv: DynamicValue<String> = DynamicValue::Literal("hello".into());
        assert_eq!(dv.as_str(), Some("hello"));
    }

    #[test]
    fn test_dynamic_value_literal_number() {
        let dv: DynamicValue<i64> = DynamicValue::Literal(42);
        assert_eq!(dv.as_i64(), Some(42));
    }

    #[test]
    fn test_dynamic_value_literal_bool() {
        let dv: DynamicValue<bool> = DynamicValue::Literal(true);
        assert_eq!(dv.as_bool(), Some(true));
    }

    #[test]
    fn test_dynamic_value_path() {
        let dv: DynamicValue<String> = DynamicValue::Path { path: "/user/name".into() };
        assert_eq!(dv.as_path(), Some("/user/name"));
    }

    #[test]
    fn test_dynamic_value_function_call() {
        let dv: DynamicValue<String> = DynamicValue::FunctionCall {
            call: "formatString".into(),
            args: json!({"template": "Hello {name}"}),
        };
        assert_eq!(dv.as_function_call(), Some("formatString"));
    }

    #[test]
    fn test_component_common_fields() {
        let common = ComponentCommon {
            id: ComponentId::new("root").unwrap(),
            accessibility: None,
            weight: None,
        };
        assert_eq!(common.id.as_str(), "root");
    }

    #[test]
    fn test_accessibility_attributes() {
        let acc = AccessibilityAttributes {
            label: Some("Submit".into()),
            description: Some("Submits form".into()),
        };
        let common = ComponentCommon {
            id: ComponentId::new("btn").unwrap(),
            accessibility: Some(acc),
            weight: Some(1.0),
        };
        assert!(common.accessibility.is_some());
        assert_eq!(common.weight, Some(1.0));
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

```bash
cargo test -p a2ui-core component::tests --no-run 2>&1
```

- [ ] **Step 3: 实现 ComponentId 和 DynamicValue**

```rust
use crate::error::{A2uiError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ComponentId(String);

impl ComponentId {
    pub fn new<S: AsRef<str>>(s: S) -> Result<Self> {
        let s = s.as_ref();
        if s.is_empty() { return Err(A2uiError::InvalidComponentId("empty ID".into())); }
        if s.starts_with('@') {
            return Err(A2uiError::InvalidComponentId(format!("'@' reserved: {}", s)));
        }
        let mut chars = s.chars();
        let first = chars.next().unwrap();
        if !is_xid_start(first) {
            return Err(A2uiError::InvalidComponentId(format!("invalid start: {}", s)));
        }
        for c in chars {
            if !is_xid_continue(c) {
                return Err(A2uiError::InvalidComponentId(format!("invalid char '{}': {}", c, s)));
            }
        }
        Ok(Self(s.to_string()))
    }
    pub fn as_str(&self) -> &str { &self.0 }
}

impl std::fmt::Display for ComponentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

fn is_xid_start(c: char) -> bool { c == '_' || c.is_ascii_alphabetic() || (c as u32) > 0x7F }
fn is_xid_continue(c: char) -> bool { is_xid_start(c) || c.is_ascii_alphanumeric() || (c as u32) > 0x7F }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DynamicValue<T = Value> {
    Literal(T),
    Path { path: String },
    FunctionCall { call: String, args: Value },
}

impl DynamicValue<String> {
    pub fn as_str(&self) -> Option<&str> {
        match self { DynamicValue::Literal(s) => Some(s.as_str()), _ => None }
    }
    pub fn as_path(&self) -> Option<&str> {
        match self { DynamicValue::Path { path } => Some(path.as_str()), _ => None }
    }
    pub fn as_function_call(&self) -> Option<&str> {
        match self { DynamicValue::FunctionCall { call, .. } => Some(call.as_str()), _ => None }
    }
}

impl DynamicValue<i64> {
    pub fn as_i64(&self) -> Option<i64> {
        match self { DynamicValue::Literal(n) => Some(*n), _ => None }
    }
}

impl DynamicValue<bool> {
    pub fn as_bool(&self) -> Option<bool> {
        match self { DynamicValue::Literal(b) => Some(*b), _ => None }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentCommon {
    pub id: ComponentId,
    pub accessibility: Option<AccessibilityAttributes>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccessibilityAttributes {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}
```

- [ ] **Step 4: 运行测试确认通过**

```bash
cargo test -p a2ui-core component::tests
```

- [ ] **Step 5: 更新 lib.rs**

添加 `pub mod component; pub use component::ComponentId;`

- [ ] **Step 6: Commit**

```bash
git add crates/a2ui-core/src/component/ crates/a2ui-core/src/lib.rs
git commit -m "feat(a2ui-core): define ComponentId and DynamicValue types"
```

---

## Task 6: 定义 ChildList 和 Component 类型

**Files:**
- Create: `crates/a2ui-core/src/component/child_list.rs`
- Modify: `crates/a2ui-core/src/component/mod.rs`, `crates/a2ui-core/src/component/component.rs`

**Interfaces:**
- Consumes: `ComponentId`
- Produces: `ChildList`, `Component`, `ComponentType`

- [ ] **Step 1: 编写失败的测试（child_list.rs）**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_child_list_array() {
        let cl = ChildList::Array { list: vec![
            ComponentId::new("child1").unwrap(),
            ComponentId::new("child2").unwrap(),
        ]};
        let ids: Vec<_> = cl.component_ids().collect();
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn test_child_list_object() {
        let cl = ChildList::Object {
            template: ComponentId::new("item_template").unwrap(),
            path: "/items".to_string(),
        };
        assert_eq!(cl.template_id(), Some("item_template"));
        assert_eq!(cl.data_path(), Some("/items"));
    }

    #[test]
    fn test_child_list_array_serialize() {
        let cl = ChildList::Array { list: vec![
            ComponentId::new("a").unwrap(),
            ComponentId::new("b").unwrap(),
        ]};
        let json = serde_json::to_value(&cl).unwrap();
        assert_eq!(json["children"][0], "a");
        assert_eq!(json["children"][1], "b");
    }

    #[test]
    fn test_child_list_object_serialize() {
        let cl = ChildList::Object {
            template: ComponentId::new("template").unwrap(),
            path: "/items".to_string(),
        };
        let json = serde_json::to_value(&cl).unwrap();
        assert_eq!(json["template"], "template");
        assert_eq!(json["path"], "/items");
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

```bash
cargo test -p a2ui-core child_list::tests --no-run 2>&1
```

- [ ] **Step 3: 实现 ChildList**

```rust
use crate::component::ComponentId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChildList {
    Array { #[serde(rename = "children")] list: Vec<ComponentId> },
    Object { template: ComponentId, path: String },
}

impl ChildList {
    pub fn array(children: Vec<ComponentId>) -> Self { Self::Array { list: children } }
    pub fn object(template: ComponentId, path: impl Into<String>) -> Self {
        Self::Object { template, path: path.into() }
    }
    pub fn component_ids(&self) -> Box<dyn Iterator<Item = &ComponentId> + '_> {
        match self {
            ChildList::Array { list } => Box::new(list.iter()),
            ChildList::Object { .. } => Box::new(std::iter::empty()),
        }
    }
    pub fn template_id(&self) -> Option<&str> {
        match self { ChildList::Object { template, .. } => Some(template.as_str()), _ => None }
    }
    pub fn data_path(&self) -> Option<&str> {
        match self { ChildList::Object { path, .. } => Some(path.as_str()), _ => None }
    }
}

// Custom Serialize to ensure Array serializes as {"children": [...]}
impl serde::Serialize for ChildList {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer,
    {
        match self {
            ChildList::Array { list } => {
                let ids: Vec<&str> = list.iter().map(|c| c.as_str()).collect();
                serde_json::json!({"children": ids}).serialize(serializer)
            }
            ChildList::Object { template, path } => {
                serde_json::json!({"template": template.as_str(), "path": path}).serialize(serializer)
            }
        }
    }
}
```

- [ ] **Step 4: 运行测试确认通过**

```bash
cargo test -p a2ui-core child_list::tests
```

- [ ] **Step 5: 更新 mod.rs**

在 `crates/a2ui-core/src/component/mod.rs` 中添加 `pub mod child_list; pub use child_list::ChildList;`

- [ ] **Step 6: Commit**

```bash
git add crates/a2ui-core/src/component/child_list.rs crates/a2ui-core/src/component/mod.rs
git commit -m "feat(a2ui-core): define ChildList type"
```

---

## Task 7: 定义 Component 类型

**Files:**
- Modify: `crates/a2ui-core/src/component/component.rs`
- Modify: `crates/a2ui-core/src/component/mod.rs`

**Interfaces:**
- Consumes: `ComponentId`, `DynamicValue`, `ChildList`
- Produces: `Component`, `ComponentType`

- [ ] **Step 1: 编写失败的测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_text() {
        let comp = Component::text(
            ComponentId::new("greeting").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        assert_eq!(comp.id().as_str(), "greeting");
        assert_eq!(comp.component_type(), "Text");
    }

    #[test]
    fn test_component_button() {
        let comp = Component::button(
            ComponentId::new("submit").unwrap(),
            ComponentId::new("submit_label").unwrap(),
        );
        assert_eq!(comp.component_type(), "Button");
    }

    #[test]
    fn test_component_column() {
        let comp = Component::column(
            ComponentId::new("col").unwrap(),
            vec![ComponentId::new("a").unwrap(), ComponentId::new("b").unwrap()],
        );
        assert_eq!(comp.component_type(), "Column");
    }

    #[test]
    fn test_component_row() {
        let comp = Component::row(ComponentId::new("row").unwrap(), vec![]);
        assert_eq!(comp.component_type(), "Row");
    }

    #[test]
    fn test_component_with_weight() {
        let comp = Component::text(
            ComponentId::new("t").unwrap(),
            DynamicValue::Literal("hi".to_string()),
        ).with_weight(2.0);
        assert_eq!(comp.common().weight, Some(2.0));
    }

    #[test]
    fn test_component_deserialize() {
        let json = r#"{"id":"root","component":"Text","text":"Hello"}"#;
        let comp: Component = serde_json::from_str(json).unwrap();
        assert_eq!(comp.id().as_str(), "root");
        assert_eq!(comp.component_type(), "Text");
    }

    #[test]
    fn test_component_type_from_str() {
        assert_eq!(ComponentType::Text.as_str(), "Text");
        assert_eq!(ComponentType::Button.as_str(), "Button");
        assert_eq!(ComponentType::Column.as_str(), "Column");
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

```bash
cargo test -p a2ui-core component::component::tests --no-run 2>&1
```

- [ ] **Step 3: 实现 Component**

```rust
use crate::component::{ComponentId, DynamicValue, AccessibilityAttributes, ChildList};
use crate::error::A2uiError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "PascalCase")]
pub enum ComponentType {
    Text, Image, Icon, Video, AudioPlayer,
    Row, Column, List, Card, Tabs, Modal, Divider,
    Button, TextField, CheckBox, ChoicePicker, Slider, DateTimeInput,
}

impl ComponentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ComponentType::Text => "Text", ComponentType::Image => "Image",
            ComponentType::Icon => "Icon", ComponentType::Video => "Video",
            ComponentType::AudioPlayer => "AudioPlayer",
            ComponentType::Row => "Row", ComponentType::Column => "Column",
            ComponentType::List => "List", ComponentType::Card => "Card",
            ComponentType::Tabs => "Tabs", ComponentType::Modal => "Modal",
            ComponentType::Divider => "Divider",
            ComponentType::Button => "Button", ComponentType::TextField => "TextField",
            ComponentType::CheckBox => "CheckBox", ComponentType::ChoicePicker => "ChoicePicker",
            ComponentType::Slider => "Slider", ComponentType::DateTimeInput => "DateTimeInput",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Component {
    #[serde(rename = "component")]
    component_type: String,
    #[serde(flatten)]
    common: crate::component::component::ComponentCommon,
    #[serde(flatten)]
    properties: Value,
}

impl Component {
    pub fn text(id: ComponentId, text: DynamicValue<String>) -> Self {
        Self {
            component_type: "Text".to_string(),
            common: crate::component::component::ComponentCommon {
                id, accessibility: None, weight: None,
            },
            properties: match text {
                DynamicValue::Literal(s) => serde_json::json!({"text": s}),
                DynamicValue::Path { path } => serde_json::json!({"text": {"path": path}}),
                DynamicValue::FunctionCall { call, args } => {
                    serde_json::json!({"text": {"call": call, "args": args}})
                }
            },
        }
    }

    pub fn button(id: ComponentId, child: ComponentId) -> Self {
        Self {
            component_type: "Button".to_string(),
            common: crate::component::component::ComponentCommon {
                id, accessibility: None, weight: None,
            },
            properties: serde_json::json!({"child": child.as_str()}),
        }
    }

    pub fn column(id: ComponentId, children: Vec<ComponentId>) -> Self {
        let ids: Vec<String> = children.iter().map(|c| c.as_str().to_string()).collect();
        Self {
            component_type: "Column".to_string(),
            common: crate::component::component::ComponentCommon {
                id, accessibility: None, weight: None,
            },
            properties: serde_json::json!({"children": {"children": ids}}),
        }
    }

    pub fn row(id: ComponentId, children: Vec<ComponentId>) -> Self {
        let ids: Vec<String> = children.iter().map(|c| c.as_str().to_string()).collect();
        Self {
            component_type: "Row".to_string(),
            common: crate::component::component::ComponentCommon {
                id, accessibility: None, weight: None,
            },
            properties: serde_json::json!({"children": {"children": ids}}),
        }
    }

    pub fn with_weight(mut self, weight: f64) -> Self {
        self.common.weight = Some(weight);
        self
    }

    pub fn id(&self) -> &ComponentId { &self.common.id }
    pub fn component_type(&self) -> &str { &self.component_type }
    pub fn common(&self) -> &crate::component::component::ComponentCommon { &self.common }
    pub fn properties(&self) -> &Value { &self.properties }
}
```

- [ ] **Step 4: 运行测试确认通过**

```bash
cargo test -p a2ui-core component::component::tests
```

- [ ] **Step 5: 更新 mod.rs**

添加 `pub mod component; pub use component::{Component, ComponentType};`

- [ ] **Step 6: Commit**

```bash
git add crates/a2ui-core/src/component/component.rs crates/a2ui-core/src/component/mod.rs
git commit -m "feat(a2ui-core): define Component type with constructors"
```

---

## Task 8: 定义 Catalog 类型 (component/catalog.rs)

**Files:**
- Create: `crates/a2ui-core/src/component/catalog.rs`
- Modify: `crates/a2ui-core/src/component/mod.rs`

**Interfaces:**
- Consumes: `ComponentType`, `serde_json::Value`
- Produces: `Catalog` 结构

- [ ] **Step 1: 编写失败的测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catalog_new() {
        let catalog = Catalog::new("https://example.com/basic".to_string());
        assert_eq!(catalog.catalog_id(), "https://example.com/basic");
        assert!(catalog.components().is_empty());
    }

    #[test]
    fn test_catalog_add_component() {
        let mut catalog = Catalog::new("basic".to_string());
        catalog.add_component("Text".to_string(), json!({"type": "object"}));
        assert!(catalog.has_component("Text"));
        assert!(catalog.get_component_schema("Text").is_some());
        assert!(catalog.get_component_schema("Button").is_none());
    }

    #[test]
    fn test_catalog_add_function() {
        let mut catalog = Catalog::new("basic".to_string());
        catalog.add_function("required".to_string(), json!({"returnType": "boolean", "callableFrom": "clientOnly"}));
        assert!(catalog.has_function("required"));
        assert_eq!(catalog.function_callable_from("required"), Some("clientOnly"));
    }

    #[test]
    fn test_catalog_deserialize() {
        let json = r#"{
            "catalogId": "my-catalog",
            "instructions": "Test catalog",
            "components": {"Text": {"type": "object", "required": ["text"]}},
            "functions": {"required": {"type": "object", "returnType": "boolean", "callableFrom": "clientOnly"}}
        }"#;
        let catalog: Catalog = serde_json::from_str(json).unwrap();
        assert_eq!(catalog.catalog_id(), "my-catalog");
        assert!(catalog.has_component("Text"));
        assert!(catalog.has_function("required"));
    }

    #[test]
    fn test_catalog_validate_rejects_extra_defs() {
        let json = r#"{
            "catalogId": "test",
            "components": {},
            "functions": {},
            "$defs": {"customSchema": {"type": "string"}}
        }"#;
        let catalog: Catalog = serde_json::from_str(json).unwrap();
        assert!(catalog.validate().is_err());
    }

    #[test]
    fn test_catalog_validate_accepts_valid_defs() {
        let json = r#"{
            "catalogId": "test",
            "components": {},
            "functions": {},
            "$defs": {"surfaceProperties": {"type": "object"}}
        }"#;
        let catalog: Catalog = serde_json::from_str(json).unwrap();
        assert!(catalog.validate().is_ok());
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

```bash
cargo test -p a2ui-core catalog::tests --no-run 2>&1
```

- [ ] **Step 3: 实现 Catalog**

```rust
use crate::error::{A2uiError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    catalog_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<String>,
    #[serde(default)]
    components: HashMap<String, Value>,
    #[serde(default)]
    functions: HashMap<String, Value>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    defs: HashMap<String, Value>,
}

impl Catalog {
    pub fn new(catalog_id: impl Into<String>) -> Self {
        Self {
            catalog_id: catalog_id.into(),
            instructions: None,
            components: HashMap::new(),
            functions: HashMap::new(),
            defs: HashMap::new(),
        }
    }
    pub fn with_instructions(mut self, instructions: impl Into<String>) -> Self {
        self.instructions = Some(instructions.into());
        self
    }
    pub fn add_component(&mut self, name: impl Into<String>, schema: Value) {
        self.components.insert(name.into(), schema);
    }
    pub fn add_function(&mut self, name: impl Into<String>, schema: Value) {
        self.functions.insert(name.into(), schema);
    }
    pub fn catalog_id(&self) -> &str { &self.catalog_id }
    pub fn components(&self) -> &HashMap<String, Value> { &self.components }
    pub fn functions(&self) -> &HashMap<String, Value> { &self.functions }
    pub fn has_component(&self, name: &str) -> bool { self.components.contains_key(name) }
    pub fn get_component_schema(&self, name: &str) -> Option<&Value> { self.components.get(name) }
    pub fn has_function(&self, name: &str) -> bool { self.functions.contains_key(name) }
    pub fn get_function_schema(&self, name: &str) -> Option<&Value> { self.functions.get(name) }
    pub fn function_callable_from(&self, name: &str) -> Option<&str> {
        self.functions.get(name)?.get("callableFrom")?.as_str()
    }
    pub fn validate(&self) -> Result<()> {
        for key in self.defs.keys() {
            if !["surfaceProperties", "anyComponent", "anyFunction"].contains(&key.as_str()) {
                return Err(A2uiError::ValidationError {
                    message: format!("$defs contains disallowed key: {}", key),
                    component_id: "catalog".into(),
                    check_index: 0,
                });
            }
        }
        Ok(())
    }
}
```

- [ ] **Step 4: 运行测试确认通过**

```bash
cargo test -p a2ui-core catalog::tests
```

- [ ] **Step 5: 更新 mod.rs**

添加 `pub mod catalog; pub use catalog::Catalog;`

- [ ] **Step 6: Commit**

```bash
git add crates/a2ui-core/src/component/catalog.rs crates/a2ui-core/src/component/mod.rs
git commit -m "feat(a2ui-core): define Catalog type with validation"
```

---

## Task 9: 定义 DataModel (datamodel/)

**Files:**
- Create: `crates/a2ui-core/src/datamodel/mod.rs`
- Create: `crates/a2ui-core/src/datamodel/model.rs`
- Modify: `crates/a2ui-core/src/lib.rs`

**Interfaces:**
- Consumes: `serde_json::Value`
- Produces: `DataModel`

- [ ] **Step 1: 编写失败的测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_new_datamodel() {
        let dm = DataModel::new(json!({"name": "Alice"}));
        assert_eq!(dm.get("/name"), Some(&json!("Alice")));
    }

    #[test]
    fn test_apply_pointer_create() {
        let mut dm = DataModel::new(json!({}));
        dm.apply_pointer("/name", Some(json!("Alice")));
        assert_eq!(dm.get("/name"), Some(&json!("Alice")));
    }

    #[test]
    fn test_apply_pointer_update() {
        let mut dm = DataModel::new(json!({"name": "Alice"}));
        dm.apply_pointer("/name", Some(json!("Bob")));
        assert_eq!(dm.get("/name"), Some(&json!("Bob")));
    }

    #[test]
    fn test_apply_pointer_delete() {
        let mut dm = DataModel::new(json!({"name": "Alice"}));
        dm.apply_pointer("/name", None);
        assert_eq!(dm.get("/name"), None);
    }

    #[test]
    fn test_apply_pointer_nested_create() {
        let mut dm = DataModel::new(json!({}));
        dm.apply_pointer("/user/name", Some(json!("Alice")));
        assert_eq!(dm.get("/user/name"), Some(&json!("Alice")));
    }

    #[test]
    fn test_apply_pointer_replace_root() {
        let mut dm = DataModel::new(json!({"old": true}));
        dm.apply_pointer("/", Some(json!({"new": true})));
        assert_eq!(dm.get("/new"), Some(&json!(true)));
        assert_eq!(dm.get("/old"), None);
    }

    #[test]
    fn test_apply_pointer_delete_root() {
        let mut dm = DataModel::new(json!({"a": 1}));
        dm.apply_pointer("/", None);
        assert_eq!(dm.get("/a"), None);
    }

    #[test]
    fn test_resolve_pointer_nonexistent() {
        let dm = DataModel::new(json!({}));
        assert_eq!(dm.get("/missing"), None);
    }

    #[test]
    fn test_resolve_pointer_nested_array() {
        let dm = DataModel::new(json!({"items": [{"id": 1}, {"id": 2}]}));
        assert_eq!(dm.get("/items/0/id"), Some(&json!(1)));
        assert_eq!(dm.get("/items/1/id"), Some(&json!(2)));
    }

    #[test]
    fn test_resolve_pointer_escaped_slash() {
        let dm = DataModel::new(json!({"a/b": "value"}));
        assert_eq!(dm.get("/a~1b"), Some(&json!("value")));
    }

    #[test]
    fn test_resolve_pointer_tilde_escape() {
        let dm = DataModel::new(json!({"a~b": "value"}));
        assert_eq!(dm.get("/a~0b"), Some(&json!("value")));
    }

    #[test]
    fn test_as_value() {
        let dm = DataModel::new(json!({"x": 1}));
        assert_eq!(dm.as_value(), &json!({"x": 1}));
    }

    #[test]
    fn test_empty_datamodel() {
        let dm = DataModel::empty();
        assert_eq!(dm.get("/anything"), None);
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

```bash
cargo test -p a2ui-core datamodel::tests --no-run 2>&1
```

- [ ] **Step 3: 实现 DataModel**

```rust
use crate::error::{A2uiError, Result};
use serde::Serialize;
use serde_json::Value;
use std::ops::Deref;

#[derive(Debug, Clone, Default, Serialize)]
pub struct DataModel {
    value: Value,
}

impl DataModel {
    pub fn new(value: Value) -> Self { Self { value } }
    pub fn empty() -> Self {
        Self { value: Value::Object(Default::default()) }
    }
    pub fn get(&self, pointer: &str) -> Option<&Value> { self.value.pointer(pointer) }
    pub fn get_mut(&mut self, pointer: &str) -> Option<&mut Value> { self.value.pointer_mut(pointer) }
    pub fn apply_pointer(&mut self, pointer: &str, value: Option<Value>) {
        if pointer.is_empty() || pointer == "/" {
            if let Some(v) = value { self.value = v; } else { self.value = Value::Object(Default::default()); }
            return;
        }
        if value.is_none() { self._delete_at_pointer(pointer); return; }
        let new_value = value.unwrap();
        if let Some(target) = self.value.pointer_mut(pointer) { *target = new_value; }
        else { self._create_path(pointer, new_value); }
    }
    pub fn delete_pointer(&mut self, pointer: &str) { self.apply_pointer(pointer, None); }
    pub fn as_value(&self) -> &Value { &self.value }
    pub fn as_value_mut(&mut self) -> &mut Value { &mut self.value }

    fn _delete_at_pointer(&mut self, pointer: &str) {
        let segments: Vec<&str> = pointer.trim_start_matches('/').split('/').collect();
        if segments.is_empty() { self.value = Value::Object(Default::default()); return; }
        if let Some(last) = segments.last() {
            let parent_path = segments[..segments.len()-1].join("/");
            let parent_path = if parent_path.is_empty() { "/" } else { format!("/{}", parent_path) };
            if let Some(Value::Object(map)) = self.value.pointer_mut(&parent_path) {
                let unescaped = last.replace("~1", "/").replace("~0", "~");
                map.remove(&unescaped);
            }
        }
    }

    fn _create_path(&mut self, pointer: &str, value: Value) {
        let segments: Vec<&str> = pointer.trim_start_matches('/').split('/').collect();
        if segments.is_empty() { return; }
        let mut current = &mut self.value;
        for (i, segment) in segments.iter().enumerate() {
            let unescaped = segment.replace("~1", "/").replace("~0", "~");
            if i == segments.len() - 1 {
                if let Value::Object(ref mut map) = current { map.insert(unescaped, value.clone()); }
            } else {
                match current {
                    Value::Object(ref mut map) => {
                        if !map.contains_key(&unescaped) {
                            map.insert(unescaped.clone(), Value::Object(Default::default()));
                        }
                        current = map.get_mut(&unescaped).unwrap();
                    }
                    _ => return,
                }
            }
        }
    }
}

impl Deref for DataModel {
    type Target = Value;
    fn deref(&self) -> &Self::Target { &self.value }
}
```

- [ ] **Step 4: 运行测试确认通过**

```bash
cargo test -p a2ui-core datamodel::tests
```

- [ ] **Step 5: 更新 lib.rs**

添加 `pub mod datamodel; pub use datamodel::DataModel;`

- [ ] **Step 6: Commit**

```bash
git add crates/a2ui-core/src/datamodel/ crates/a2ui-core/src/lib.rs
git commit -m "feat(a2ui-core): implement DataModel with JSON Pointer operations"
```

---

## Task 10: 定义消息类型 — Server → Client (message/)

**Files:**
- Create: `crates/a2ui-core/src/message/mod.rs`
- Create: `crates/a2ui-core/src/message/envelope.rs`
- Create: `crates/a2ui-core/src/message/server_to_client.rs`
- Create: `crates/a2ui-core/src/message/client_to_server.rs`
- Modify: `crates/a2ui-core/src/lib.rs`

**Interfaces:**
- Consumes: `Component`, `DataModel`, `DynamicValue`
- Produces: `CreateSurface`, `UpdateComponents`, `UpdateDataModel`, `DeleteSurface`, `ActionResponse`, `CallFunction`, `Action`, `FunctionResponse`, `ClientError`, `ServerEnvelope`, `ClientEnvelope`

- [ ] **Step 1: 编写 server_to_client 测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::{Component, ComponentId};
    use crate::datamodel::DataModel;
    use crate::prelude::*;
    use serde_json::json;

    #[test]
    fn test_create_surface_serialize() {
        let msg = CreateSurface {
            surface_id: "s1".into(),
            catalog_id: "basic".into(),
            surface_properties: None,
            send_data_model: false,
            components: None,
            data_model: None,
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["surfaceId"], "s1");
        assert_eq!(json["catalogId"], "basic");
        assert_eq!(json["sendDataModel"], false);
    }

    #[test]
    fn test_create_surface_with_components() {
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        let msg = CreateSurface {
            surface_id: "s1".into(),
            catalog_id: "basic".into(),
            surface_properties: None,
            send_data_model: false,
            components: Some(vec![comp]),
            data_model: None,
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["components"][0]["id"], "root");
    }

    #[test]
    fn test_update_components() {
        let comp = Component::text(
            ComponentId::new("title").unwrap(),
            DynamicValue::Literal("Title".to_string()),
        );
        let msg = UpdateComponents { surface_id: "s1".into(), components: vec![comp] };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["surfaceId"], "s1");
        assert_eq!(json["components"][0]["id"], "title");
    }

    #[test]
    fn test_update_data_model() {
        let msg = UpdateDataModel {
            surface_id: "s1".into(),
            path: None,
            value: Some(json!({"name": "Alice"})),
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["dataModel"]["name"], "Alice");
    }

    #[test]
    fn test_delete_surface() {
        let msg = DeleteSurface { surface_id: "s1".into() };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["surfaceId"], "s1");
    }

    #[test]
    fn test_action_response_success() {
        let msg = ActionResponse {
            action_id: "act1".into(),
            response: ActionResponsePayload::Success(json!("done")),
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["actionId"], "act1");
        assert_eq!(json["actionResponse"]["value"], "done");
    }

    #[test]
    fn test_action_response_error() {
        let msg = ActionResponse {
            action_id: "act1".into(),
            response: ActionResponsePayload::Error(ResponseError {
                code: "INVALID_INPUT".into(),
                message: "Invalid data".into(),
            }),
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["actionResponse"]["error"]["code"], "INVALID_INPUT");
    }

    #[test]
    fn test_call_function() {
        let msg = CallFunction {
            function_call_id: "fc1".into(),
            want_response: true,
            call: CallFunctionPayload { call: "formatString".into(), args: json!({"template": "Hi"}) },
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["functionCallId"], "fc1");
        assert_eq!(json["callFunction"]["call"], "formatString");
        assert!(json["wantResponse"].as_bool().unwrap());
    }

    #[test]
    fn test_deserialize_create_surface() {
        let json = r#"{"version":"v1.0","createSurface":{"surfaceId":"s1","catalogId":"basic"}}"#;
        let env: ServerEnvelope = serde_json::from_str(json).unwrap();
        match env {
            ServerEnvelope::V1_0(V1_0ServerMessage::CreateSurface(msg)) => {
                assert_eq!(msg.surface_id, "s1");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_deserialize_action_response() {
        let json = r#"{"version":"v1.0","actionId":"a1","actionResponse":{"value":"ok"}}"#;
        let env: ServerEnvelope = serde_json::from_str(json).unwrap();
        match env {
            ServerEnvelope::V1_0(V1_0ServerMessage::ActionResponse(msg)) => {
                assert_eq!(msg.action_id, "a1");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_deserialize_unknown_fails() {
        let json = r#"{"version":"v1.0","unknownMessage":{}}"#;
        let result: Result<ServerEnvelope> = serde_json::from_str(json).map_err(Into::into);
        assert!(result.is_err());
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

```bash
cargo test -p a2ui-core message::server_to_client::tests --no-run 2>&1
```

- [ ] **Step 3: 实现消息类型**

`message/server_to_client.rs`:

```rust
use crate::component::Component;
use crate::error::{A2uiError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseError { pub code: String, pub message: String }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ActionResponsePayload {
    Success(Value),
    Error(ResponseError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallFunctionPayload { pub call: String, pub args: Value }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSurface {
    pub surface_id: String,
    pub catalog_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surface_properties: Option<Value>,
    pub send_data_model: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub components: Option<Vec<Component>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_model: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateComponents { pub surface_id: String, pub components: Vec<Component> }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDataModel {
    pub surface_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteSurface { pub surface_id: String }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionResponse {
    pub action_id: String,
    #[serde(flatten)]
    pub response: ActionResponsePayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallFunction {
    pub function_call_id: String,
    pub want_response: bool,
    #[serde(flatten)]
    pub call: CallFunctionPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "version", content = "message", rename_all = "camelCase")]
pub enum V1_0ServerMessage {
    CreateSurface(CreateSurface),
    UpdateComponents(UpdateComponents),
    UpdateDataModel(UpdateDataModel),
    DeleteSurface(DeleteSurface),
    ActionResponse(ActionResponse),
    CallFunction(CallFunction),
}
```

- [ ] **Step 4: 运行测试确认通过**

```bash
cargo test -p a2ui-core message::server_to_client::tests
```

- [ ] **Step 5: 更新 mod.rs**

```rust
// message/mod.rs
pub mod envelope;
pub mod server_to_client;
pub mod client_to_server;
pub use envelope::{ServerEnvelope, ClientEnvelope};
pub use server_to_client::V1_0ServerMessage;
pub use client_to_server::V1_0ClientMessage;
```

- [ ] **Step 6: Commit**

```bash
git add crates/a2ui-core/src/message/ crates/a2ui-core/src/lib.rs
git commit -m "feat(a2ui-core): define server-to-client message types"
```

### Task 10 (续): Client → Server 消息类型

**Files:**
- Create: `crates/a2ui-core/src/message/client_to_server.rs`

**测试要点:**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn test_action_event() {
        let action = Action::Event {
            name: "submit".into(), surface_id: "s1".into(),
            source_component_id: Some("btn".into()),
            context: HashMap::new(), want_response: false,
            response_path: None, action_id: None,
        };
        let json = serde_json::to_value(&action).unwrap();
        assert_eq!(json["name"], "submit");
    }

    #[test]
    fn test_action_event_with_response() {
        let action = Action::Event {
            name: "fetch".into(), surface_id: "s1".into(),
            source_component_id: None,
            context: HashMap::new(), want_response: true,
            response_path: Some("/result".into()), action_id: Some("act-1".into()),
        };
        let json = serde_json::to_value(&action).unwrap();
        assert!(json["wantResponse"].as_bool().unwrap());
        assert_eq!(json["responsePath"], "/result");
    }

    #[test]
    fn test_function_response() {
        let msg = FunctionResponse {
            function_call_id: "fc1".into(), call: "required".into(), value: json!(true),
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["functionResponse"]["value"], true);
    }

    #[test]
    fn test_client_error() {
        let msg = ClientError {
            code: "INVALID_FUNCTION_CALL".into(),
            message: "Function not registered".into(),
            function_call_id: Some("fc1".into()),
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["error"]["code"], "INVALID_FUNCTION_CALL");
    }

    #[test]
    fn test_client_envelope_action() {
        let json = r#"{"version":"v1.0","action":{"name":"click","surfaceId":"s1"}}"#;
        let env: ClientEnvelope = serde_json::from_str(json).unwrap();
        match env {
            ClientEnvelope::V1_0(V1_0ClientMessage::Action(a)) => assert_eq!(a.name, "click"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_unknown_version_fails() {
        let json = r#"{"version":"v9.9","createSurface":{"surfaceId":"s1"}}"#;
        let result: Result<ServerEnvelope> = serde_json::from_str(json).map_err(Into::into);
        assert!(result.is_err());
    }
}
```

**实现要点:**

`message/client_to_server.rs`:

```rust
use crate::component::DynamicValue;
use crate::error::{A2uiError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

pub type ActionContext = HashMap<String, DynamicValue>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Action {
    Event {
        name: String, surface_id: String,
        source_component_id: Option<String>,
        context: ActionContext, want_response: bool,
        response_path: Option<String>, action_id: Option<String>,
    },
    FunctionCall { call: String, args: ActionContext },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionMessage {
    pub name: String, pub surface_id: String,
    #[serde(skip_serializing_if = "Option::is_none")] pub source_component_id: Option<String>,
    #[serde(default)] pub context: ActionContext,
    pub want_response: bool,
    #[serde(skip_serializing_if = "Option::is_none")] pub response_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub action_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionResponse {
    pub function_call_id: String, pub call: String, pub value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientError {
    pub code: String, pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")] pub function_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "version", content = "message", rename_all = "camelCase")]
pub enum V1_0ClientMessage {
    Action(ActionMessage), FunctionResponse(FunctionResponse), Error(ClientError),
}
```

`message/envelope.rs`:

```rust
use crate::error::{A2uiError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "version", content = "message", rename_all = "camelCase")]
pub enum ServerEnvelope {
    V1_0(super::server_to_client::V1_0ServerMessage),
}

impl ServerEnvelope {
    pub fn from_json(json: &str) -> Result<Self> { Ok(serde_json::from_str(json)?) }
    pub fn to_value(&self) -> Value { serde_json::to_value(self).unwrap() }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "version", content = "message", rename_all = "camelCase")]
pub enum ClientEnvelope {
    V1_0(super::client_to_server::V1_0ClientMessage),
}

impl ClientEnvelope {
    pub fn from_json(json: &str) -> Result<Self> { Ok(serde_json::from_str(json)?) }
    pub fn to_value(&self) -> Value { serde_json::to_value(self).unwrap() }
}
```

Commit:
```bash
git add crates/a2ui-core/src/message/client_to_server.rs crates/a2ui-core/src/message/envelope.rs crates/a2ui-core/src/message/mod.rs
git commit -m "feat(a2ui-core): define client-to-server messages and envelope types"
```

---

## Task 11: 添加 Basic Catalog JSON 资产

**Files:**
- Create: `crates/a2ui-core/assets/catalogs/basic/catalog.json`

**Interfaces:**
- Consumes: Catalog 类型
- Produces: Basic Catalog JSON 文件

- [ ] **Step 1: 创建 catalog.json**

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "a2ui://catalogs/basic/v1",
  "catalogId": "a2ui://catalogs/basic/v1",
  "instructions": "# A2UI Basic Catalog\n\nStandard component set for A2UI Protocol v1.0.",
  "components": {
    "Text": {"type":"object","required":["text"],"properties":{"text":{"anyOf":[{"type":"string"},{"$ref":"#/$defs/dynamicString"}]}}},
    "Button": {"type":"object","required":["child"],"properties":{"child":{"type":"string"},"variant":{"enum":["default","primary","borderless"]}}},
    "TextField": {"type":"object","required":["value"],"properties":{"value":{"$ref":"#/$defs/dynamicString"},"variant":{"enum":["shortText","number","longText","obscured"]}}},
    "Column": {"type":"object","properties":{"children":{"$ref":"#/$defs/childList"}}},
    "Row": {"type":"object","properties":{"children":{"$ref":"#/$defs/childList"}}},
    "Image": {"type":"object","required":["url"],"properties":{"url":{"$ref":"#/$defs/dynamicString"}}},
    "Card": {"type":"object","required":["child"],"properties":{"child":{"type":"string"}}},
    "CheckBox": {"type":"object","properties":{"value":{"$ref":"#/$defs/dynamicBoolean"},"label":{"type":"string"}}},
    "Divider": {"type":"object"},
    "Icon": {"type":"object","properties":{"name":{"type":"string"}}},
    "List": {"type":"object","properties":{"children":{"$ref":"#/$defs/childList"}}},
    "Tabs": {"type":"object","properties":{"tabs":{"type":"array"}}},
    "Modal": {"type":"object","properties":{"content":{"type":"string"},"trigger":{"type":"string"}}},
    "Slider": {"type":"object","properties":{"value":{"$ref":"#/$defs/dynamicNumber"},"min":{"type":"number"},"max":{"type":"number"}}},
    "ChoicePicker": {"type":"object","properties":{"value":{"$ref":"#/$defs/dynamicStringList"},"options":{"type":"array"}}},
    "DateTimeInput": {"type":"object","properties":{"label":{"type":"string"}}},
    "Video": {"type":"object","properties":{"url":{"type":"string"}}},
    "AudioPlayer": {"type":"object","properties":{"url":{"type":"string"}}}
  },
  "functions": {
    "required": {"type":"object","returnType":"boolean","callableFrom":"clientOnly","properties":{"value":{}}},
    "regex": {"type":"object","returnType":"boolean","callableFrom":"clientOnly","properties":{"value":{},"pattern":{"type":"string"}}},
    "email": {"type":"object","returnType":"boolean","callableFrom":"clientOnly","properties":{"value":{"type":"string"}}},
    "length": {"type":"object","returnType":"boolean","callableFrom":"clientOnly","properties":{"value":{"type":"string"},"min":{"type":"integer"},"max":{"type":"integer"}}},
    "numeric": {"type":"object","returnType":"boolean","callableFrom":"clientOnly","properties":{"value":{},"min":{"type":"number"},"max":{"type":"number"}}},
    "and": {"type":"object","returnType":"boolean","callableFrom":"clientOrRemote","properties":{"values":{"type":"array"}}},
    "or": {"type":"object","returnType":"boolean","callableFrom":"clientOrRemote","properties":{"values":{"type":"array"}}},
    "not": {"type":"object","returnType":"boolean","callableFrom":"clientOrRemote","properties":{"value":{"type":"boolean"}}},
    "formatString": {"type":"object","returnType":"string","callableFrom":"clientOrRemote","properties":{"template":{"type":"string"}}},
    "formatNumber": {"type":"object","returnType":"string","callableFrom":"clientOrRemote"},
    "formatCurrency": {"type":"object","returnType":"string","callableFrom":"clientOrRemote"},
    "formatDate": {"type":"object","returnType":"string","callableFrom":"clientOrRemote"},
    "pluralize": {"type":"object","returnType":"string","callableFrom":"clientOrRemote"},
    "openUrl": {"type":"object","returnType":"void","callableFrom":"clientOnly","properties":{"url":{"type":"string"}}}
  },
  "$defs": {
    "surfaceProperties": {"type":"object","properties":{"agentDisplayName":{"type":"string"},"iconUrl":{"type":"string","format":"uri"}}},
    "anyComponent": {"oneOf":[],"discriminator":{"propertyName":"component"}},
    "anyFunction": {"oneOf":[],"discriminator":{"propertyName":"call"}}
  }
}
```

- [ ] **Step 2: 添加资源加载函数到 lib.rs**

```rust
#[cfg(feature = "embed-assets")]
pub fn load_basic_catalog() -> Result<Catalog> {
    let json = include_str!("assets/catalogs/basic/catalog.json");
    let catalog: Catalog = serde_json::from_str(json)?;
    catalog.validate()?;
    Ok(catalog)
}
```

- [ ] **Step 3: Commit**

```bash
git add crates/a2ui-core/assets/catalogs/basic/catalog.json crates/a2ui-core/src/lib.rs
git commit -m "feat(a2ui-core): add Basic Catalog JSON asset"
```

---

## Task 12: 实现 Catalog Schema 验证 (schema/)

**Files:**
- Create: `crates/a2ui-core/src/schema/mod.rs`
- Create: `crates/a2ui-core/src/schema/catalog_schema.rs`
- Modify: `crates/a2ui-core/src/lib.rs`

**测试要点:**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::Catalog;

    #[test]
    fn test_validate_ok() {
        let mut catalog = Catalog::new("test".to_string());
        catalog.add_component("Text".into(), json!({"type": "object"}));
        catalog.add_function("required".into(), json!({"returnType":"boolean","callableFrom":"clientOnly"}));
        assert!(CatalogValidator::validate(&catalog).is_ok());
    }

    #[test]
    fn test_validate_missing_catalog_id() {
        let catalog = Catalog::new("".to_string());
        assert!(CatalogValidator::validate(&catalog).is_err());
    }

    #[test]
    fn test_validate_function_missing_return_type() {
        let mut catalog = Catalog::new("test".to_string());
        catalog.add_function("bad".into(), json!({"type": "object"}));
        assert!(CatalogValidator::validate(&catalog).is_err());
    }
}
```

**实现:**

```rust
use crate::component::Catalog;
use crate::error::{A2uiError, Result};

pub struct CatalogValidator;

impl CatalogValidator {
    pub fn validate(catalog: &Catalog) -> Result<()> {
        if catalog.catalog_id().is_empty() {
            return Err(A2uiError::ValidationError {
                message: "catalogId is required".into(),
                component_id: "catalog".into(), check_index: 0,
            });
        }
        catalog.validate()?;
        for name in catalog.components().keys() {
            let schema = catalog.get_component_schema(name).unwrap();
            if !schema.is_object() {
                return Err(A2uiError::ValidationError {
                    message: format!("Component '{}' schema must be object", name),
                    component_id: name.clone(), check_index: 0,
                });
            }
        }
        for (name, schema) in catalog.functions() {
            if !schema.is_object() {
                return Err(A2uiError::ValidationError {
                    message: format!("Function '{}' schema must be object", name),
                    component_id: name.clone(), check_index: 0,
                });
            }
            let obj = schema.as_object().unwrap();
            if !obj.contains_key("returnType") {
                return Err(A2uiError::ValidationError {
                    message: format!("Function '{}' missing returnType", name),
                    component_id: name.clone(), check_index: 0,
                });
            }
            if !obj.contains_key("callableFrom") {
                return Err(A2uiError::ValidationError {
                    message: format!("Function '{}' missing callableFrom", name),
                    component_id: name.clone(), check_index: 0,
                });
            }
        }
        Ok(())
    }
}
```

Commit:
```bash
git add crates/a2ui-core/src/schema/ crates/a2ui-core/src/lib.rs
git commit -m "feat(a2ui-core): implement Catalog schema validation"
```

---

## Task 13: Phase 1 集成测试

**Files:**
- Create: `tests/integration/a2ui_core_tests.rs`
- Modify: `crates/a2ui-core/Cargo.toml`

**测试要点:**

```rust
use a2ui_core::prelude::*;
use serde_json::json;

#[test]
fn test_full_surface_lifecycle() {
    let json = r#"{
        "version": "v1.0",
        "createSurface": {
            "surfaceId": "lifecycle-test", "catalogId": "basic",
            "sendDataModel": true,
            "components": [
                {"id":"root","component":"Column","children":{"children":["title","btn"]}},
                {"id":"title","component":"Text","text":"Hello"},
                {"id":"btn","component":"Button","child":"btn_label"},
                {"id":"btn_label","component":"Text","text":"Click me"}
            ],
            "dataModel": {"user": {"name": "Alice"}}
        }
    }"#;
    let envelope = ServerEnvelope::from_json(json).unwrap();
    match envelope {
        ServerEnvelope::V1_0(V1_0ServerMessage::CreateSurface(msg)) => {
            assert_eq!(msg.surface_id, "lifecycle-test");
            assert_eq!(msg.components.as_ref().unwrap().len(), 4);
            assert_eq!(msg.data_model.as_ref().unwrap()["user"]["name"], "Alice");
        }
        _ => panic!("expected CreateSurface"),
    }
}

#[test]
fn test_data_model_round_trip() {
    let dm = DataModel::new(json!({"form": {"fields": [{"name": "email"}, {"name": "age", "value": 30}]}}));
    assert_eq!(dm.get("/form/fields/0/name"), Some(&json!("email")));
    assert_eq!(dm.get("/form/fields/1/value"), Some(&json!(30)));
}

#[test]
fn test_state_machine_full_cycle() {
    let mut sm = StateMachine::new("s1".to_string());
    assert_eq!(sm.state(), SurfaceState::Pending);
    sm.create_surface().unwrap();
    assert_eq!(sm.state(), SurfaceState::Active);
    sm.delete_surface().unwrap();
    assert_eq!(sm.state(), SurfaceState::Deleted);
    assert!(sm.create_surface().is_err());
}
```

在 `crates/a2ui-core/Cargo.toml` 中添加:
```toml
[[test]]
name = "integration"
path = "tests/integration/a2ui_core_tests.rs"
```

Commit:
```bash
git add tests/integration/a2ui_core_tests.rs crates/a2ui-core/Cargo.toml
git commit -m "test(a2ui-core): add Phase 1 integration tests"
```

---

## Task 14: Phase 1 文档测试完善

**Files:**
- Modify: 所有 pub API 的文档注释

- [ ] **Step 1: 确保每个 pub 类型和函数都有文档测试**

在每个模块中补充 `///` 文档注释 + ` ```rust ` 代码块，例如:

```rust
/// 从 JSON Value 创建 DataModel
///
/// # 示例
///
/// ```rust
/// use a2ui_core::DataModel;
/// use serde_json::json;
///
/// let dm = DataModel::new(json!({"name": "Alice"}));
/// assert_eq!(dm.get("/name"), Some(&json!("Alice")));
/// ```
pub fn new(value: Value) -> Self { ... }
```

- [ ] **Step 2: 运行文档测试**

```bash
cargo test --doc -p a2ui-core
```

Expected: 全部 PASS

- [ ] **Step 3: Commit**

```bash
git add crates/a2ui-core/src/
git commit -m "docs(a2ui-core): add doc tests for all public APIs"
```

---

# Phase 2: a2ui-renderer 渲染器抽象层

**目标:** 实现 `Renderer` trait、组件树管理（ComponentForest）、DataBinding 引擎、路径解析器、函数调度器和 Catalog 注册表。

## 文件结构

```
a2ui-renderer/src/
├── lib.rs
├── error.rs
├── renderer.rs
├── component_forest.rs
├── data_binding.rs
├── path_resolver.rs
├── function_dispatcher.rs
├── catalog_registry.rs
└── dependency_graph.rs
```

---

## Task 16: 创建 a2ui-renderer Crate

**Files:**
- Create: `crates/a2ui-renderer/Cargo.toml`
- Create: `crates/a2ui-renderer/src/lib.rs`
- Modify: 根 `Cargo.toml`

- [ ] **Step 1: 创建 Cargo.toml**

```toml
[package]
name = "a2ui-renderer"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true
description = "A2UI Renderer trait and component tree management"
repository = "https://github.com/yufeng108/a2ui-rs"

[dependencies]
a2ui-core = { path = "../a2ui-core" }
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tokio.workspace = true
async-trait.workspace = true
tracing = "0.1"
uuid = { version = "1.6", features = ["v4"] }
```

- [ ] **Step 2: 创建 lib.rs**

```rust
//! A2UI Renderer — 渲染器抽象层
//!
//! 定义 `Renderer` trait、组件树管理、路径解析、函数调度等核心抽象。

pub mod error;
pub mod renderer;
pub mod component_forest;
pub mod data_binding;
pub mod path_resolver;
pub mod function_dispatcher;
pub mod catalog_registry;
pub mod dependency_graph;

pub use error::RendererError;
pub use renderer::Renderer;
pub use component_forest::ComponentForest;
pub use data_binding::DataBinding;
pub use path_resolver::PathResolver;
pub use function_dispatcher::{FunctionDispatcher, CallableFrom};
pub use catalog_registry::CatalogRegistry;
pub use dependency_graph::DependencyGraph;
```

- [ ] **Step 3: 注册到 workspace**

修改根 `Cargo.toml`，在 `members` 中添加 `"crates/a2ui-renderer"`

- [ ] **Step 4: 验证编译**

```bash
cargo build -p a2ui-renderer
```

- [ ] **Step 5: Commit**

```bash
git add crates/a2ui-renderer/ Cargo.toml
git commit -m "feat: create a2ui-renderer crate skeleton"
```

---

## Task 17: 定义渲染器错误类型 (error.rs)

**Files:**
- Create: `crates/a2ui-renderer/src/error.rs`
- Modify: `crates/a2ui-renderer/src/lib.rs`

**测试要点:**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use a2ui_core::{ComponentId, state::{SurfaceState, StateOperation}};

    #[test]
    fn test_surface_not_found() {
        let err = RendererError::SurfaceNotFound("s1".into());
        assert!(err.to_string().contains("s1"));
    }

    #[test]
    fn test_from_a2ui_error() {
        let a2ui_err = a2ui_core::A2uiError::SurfaceNotFound("s1".into());
        let renderer_err: RendererError = a2ui_err.into();
        assert!(matches!(renderer_err, RendererError::CoreError(_)));
    }

    #[test]
    fn test_invalid_state_transition() {
        let err = RendererError::InvalidStateTransition {
            current: SurfaceState::Deleted,
            attempted: StateOperation::CreateSurface,
        };
        assert!(err.to_string().contains("Deleted"));
    }
}
```

**实现:**

```rust
use a2ui_core::error::A2uiError;
use a2ui_core::state::{StateOperation, SurfaceState};
use a2ui_core::ComponentId;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RendererError {
    #[error("Surface not found: {0}")]
    SurfaceNotFound(String),
    #[error("Surface ID already exists: {0}")]
    SurfaceIdConflict(String),
    #[error("Component reference not found: {0}")]
    ComponentNotFound(ComponentId),
    #[error("Catalog not found: {0}")]
    CatalogNotFound(String),
    #[error("Function not available: {0}")]
    FunctionNotAvailable(String),
    #[error("Invalid state transition: current={current:?}, attempted={attempted:?}")]
    InvalidStateTransition { current: SurfaceState, attempted: StateOperation },
    #[error("Core error: {0}")]
    CoreError(#[from] A2uiError),
    #[error("Binding error: {0}")]
    BindingError(String),
    #[error("Path resolution error: {0}")]
    PathError(String),
}

pub type RenderResult<T> = Result<T, RendererError>;
```

Commit:
```bash
git add crates/a2ui-renderer/src/error.rs
git commit -m "feat(a2ui-renderer): define RendererError types"
```

---

## Task 18: 定义 Renderer trait (renderer.rs)

**Files:**
- Create: `crates/a2ui-renderer/src/renderer.rs`

**测试要点:**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_surface_handle_new() {
        let h1 = SurfaceHandle::new();
        let h2 = SurfaceHandle::new();
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_user_event_variants() {
        let _e = UserEvent::Click { component_id: ComponentId::new("btn").unwrap() };
        let _e = UserEvent::KeyPress { key: "Enter".into() };
        let _e = UserEvent::TextInput { component_id: ComponentId::new("input").unwrap(), value: "hi".into() };
        let _e = UserEvent::CheckToggle { component_id: ComponentId::new("cb").unwrap(), checked: true };
        let _e = UserEvent::SliderChange { component_id: ComponentId::new("slider").unwrap(), value: 0.5 };
    }
}
```

**实现:**

```rust
use a2ui_core::prelude::*;
use async_trait::async_trait;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceHandle(Uuid);

impl SurfaceHandle {
    pub fn new() -> Self { Self(Uuid::new_v4()) }
    pub fn as_uuid(&self) -> &Uuid { &self.0 }
}

#[derive(Debug, Clone)]
pub enum UserEvent {
    Click { component_id: ComponentId },
    KeyPress { key: String },
    TextInput { component_id: ComponentId, value: String },
    CheckToggle { component_id: ComponentId, checked: bool },
    SliderChange { component_id: ComponentId, value: f64 },
}

#[async_trait::async_trait]
pub trait Renderer: Send {
    async fn create_surface(&mut self, msg: CreateSurface) -> RenderResult<SurfaceHandle>;
    async fn update_components(&mut self, msg: UpdateComponents) -> RenderResult<()>;
    async fn update_data_model(&mut self, msg: UpdateDataModel) -> RenderResult<()>;
    async fn delete_surface(&mut self, msg: DeleteSurface) -> RenderResult<()>;
    async fn action_response(&mut self, msg: ActionResponse) -> RenderResult<()>;
    async fn call_function(&mut self, msg: CallFunction) -> RenderResult<FunctionResponse>;
    async fn render(&mut self) -> RenderResult<()>;
    async fn handle_user_event(&mut self, event: UserEvent) -> RenderResult<Option<ActionMessage>>;
}
```

Commit:
```bash
git add crates/a2ui-renderer/src/renderer.rs
git commit -m "feat(a2ui-renderer): define Renderer trait and SurfaceHandle"
```

---

## Task 19-25: ComponentForest, DataBinding, PathResolver, FunctionDispatcher, CatalogRegistry, DependencyGraph

每个 Task 遵循相同 TDD 模式：写测试 → 确认失败 → 实现 → 确认通过 → commit。

**关键接口摘要:**

```rust
// ComponentForest
impl ComponentForest {
    pub fn new() -> Self;
    pub fn upsert(&mut self, surface_id: SurfaceId, component: Component) -> Result<()>;
    pub fn get(&self, surface_id: &SurfaceId, component_id: &ComponentId) -> Option<&Component>;
    pub fn get_root(&self, surface_id: &SurfaceId) -> Option<&Component>;
    pub fn remove_surface(&mut self, surface_id: &SurfaceId) -> Result<()>;
    pub fn build_tree(&self, surface_id: &SurfaceId) -> Result<ComponentTreeNode>;
}

// DataBinding
impl DataBinding {
    pub fn new(data_model: DataModel) -> Self;
    pub fn get(&self, path: &str) -> Option<&Value>;
    pub fn set(&mut self, path: &str, value: Value) -> Result<()>;
    pub fn resolve_dynamic(&self, dynamic: &DynamicValue) -> Result<Value>;
    pub fn as_value(&self) -> &Value;
}

// PathResolver
impl PathResolver {
    pub fn new(data_model: DataModel) -> Self;
    pub fn resolve(&self, path: &str) -> Option<&Value>;
    pub fn resolve_relative(&self, base: &str, relative: &str, index: usize) -> String;
    pub fn resolve_dynamic(&self, dynamic: &DynamicValue) -> Result<Value>;
}

// FunctionDispatcher
pub enum CallableFrom { ClientOnly, RemoteOnly, ClientOrRemote }
impl FunctionDispatcher {
    pub fn new() -> Self;
    pub fn register(&mut self, name: String, callable_from: CallableFrom, handler: fn(Value) -> Result<Value>);
    pub fn dispatch(&self, name: &str, args: Value) -> Result<Value>;
    pub fn can_call_from(&self, name: &str, from: CallableFrom) -> bool;
}

// CatalogRegistry
impl CatalogRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, catalog: Catalog) -> Result<()>;
    pub fn get(&self, catalog_id: &str) -> Option<&Catalog>;
    pub fn get_or_load_basic(&mut self) -> Result<&Catalog>;
}

// DependencyGraph
impl DependencyGraph {
    pub fn new() -> Self;
    pub fn register_dependency(&mut self, component_id: ComponentId, path: String);
    pub fn dependents(&self, path: &str) -> Vec<&ComponentId>;
    pub fn on_data_change(&mut self, path: &str) -> Vec<ComponentId>;
}
```

**Commit序列:**
```
git commit -m "feat(a2ui-renderer): implement ComponentForest"
git commit -m "feat(a2ui-renderer): implement DataBinding"
git commit -m "feat(a2ui-renderer): implement PathResolver"
git commit -m "feat(a2ui-renderer): implement FunctionDispatcher"
git commit -m "feat(a2ui-renderer): implement CatalogRegistry"
git commit -m "feat(a2ui-renderer): implement DependencyGraph"
```

---

# Phase 3: a2ui-renderer-tui TUI 渲染器

**目标:** 基于 ratatui + crossterm 实现完整的 TUI 渲染器。

## 文件结构

```
a2ui-renderer-tui/src/
├── lib.rs
├── tui_renderer.rs
├── widget_mapper.rs
├── focus_manager.rs
├── input_handler.rs
└── terminal.rs
```

---

## Task 26-45: TUI 渲染器实现

### Task 26: 创建 crate

```toml
[package]
name = "a2ui-renderer-tui"
version.workspace = true
edition.workspace = true

[dependencies]
a2ui-core = { path = "../a2ui-core" }
a2ui-renderer = { path = "../a2ui-renderer" }
ratatui = "0.26"
crossterm = "0.27"
tokio.workspace = true
tracing.workspace = true
```

### Task 27-31: TuiRenderer 核心

实现 `Renderer` trait：
- `create_surface`: 创建 SurfaceState，注册组件树
- `update_components`: 调用 ComponentForest.upsert
- `update_data_model`: 调用 DataBinding.set
- `delete_surface`: 清理所有状态

每个方法写测试验证行为，然后实现。

### Task 32-38: Widget Mapper

将 A2UI 组件映射为 ratatui widget：

```
Text     → Paragraph
Button   → Block + Paragraph (with style)
Column   → Layout::vertical()
Row      → Layout::horizontal()
TextField → Paragraph (with cursor)
CheckBox → Block + Paragraph (with checkbox char)
Slider   → Gauge / custom bar
Divider  → Line
```

每个映射写测试：输入 Component → 验证输出 widget 类型正确。

### Task 39: 实现 render 帧

- 测试：调用 render 后 Frame 包含 widget
- 实现：构建 Layout → 遍历组件树 → 绘制 widget

### Task 40-41: 输入处理

- 测试：Key(Enter) 在 Button 上 → Action::Event("click")
- 测试：KeyPress 在 TextField 上 → TextInput Action
- 实现：match event → 查找焦点组件 → 生成 ActionMessage

### Task 42: Focus Manager

- 测试：Tab 循环焦点
- 实现：可聚焦组件集合 + focus index

### Task 43-44: action_response + call_function

- 测试：actionResponse 写入 responsePath
- 测试：callFunction 执行本地注册函数

### Task 45: 端到端测试

```rust
#[test]
fn test_tui_full_flow() {
    // createSurface → render → handle_event → update → delete
}
```

**Commit序列:**
```
git commit -m "feat(a2ui-renderer-tui): create crate and TuiRenderer skeleton"
git commit -m "feat(a2ui-renderer-tui): implement create_surface and delete_surface"
git commit -m "feat(a2ui-renderer-tui): implement update_components and update_data_model"
git commit -m "feat(a2ui-renderer-tui): add widget mapper for layout components"
git commit -m "feat(a2ui-renderer-tui): add widget mapper for input components"
git commit -m "feat(a2ui-renderer-tui): implement render frame"
git commit -m "feat(a2ui-renderer-tui): implement input handling"
git commit -m "feat(a2ui-renderer-tui): implement focus manager"
git commit -m "feat(a2ui-renderer-tui): implement action_response and call_function"
git commit -m "test(a2ui-renderer-tui): add end-to-end integration tests"
```

---

# Phase 4: Transport 层 + CLI

**目标:** 实现传输层抽象 + WebSocket/JSONL 绑定 + CLI 入口。

---

## Task 46-55: Transport + CLI

### Task 46: 创建 a2ui-transport crate

```toml
[package]
name = "a2ui-transport"
version.workspace = true

[dependencies]
a2ui-core = { path = "../a2ui-core" }
tokio.workspace = true
async-trait.workspace = true
tracing.workspace = true
tokio-tungstenite = "0.21"
```

定义 `Transport` trait:
```rust
#[async_trait::async_trait]
pub trait Transport: Send {
    async fn send(&mut self, envelope: ServerEnvelope) -> Result<()>;
    async fn receive(&mut self) -> Result<ClientEnvelope>;
    async fn close(&mut self) -> Result<()>;
}
```

实现 `JsonlTransport` (STDIN/STDOUT) 和 `WebSocketTransport`。

### Task 47-50: 实现 a2ui-cli

```toml
[package]
name = "a2ui-cli"
version.workspace = true

[dependencies]
a2ui-core = { path = "../a2ui-core" }
a2ui-renderer = { path = "../a2ui-renderer" }
a2ui-renderer-tui = { path = "../a2ui-renderer-tui" }
a2ui-transport = { path = "../a2ui-transport" }
clap = { version = "4.5", features = ["derive"] }
tokio.workspace = true
```

CLI 结构:
```rust
#[derive(clap::Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// 从 STDIN 读取 JSONL 流并渲染
    Render { input: Option<PathBuf> },
}
```

### Task 51-55: 端到端测试 + 文档

- 完整 JSONL 示例流测试
- README.md 编写
- 性能基准测试（可选）

---

# 执行策略

## 并行子 Agent 分工

| 工作流 | 任务 | 产出 |
|--------|------|------|
| W1: 基础设施 | Task 1-3 | Cargo.toml, error.rs, state/ |
| W2: Component 类型 | Task 4-7 | component/ 模块 |
| W3: DataModel + 消息 | Task 8-10 | datamodel/, message/ 模块 |
| W4: 验证 + 资产 + 集成测试 | Task 11-15 | assets/, schema/, tests/ |

---

# Phase 5: 缺口补全 — TUI 渲染 + WebSocket + 响应性 + formatString

**目标:** 补全 Phase 3 和 Phase 4 中遗留的 4 个缺口，使 a2ui-rs 达到 ARCHITECTURE.md 的完整规格。

**前置条件:** Phase 1-4 已完成，以下任务按顺序执行（存在依赖关系）。

---

## 任务总览

| Task | 名称 | 依赖 |
|------|------|------|
| Task 56 | formatString 插值引擎 | 无 |
| Task 57 | DependencyGraph 接入渲染管线 | 无 |
| Task 58 | render() 帧 — 组件树遍历 + Widget 生成 | Task 57 |
| Task 59 | render() 帧 — Layout 计算 + Frame 绘制 | Task 58 |
| Task 60 | WebSocketTransport | 无 |

---

## Task 56: formatString 插值引擎

**Files:**
- Create: `crates/a2ui-renderer/src/format_string.rs`
- Modify: `crates/a2ui-renderer/src/lib.rs`

**Interfaces:**
- Consumes: `PathResolver`, `FunctionDispatcher`
- Produces: `FormatString::resolve()`

### Step 1: 编写失败的测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use a2ui_core::DataModel;
    use serde_json::json;

    #[test]
    fn test_resolve_literal() {
        let dm = DataModel::new(json!({}));
        let resolver = PathResolver::new(dm);
        let result = FormatString::resolve("hello", &resolver, &FunctionDispatcher::new());
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_resolve_path_interpolation() {
        let dm = DataModel::new(json!({"user": {"name": "Alice"}}));
        let resolver = PathResolver::new(dm);
        let dispatcher = FunctionDispatcher::new();
        let result = FormatString::resolve("Hello, ${user/name}!", &resolver, &dispatcher);
        assert_eq!(result, "Hello, Alice!");
    }

    #[test]
    fn test_resolve_function_call() {
        let dm = DataModel::new(json!({}));
        let resolver = PathResolver::new(dm);
        let mut dispatcher = FunctionDispatcher::new();
        dispatcher.register("upper".into(), CallableFrom::ClientOrRemote, |args| {
            let s = args.get("value").and_then(|v| v.as_str()).unwrap_or("");
            Ok(json!(s.to_uppercase()))
        });
        let result = FormatString::resolve("${upper:value=hello}", &resolver, &dispatcher);
        assert_eq!(result, "HELLO");
    }

    #[test]
    fn test_resolve_multiple_interpolations() {
        let dm = DataModel::new(json!({"first": "Alice", "last": "Bob"}));
        let resolver = PathResolver::new(dm);
        let dispatcher = FunctionDispatcher::new();
        let result = FormatString::resolve("${first} ${last}", &resolver, &dispatcher);
        assert_eq!(result, "Alice Bob");
    }

    #[test]
    fn test_resolve_unknown_path_returns_empty() {
        let dm = DataModel::new(json!({}));
        let resolver = PathResolver::new(dm);
        let dispatcher = FunctionDispatcher::new();
        let result = FormatString::resolve("Hello, ${missing/path}!", &resolver, &dispatcher);
        assert_eq!(result, "Hello, !");
    }

    #[test]
    fn test_resolve_unknown_function_returns_empty() {
        let dm = DataModel::new(json!({}));
        let resolver = PathResolver::new(dm);
        let dispatcher = FunctionDispatcher::new();
        let result = FormatString::resolve("${unknownFunc:value=x}", &resolver, &dispatcher);
        assert_eq!(result, "");
    }
}
```

### Step 2: 运行测试确认失败

```bash
cargo test -p a2ui-renderer format_string::tests --no-run 2>&1
```

Expected: 编译失败（`FormatString` 未定义）

### Step 3: 实现 FormatString

```rust
use crate::data_binding::DataBinding;
use crate::function_dispatcher::{CallableFrom, FunctionDispatcher};
use crate::path_resolver::PathResolver;
use a2ui_core::DataModel;
use regex::Regex;

/// formatString 插值解析器
///
/// 支持两种插值语法：
/// - `${path}` — JSON Pointer 路径，从 DataModel 解析值
/// - `${funcName:key=value}` — 调用注册的函数
///
/// 字面量文本原样保留。
pub struct FormatString;

impl FormatString {
    /// 解析模板字符串，返回插值后的结果
    pub fn resolve(
        template: &str,
        resolver: &PathResolver,
        dispatcher: &FunctionDispatcher,
    ) -> String {
        lazy_static::lazy_static! {
            static ref RE: Regex = Regex::new(r"\$\{([^}]+)\}").unwrap();
        }

        RE.replace_all(template, |caps: &regex::Captures| {
            let expr = caps.get(1).unwrap().as_str();

            // 尝试解析为函数调用：funcName:key=value,key2=value2
            if let Some(colon_pos) = expr.find(':') {
                let func_name = &expr[..colon_pos];
                let args_str = &expr[colon_pos + 1..];

                if dispatcher.can_call_from(func_name, CallableFrom::ClientOrRemote) {
                    let args = parse_function_args(args_str);
                    if let Ok(value) = dispatcher.dispatch(func_name, args) {
                        return value.to_string();
                    }
                }
                // 函数不可用或执行失败 → 返回空
                return "".into();
            }

            // 否则解析为 JSON Pointer 路径
            match resolver.resolve(expr) {
                Some(value) => value.to_string(),
                None => "".into(),
            }
        })
        .into_owned()
    }
}

/// 解析函数参数字符串 "key=value,key2=value2" → HashMap
fn parse_function_args(s: &str) -> std::collections::HashMap<String, serde_json::Value> {
    let mut args = std::collections::HashMap::new();
    for pair in s.split(',') {
        if let Some(eq) = pair.find('=') {
            let key = pair[..eq].trim().to_string();
            let val = pair[eq + 1..].trim().to_string();
            args.insert(key, serde_json::Value::String(val));
        }
    }
    args
}
```

### Step 4: 运行测试确认通过

```bash
cargo test -p a2ui-renderer format_string::tests
```

### Step 5: Commit

```bash
git add crates/a2ui-renderer/src/format_string.rs crates/a2ui-renderer/src/lib.rs crates/a2ui-renderer/Cargo.toml
git commit -m "feat(a2ui-renderer): implement formatString interpolation engine"
```

---

## Task 57: DependencyGraph 接入渲染管线

**Files:**
- Modify: `crates/a2ui-renderer-tui/src/tui_renderer.rs`
- Modify: `crates/a2ui-renderer/src/dependency_graph.rs`

**Interfaces:**
- Consumes: `DependencyGraph`, `ComponentForest`, `DataBinding`
- Produces: 响应式 `render()` 实现

### Step 1: 编写失败的测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use a2ui_core::prelude::*;

    #[test]
    fn test_render_only_affected_components() {
        let mut renderer = TuiRenderer::new();
        // 创建两个不相关的组件
        let comp_a = Component::text(
            ComponentId::new("name_label").unwrap(),
            DynamicValue::Path { path: "/user/name".into() },
        );
        let comp_b = Component::text(
            ComponentId::new("count_label").unwrap(),
            DynamicValue::Path { path: "/user/count".into() },
        );
        renderer.create_surface(CreateSurface {
            surface_id: "s1".into(),
            catalog_id: "basic".into(),
            surface_properties: None,
            send_data_model: false,
            components: Some(vec![comp_a, comp_b]),
            data_model: None,
        }).await.unwrap();

        // 更新 /user/name 路径
        renderer.update_data_model(UpdateDataModel {
            surface_id: "s1".into(),
            path: Some("/user/name".into()),
            value: Some(json!("Alice")),
        }).await.unwrap();

        // 验证：只有 name_label 被标记为需要重渲染
        let affected = renderer.dependency_graph()
            .dependents("/user/name");
        assert!(affected.contains(&ComponentId::new("name_label").unwrap()));
        assert!(!affected.contains(&ComponentId::new("count_label").unwrap()));
    }
}
```

### Step 2: 运行测试确认失败

```bash
cargo test -p a2ui-renderer-tui reactive::tests --no-run 2>&1
```

### Step 3: 实现响应式渲染管线

在 `TuiRenderer` 中：
1. `create_surface` 时遍历所有组件的 `DynamicValue`，注册依赖关系到 `DependencyGraph`
2. `update_data_model` 时查询 `DependencyGraph::on_data_change(path)`，只重渲染受影响组件
3. `render()` 使用增量渲染策略

### Step 4: 运行测试确认通过

```bash
cargo test -p a2ui-renderer-tui reactive::tests
```

### Step 5: Commit

```bash
git add crates/a2ui-renderer-tui/src/tui_renderer.rs
git commit -m "feat(a2ui-renderer-tui): integrate DependencyGraph into render pipeline"
```

---

## Task 58: render() 帧 — 组件树遍历 + Widget 生成

**Files:**
- Create: `crates/a2ui-renderer-tui/src/widget_builder.rs`
- Modify: `crates/a2ui-renderer-tui/src/tui_renderer.rs`

**Interfaces:**
- Consumes: `ComponentForest`, `WidgetMapper`, `DataBinding`
- Produces: `WidgetBuilder::build_tree()` → `Vec<RenderableWidget>`

### Step 1: 编写失败的测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use a2ui_core::prelude::*;
    use ratatui::layout::Rect;

    #[test]
    fn test_build_widget_tree_from_components() {
        let forest = ComponentForest::new();
        let mut binding = DataBinding::new(DataModel::new(json!({"title": "Hello"})));

        let root = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("title").unwrap()],
        );
        let title = Component::text(
            ComponentId::new("title").unwrap(),
            DynamicValue::Path { path: "/title".into() },
        );

        forest.upsert("s1", root).unwrap();
        forest.upsert("s1", title).unwrap();

        let mapper = WidgetMapper;
        let builder = WidgetBuilder::new(&mapper, &binding);
        let widgets = builder.build_tree("s1", Rect::new(0, 0, 80, 24));

        assert_eq!(widgets.len(), 2); // root Column + title Text
        assert_eq!(widgets[0].id(), ComponentId::new("root").unwrap());
        assert_eq!(widgets[1].id(), ComponentId::new("title").unwrap());
    }

    #[test]
    fn test_missing_component_renders_placeholder() {
        let forest = ComponentForest::new();
        let binding = DataBinding::new(DataModel::empty());

        let root = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("missing").unwrap()],
        );
        forest.upsert("s1", root).unwrap();

        let mapper = WidgetMapper;
        let builder = WidgetBuilder::new(&mapper, &binding);
        let widgets = builder.build_tree("s1", Rect::new(0, 0, 80, 24));

        // missing 组件应渲染为占位符
        let placeholder = widgets.iter().find(|w| w.id() == "missing");
        assert!(placeholder.is_some());
    }
}
```

### Step 2: 运行测试确认失败

```bash
cargo test -p a2ui-renderer-tui widget_builder::tests --no-run 2>&1
```

### Step 3: 实现 WidgetBuilder

```rust
use a2ui_core::prelude::*;
use ratatui::layout::Rect;

/// 渲染目标 widget（类型 erased）
pub enum RenderableWidget {
    Paragraph { id: ComponentId, area: Rect, text: String, style: ratatui::style::Style },
    Block { id: ComponentId, area: Rect, title: String },
    Gauge { id: ComponentId, area: Rect, ratio: f64 },
    Line { id: ComponentId, area: Rect },
    Placeholder { id: ComponentId, area: Rect, reason: String },
}

impl RenderableWidget {
    pub fn id(&self) -> &ComponentId {
        match self { Self::Placeholder { id, .. } => id, Self::Paragraph { id, .. } => id, /* ... */ }
    }
}

/// 将组件森林构建为渲染目标列表
pub struct WidgetBuilder<'a> {
    mapper: &'a WidgetMapper,
    binding: &'a DataBinding,
}

impl<'a> WidgetBuilder<'a> {
    pub fn new(mapper: &'a WidgetMapper, binding: &'a DataBinding) -> Self {
        Self { mapper, binding }
    }

    /// 从指定 Surface 的根组件开始构建 widget 树
    pub fn build_tree(&self, surface_id: &str, area: Rect) -> Vec<RenderableWidget> {
        let forest = &self.mapper.forest; // 需要访问 ComponentForest
        let root = match forest.get_root(surface_id) {
            Some(c) => c,
            None => return vec![],
        };
        let mut widgets = Vec::new();
        self.build_node(root, area, &mut widgets);
        widgets
    }

    fn build_node(&self, component: &Component, area: Rect, widgets: &mut Vec<RenderableWidget>) {
        // 查找子组件
        let children = self.get_children(component);
        if children.is_empty() {
            widgets.push(self.render_leaf(component, area));
        } else {
            // 布局组件：分配子区域
            let child_areas = self.layout_children(&children, area);
            for (child, child_area) in children.iter().zip(child_areas) {
                self.build_node(child, child_area, widgets);
            }
        }
    }

    fn render_leaf(&self, component: &Component, area: Rect) -> RenderableWidget {
        // 解析 DynamicValue → 文本
        let text = match self.mapper.extract_text(component) {
            Ok(t) => t,
            Err(_) => return RenderableWidget::Placeholder { /* ... */ },
        };
        RenderableWidget::Paragraph { /* ... */ }
    }

    fn get_children(&self, component: &Component) -> Vec<&Component> {
        // 从 ComponentForest 查找子组件
        // ...
        vec![]
    }

    fn layout_children(&self, children: &[&Component], area: Rect) -> Vec<Rect> {
        // 根据组件类型分配子区域
        // Column → 垂直分割，Row → 水平分割
        // ...
        vec![]
    }
}
```

### Step 4: 运行测试确认通过

```bash
cargo test -p a2ui-renderer-tui widget_builder::tests
```

### Step 5: Commit

```bash
git add crates/a2ui-renderer-tui/src/widget_builder.rs crates/a2ui-renderer-tui/src/tui_renderer.rs
git commit -m "feat(a2ui-renderer-tui): implement widget tree builder"
```

---

## Task 59: render() 帧 — Layout 计算 + Frame 绘制

**Files:**
- Modify: `crates/a2ui-renderer-tui/src/tui_renderer.rs`
- Modify: `crates/a2ui-renderer-tui/src/widget_builder.rs`

**Interfaces:**
- Consumes: `WidgetBuilder`, `ratatui::Frame`
- Produces: `TuiRenderer::render()` 实际绘制

### Step 1: 编写失败的测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn test_render_produces_frame_with_widgets() {
        let mut renderer = TuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("title").unwrap(),
            DynamicValue::Literal("Hello".into()),
        );
        // 使用 TestBackend 渲染
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        renderer.create_surface(CreateSurface { /* ... */ }).await.unwrap();
        renderer.render(&mut terminal).await.unwrap();

        // 验证 Frame 中有内容
        let buf = terminal.backend().buffer();
        assert!(buf.area().width > 0);
    }
}
```

### Step 2: 运行测试确认失败

```bash
cargo test -p a2ui-renderer-tui frame::tests --no-run 2>&1
```

### Step 3: 实现 render()

```rust
impl TuiRenderer {
    pub async fn render<B>(&mut self, terminal: &mut Terminal<B>) -> RenderResult<()>
    where
        B: ratatui::backend::Backend,
    {
        terminal.draw(|frame: &mut Frame| {
            let area = frame.area();

            // 遍历所有活跃 Surface
            for (handle, surface_id) in &self.surfaces {
                let builder = WidgetBuilder::new(&self.mapper, &self.bindings[surface_id]);
                let widgets = builder.build_tree(surface_id, area);

                // 绘制每个 widget
                for widget in widgets {
                    self.draw_widget(frame, widget);
                }
            }
        }).map_err(|e| RendererError::BindingError(format!("terminal draw error: {}", e)))?;

        Ok(())
    }

    fn draw_widget(&self, frame: &mut Frame, widget: RenderableWidget) {
        match widget {
            RenderableWidget::Paragraph { area, text, style, .. } => {
                let para = Paragraph::new(text).style(style);
                frame.render_widget(para, area);
            }
            RenderableWidget::Block { area, title, .. } => {
                let block = Block::default().title(title);
                frame.render_widget(block, area);
            }
            // ... 其他 widget 类型
            RenderableWidget::Placeholder { area, reason, .. } => {
                let text = Paragraph::new(format!("[{}]", reason))
                    .style(Style::default().fg(Color::DarkGray));
                frame.render_widget(text, area);
            }
        }
    }
}
```

### Step 4: 运行测试确认通过

```bash
cargo test -p a2ui-renderer-tui frame::tests
```

### Step 5: Commit

```bash
git add crates/aui-renderer-tui/src/tui_renderer.rs
git commit -m "feat(a2ui-renderer-tui): implement render frame with ratatui"
```

---

## Task 60: WebSocketTransport

**Files:**
- Create: `crates/a2ui-transport/src/websocket.rs`
- Modify: `crates/a2ui-transport/Cargo.toml`
- Modify: `crates/a2ui-transport/src/lib.rs`

**Interfaces:**
- Consumes: `Transport` trait
- Produces: `WebSocketTransport`

### Step 1: 添加 tokio-tungstenite 依赖

```toml
# crates/a2ui-transport/Cargo.toml
[dependencies]
# ... 现有依赖
tokio-tungstenite = "0.21"
url = "2.5"
```

### Step 2: 编写失败的测试

```rust
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
```

### Step 3: 实现 WebSocketTransport

```rust
use crate::Transport;
use a2ui_core::{ClientEnvelope, ServerEnvelope};
use async_trait::async_trait;
use thiserror::Error;
use tokio_tungstenite::tungstenite::Error as WsError;

#[derive(Debug, Error)]
pub enum WebSocketError {
    #[error("connection error: {0}")]
    ConnectionError(String),
    #[error("send error: {0}")]
    SendError(String),
    #[error("receive error: {0}")]
    ReceiveError(String),
}

pub type WebSocketResult<T> = Result<T, WebSocketError>;

pub struct WebSocketTransport {
    url: url::Url,
    // WebSocket 连接在 connect 时建立
}

impl WebSocketTransport {
    pub fn new(url: impl AsRef<str>) -> WebSocketResult<Self> {
        let url = url::Url::parse(url.as_ref())
            .map_err(|e| WebSocketError::ConnectionError(format!("invalid URL: {}", e)))?;
        Ok(Self { url })
    }
}

#[async_trait::async_trait]
impl Transport for WebSocketTransport {
    async fn connect(&mut self) -> WebSocketResult<()> {
        let (_, _) = tokio_tungstenite::connect_async(&self.url)
            .await
            .map_err(|e| WebSocketError::ConnectionError(format!("{}", e)))?;
        Ok(())
    }

    async fn send(&mut self, envelope: ClientEnvelope) -> WebSocketResult<()> {
        let json = serde_json::to_string(&envelope)
            .map_err(|e| WebSocketError::SendError(format!("serialization: {}", e)))?;
        // 发送 WebSocket 文本帧
        // ...
        Ok(())
    }

    async fn receive(&mut self) -> WebSocketResult<ServerEnvelope> {
        // 接收 WebSocket 文本帧并反序列化
        // ...
        todo!()
    }

    async fn close(&mut self) -> WebSocketResult<()> {
        // 关闭 WebSocket 连接
        Ok(())
    }
}
```

### Step 4: 运行测试确认通过

```bash
cargo test -p a2ui-transport websocket::tests
```

### Step 5: Commit

```bash
git add crates/a2ui-transport/src/websocket.rs crates/a2ui-transport/Cargo.toml crates/a2ui-transport/src/lib.rs
git commit -m "feat(a2ui-transport): implement WebSocketTransport"
```

---

## 后续工作（超出本次计划范围）

以下架构要求已规划但**不在本次执行范围内**，标注为后续迭代：

| 工作 | 说明 |
|------|------|
| a2ui-renderer-gui | egui/tao 桌面渲染器，Phase 4 之后实现 |
| a2ui-renderer-web | WASM 或服务端渲染，Phase 4 之后实现 |
| 能力协商握手 | 服务端/客户端能力声明，需 Transport 层扩展 |
| AG-UI / A2A / MCP 绑定 | 传输协议适配层 |
| 虚拟滚动 | List 组件的虚拟滚动（TUI 性能优化） |
| 校验错误 UI | checks 失败时的视觉反馈和自动禁用 |
| 响应式剪裁 | 终端宽度有限时的组件裁剪策略 |

## 每次提交规范

```bash
git add <具体文件>
git commit -m "type(scope): description"
```

类型：`feat` / `fix` / `test` / `docs` / `refactor` / `chore`

## 验证检查点

每个 Phase 结束后运行:
```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt -- --check
```
