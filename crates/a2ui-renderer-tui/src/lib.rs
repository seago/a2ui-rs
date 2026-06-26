//! A2UI TUI Renderer — 基于 ratatui + crossterm 的终端渲染器实现
//!
//! 将 A2UI 组件映射为 ratatui widget，处理键盘事件并生成 action 消息。

pub mod focus_manager;
pub mod input_handler;
pub mod tui_renderer;
pub mod widget_mapper;

pub use focus_manager::FocusManager;
pub use input_handler::InputHandler;
pub use tui_renderer::TuiRenderer;
pub use widget_mapper::WidgetMapper;
