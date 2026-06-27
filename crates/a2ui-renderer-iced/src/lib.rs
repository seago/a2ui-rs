//! A2UI Iced Renderer — 基于 Iced 框架的桌面渲染器实现
//!
//! 采用 Elm Architecture（保留模式），将 A2UI 组件映射为 Iced widget 树。
//!
//! ## 使用示例
//!
//! ```rust,ignore
//! use a2ui_renderer_iced::{IcedApp, IcedRenderer, run_iced_app};
//!
//! let renderer = IcedRenderer::new();
//! let (msg_tx, msg_rx) = IcedApp::create_channel();
//! let (action_tx, _action_rx) = IcedApp::create_action_channel();
//!
//! let app = IcedApp::new(renderer, msg_rx, action_tx);
//! run_iced_app(app, "A2UI Iced Demo", [800.0, 600.0])?;
//! ```

pub mod app;
pub mod fonts;
pub mod iced_renderer;
pub mod widget_mapper;

pub use app::IcedApp;
pub use fonts::{load_cjk_font, LoadedFont};
pub use iced_renderer::IcedRenderer;
