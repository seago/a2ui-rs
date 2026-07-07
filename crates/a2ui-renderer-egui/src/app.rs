use crate::GuiRenderer;
use a2ui_core::message::V1_0ServerMessage;
use a2ui_core::ServerEnvelope;
use a2ui_renderer::{RenderResult, Renderer};
use std::time::Duration;

/// 空闲时的低频轮询间隔。
/// egui 本身不会因为 mpsc 收到消息自动唤醒，所以空闲时仍需周期性检查。
const IDLE_REPAINT_POLL_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RepaintPolicy {
    Immediate,
    After(Duration),
}

/// A2UI eframe 应用
///
/// 将 A2UI 消息处理管道连接到 egui 渲染循环。
/// 通过 channel 接收服务端消息，在每帧 update 中处理并渲染。
pub struct A2uiApp {
    /// A2UI 渲染器核心
    renderer: GuiRenderer,
    /// 接收 A2UI 服务端消息的 channel
    message_rx: tokio::sync::mpsc::UnboundedReceiver<ServerEnvelope>,
    /// 发送客户端 action 消息的 channel
    action_tx: tokio::sync::mpsc::UnboundedSender<a2ui_core::ClientEnvelope>,
}

impl A2uiApp {
    /// 创建新的 A2uiApp
    pub fn new(
        renderer: GuiRenderer,
        message_rx: tokio::sync::mpsc::UnboundedReceiver<ServerEnvelope>,
        action_tx: tokio::sync::mpsc::UnboundedSender<a2ui_core::ClientEnvelope>,
    ) -> Self {
        Self {
            renderer,
            message_rx,
            action_tx,
        }
    }

    /// 获取渲染器的只读引用
    pub fn renderer(&self) -> &GuiRenderer {
        &self.renderer
    }

    /// 获取渲染器的可变引用（用于测试和外部操作）
    pub fn renderer_mut(&mut self) -> &mut GuiRenderer {
        &mut self.renderer
    }

    /// 创建配对的 message sender 和 channel
    /// 返回 (sender, receiver) — sender 可在外部线程用于发送消息
    pub fn create_channel() -> (
        tokio::sync::mpsc::UnboundedSender<ServerEnvelope>,
        tokio::sync::mpsc::UnboundedReceiver<ServerEnvelope>,
    ) {
        tokio::sync::mpsc::unbounded_channel()
    }

    /// 创建配对的 action channel
    pub fn create_action_channel() -> (
        tokio::sync::mpsc::UnboundedSender<a2ui_core::ClientEnvelope>,
        tokio::sync::mpsc::UnboundedReceiver<a2ui_core::ClientEnvelope>,
    ) {
        tokio::sync::mpsc::unbounded_channel()
    }

    /// 处理单个 ServerEnvelope，分发给对应的渲染器方法
    /// 返回是否触发了渲染更新
    fn process_envelope(&mut self, envelope: ServerEnvelope) -> RenderResult<bool> {
        match envelope {
            ServerEnvelope::V1_0(message) => match message {
                V1_0ServerMessage::CreateSurface(msg) => {
                    let surface_id = msg.surface_id.clone();
                    let _handle = pollster::block_on(self.renderer.create_surface(msg))?;
                    Ok(!surface_id.is_empty())
                }
                V1_0ServerMessage::UpdateComponents(msg) => {
                    let surface_id = msg.surface_id.clone();
                    pollster::block_on(self.renderer.update_components(msg))?;
                    Ok(!surface_id.is_empty())
                }
                V1_0ServerMessage::UpdateDataModel(msg) => {
                    pollster::block_on(self.renderer.update_data_model(msg))?;
                    Ok(true)
                }
                V1_0ServerMessage::DeleteSurface(msg) => {
                    pollster::block_on(self.renderer.delete_surface(msg))?;
                    Ok(true)
                }
                V1_0ServerMessage::ActionResponse(msg) => {
                    pollster::block_on(self.renderer.action_response(msg))?;
                    Ok(true)
                }
                V1_0ServerMessage::CallFunction(msg) => {
                    let response = pollster::block_on(self.renderer.call_function(msg))?;
                    let envelope = a2ui_core::ClientEnvelope::v1_0(
                        a2ui_core::message::V1_0ClientMessage::FunctionResponse(response),
                    );
                    let _ = self.action_tx.send(envelope);
                    Ok(true)
                }
                V1_0ServerMessage::Capabilities(_msg) => {
                    // 能力协商消息由传输层处理，渲染器不需要处理
                    Ok(false)
                }
            },
        }
    }

    fn next_repaint_policy(had_updates: bool, emitted_actions: usize) -> RepaintPolicy {
        if had_updates || emitted_actions > 0 {
            RepaintPolicy::Immediate
        } else {
            RepaintPolicy::After(IDLE_REPAINT_POLL_INTERVAL)
        }
    }
}

