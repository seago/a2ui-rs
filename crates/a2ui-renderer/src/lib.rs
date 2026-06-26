//! A2UI Renderer — 渲染器抽象层
//!
//! 定义 `Renderer` trait、组件树管理、路径解析、函数调度等核心抽象。
//! 具体渲染 API 由各平台 crate 实现。

pub mod error;
pub mod renderer;
pub mod component_forest;
pub mod data_binding;
pub mod path_resolver;
pub mod function_dispatcher;
pub mod catalog_registry;
pub mod dependency_graph;

pub use error::{RendererError, RenderResult};
pub use renderer::Renderer;
pub use component_forest::ComponentForest;
pub use data_binding::DataBinding;
pub use path_resolver::PathResolver;
pub use function_dispatcher::{FunctionDispatcher, CallableFrom};
pub use catalog_registry::CatalogRegistry;
pub use dependency_graph::DependencyGraph;
