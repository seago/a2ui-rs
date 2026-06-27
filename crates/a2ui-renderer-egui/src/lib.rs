//! A2UI GUI Renderer — 基于 egui 的桌面渲染器实现
//!
//! 将 A2UI 组件映射为 egui widget，处理鼠标/键盘事件并生成 action 消息。
//!
//! ## 使用示例
//!
//! ```rust,ignore
//! use a2ui_renderer_egui::{A2uiApp, GuiRenderer};
//!
//! let rt = tokio::runtime::Runtime::new().unwrap();
//! let _guard = rt.enter();
//!
//! let renderer = GuiRenderer::new();
//! let (msg_tx, msg_rx) = A2uiApp::create_channel();
//! let (action_tx, _action_rx) = A2uiApp::create_action_channel();
//!
//! let app = A2uiApp::new(renderer, msg_rx, action_tx);
//! let options = eframe::NativeOptions::default();
//! eframe::run_native("A2UI", options, Box::new(|_cc| Box::new(app)));
//! ```

pub mod app;
pub mod gui_renderer;
pub mod widget_mapper;

pub use app::A2uiApp;
pub use gui_renderer::GuiRenderer;
pub use widget_mapper::WidgetMapper;
