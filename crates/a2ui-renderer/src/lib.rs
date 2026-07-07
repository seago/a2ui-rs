//! A2UI Renderer — 渲染器抽象层
//!
//! 定义 `Renderer` trait、组件树管理、路径解析、函数调度等核心抽象。
//! 具体渲染 API 由各平台 crate 实现。

pub mod catalog_registry;
pub mod component_forest;
pub mod custom_component;
pub mod data_binding;
pub mod dependency_graph;
pub mod dynamic_value;
pub mod error;
pub mod format_string;
pub mod function_dispatcher;
pub mod input_writeback;
pub mod path_resolver;
pub mod renderer;
pub mod renderer_core;
pub mod style;
pub mod surface_lru;

pub use catalog_registry::CatalogRegistry;
pub use component_forest::ComponentForest;
pub use custom_component::{CustomComponentDef, CustomComponentRegistry};
pub use data_binding::DataBinding;
pub use dependency_graph::DependencyGraph;
#[allow(deprecated)] // 旧 &Value 入参函数按禁删约定保留并继续 re-export
pub use dynamic_value::{
    resolve_bool, resolve_dynamic_string_prop, resolve_dynamic_string_prop_with_missing_path,
    resolve_dynamic_string_value, resolve_dynamic_string_value_with_missing_path, resolve_f64,
    resolve_str, resolve_str_with_missing_path, value_to_display_string,
};
pub use error::{RenderResult, RendererError};
pub use function_dispatcher::{CallableFrom, FunctionDispatcher};
pub use input_writeback::{write_back_input, write_back_user_event};
pub use path_resolver::PathResolver;
pub use renderer::{Renderer, SurfaceHandle, UserEvent};
pub use renderer_core::{CoreEffects, RendererCore};
pub use style::{ComponentStyle, StyleColor, StyleSpacing};
pub use surface_lru::SurfaceLru;
