//! A2UI Renderer — 渲染器抽象层
//!
//! 定义 `Renderer` trait、组件树管理、路径解析、函数调度等核心抽象。
//! 具体渲染 API 由各平台 crate 实现。

pub mod catalog_registry;
pub mod component_forest;
pub mod custom_component;
pub mod data_binding;
pub mod dependency_graph;
pub mod error;
pub mod format_string;
pub mod function_dispatcher;
pub mod path_resolver;
pub mod renderer;
pub mod surface_lru;

pub use catalog_registry::CatalogRegistry;
pub use custom_component::{CustomComponentDef, CustomComponentRegistry};
pub use component_forest::ComponentForest;
pub use data_binding::DataBinding;
pub use dependency_graph::DependencyGraph;
pub use error::{RenderResult, RendererError};
pub use function_dispatcher::{CallableFrom, FunctionDispatcher};
pub use path_resolver::PathResolver;
pub use renderer::{Renderer, SurfaceHandle, UserEvent};
pub use surface_lru::SurfaceLru;
