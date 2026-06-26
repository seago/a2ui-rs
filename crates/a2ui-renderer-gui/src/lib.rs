//! A2UI GUI Renderer — 基于 egui 的桌面渲染器实现
//!
//! 将 A2UI 组件映射为 egui widget，处理鼠标/键盘事件并生成 action 消息。

pub mod gui_renderer;
pub mod widget_mapper;

pub use gui_renderer::GuiRenderer;
pub use widget_mapper::WidgetMapper;