impl eframe::App for A2uiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut had_updates = false;

        // 1. 处理所有待处理的 A2UI 消息
        while let Ok(envelope) = self.message_rx.try_recv() {
            match self.process_envelope(envelope) {
                Ok(true) => {
                    had_updates = true;
                }
                Ok(false) => {} // 能力协商等不需要重绘
                Err(e) => {
                    tracing::error!("处理消息失败: {}", e);
                }
            }
        }

        // 2. 渲染当前帧，获取用户交互产生的客户端信封
        let mut emitted_actions = 0usize;
        match self.renderer.render_frame(ctx) {
            Ok(envelopes) => {
                emitted_actions = envelopes.len();
                for envelope in envelopes {
                    let _ = self.action_tx.send(envelope);
                }
            }
            Err(e) => {
                tracing::error!("渲染帧失败: {}", e);
            }
        }

        // 3. 按需重绘：
        // - 有消息更新或用户 action 时立即刷新
        // - 空闲时低频轮询消息队列，避免持续满帧占用 CPU
        match Self::next_repaint_policy(had_updates, emitted_actions) {
            RepaintPolicy::Immediate => ctx.request_repaint(),
            RepaintPolicy::After(duration) => ctx.request_repaint_after(duration),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use a2ui_core::message::server_to_client::CreateSurface;
    use a2ui_core::prelude::json;
    use a2ui_core::prelude::*;

    #[test]
    fn test_a2ui_app_new() {
        let renderer = GuiRenderer::new();
        let (_msg_tx, msg_rx) = A2uiApp::create_channel();
        let (action_tx, _action_rx) = A2uiApp::create_action_channel();

        let app = A2uiApp::new(renderer, msg_rx, action_tx);
        assert!(app.renderer().core.surfaces().is_empty());
    }

    #[test]
    fn test_a2ui_app_renderer_access() {
        let mut renderer = GuiRenderer::new();
        renderer.register_function("test_fn", a2ui_renderer::CallableFrom::ClientOrRemote);

        let (_msg_tx, msg_rx) = A2uiApp::create_channel();
        let (action_tx, _action_rx) = A2uiApp::create_action_channel();

        let mut app = A2uiApp::new(renderer, msg_rx, action_tx);
        assert!(app
            .renderer()
            .registered_functions()
            .iter()
            .any(|s| s.as_str() == "test_fn"));
        assert!(app
            .renderer_mut()
            .registered_functions()
            .iter()
            .any(|s| s.as_str() == "test_fn"));
    }

    #[test]
    fn test_process_envelope_create_surface() {
        let renderer = GuiRenderer::new();
        let (_msg_tx, msg_rx) = A2uiApp::create_channel();
        let (action_tx, _action_rx) = A2uiApp::create_action_channel();

        let mut app = A2uiApp::new(renderer, msg_rx, action_tx);

        let envelope =
            a2ui_core::ServerEnvelope::V1_0(V1_0ServerMessage::CreateSurface(CreateSurface {
                surface_id: "test_surface".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![Component::text(
                    ComponentId::new("root").unwrap(),
                    DynamicValue::Literal("Hello".to_string()),
                )]),
                data_model: None,
            }));

        let result = app.process_envelope(envelope);
        assert!(result.is_ok());
        assert!(result.unwrap()); // had_updates = true
                                  // 验证 surface 已创建
        assert_eq!(app.renderer().core.surfaces().len(), 1);
    }

    #[test]
    fn test_process_envelope_call_function() {
        let mut renderer = GuiRenderer::new();
        // 注册一个测试函数
        renderer.register_function("echo", a2ui_renderer::CallableFrom::ClientOrRemote);

        let (_msg_tx, msg_rx) = A2uiApp::create_channel();
        let (action_tx, mut action_rx) = A2uiApp::create_action_channel();

        let mut app = A2uiApp::new(renderer, msg_rx, action_tx);

        let envelope = a2ui_core::ServerEnvelope::V1_0(V1_0ServerMessage::CallFunction(
            a2ui_core::message::server_to_client::CallFunction {
                function_call_id: "fc1".into(),
                want_response: true,
                call: a2ui_core::message::server_to_client::CallFunctionPayload {
                    call: "echo".into(),
                    args: json!({"value": "test"}),
                },
            },
        ));

        let result = app.process_envelope(envelope);
        assert!(result.is_ok());
        // 验证函数响应已发送
        let response = action_rx.try_recv();
        assert!(response.is_ok());
    }

    #[test]
    fn test_process_envelope_capabilities_no_repaint() {
        let renderer = GuiRenderer::new();
        let (_msg_tx, msg_rx) = A2uiApp::create_channel();
        let (action_tx, _action_rx) = A2uiApp::create_action_channel();

        let mut app = A2uiApp::new(renderer, msg_rx, action_tx);

        use a2ui_core::message::Capabilities;
        let envelope =
            a2ui_core::ServerEnvelope::V1_0(V1_0ServerMessage::Capabilities(Capabilities {
                version: "v1.0".into(),
                features: vec![],
            }));

        let result = app.process_envelope(envelope);
        assert!(result.is_ok());
        assert!(!result.unwrap()); // had_updates = false (Capabilities 不需要重绘)
    }

    #[test]
    fn test_next_repaint_policy_immediate_on_updates() {
        assert_eq!(
            A2uiApp::next_repaint_policy(true, 0),
            RepaintPolicy::Immediate
        );
    }

    #[test]
    fn test_next_repaint_policy_immediate_on_actions() {
        assert_eq!(
            A2uiApp::next_repaint_policy(false, 1),
            RepaintPolicy::Immediate
        );
    }

    #[test]
    fn test_next_repaint_policy_idle_poll_when_quiet() {
        assert_eq!(
            A2uiApp::next_repaint_policy(false, 0),
            RepaintPolicy::After(IDLE_REPAINT_POLL_INTERVAL)
        );
    }
}
