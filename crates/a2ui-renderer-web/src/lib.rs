//! A2UI Web Renderer — 基于 HTML 的 Web 渲染器实现
//!
//! 将 A2UI 组件映射为 HTML 元素，支持服务端渲染和 WASM 前端渲染。
//!
//! 所有 18 个 Basic Catalog 组件均可完整渲染为 HTML（浏览器原生支持 Image/Video/Audio）。

pub mod html_builder;
pub mod web_renderer;

pub use html_builder::HtmlBuilder;
pub use web_renderer::WebRenderer;
