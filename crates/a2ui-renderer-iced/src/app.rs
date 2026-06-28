use crate::iced_renderer::IcedRenderer;
use a2ui_core::message::V1_0ServerMessage;
use a2ui_core::prelude::*;
use a2ui_core::{ClientEnvelope, ServerEnvelope};
use a2ui_renderer::{RenderResult, Renderer};
use std::sync::{Arc, Mutex};

/// 用户交互动作（框架无关）
#[derive(Debug, Clone)]
pub enum UserAction {
    Click {
        component_id: ComponentId,
    },
    TextInput {
        component_id: ComponentId,
        value: String,
    },
    CheckToggle {
        component_id: ComponentId,
        checked: bool,
    },
    SliderChange {
        component_id: ComponentId,
        value: f64,
    },
}

/// 驱动 iced Application 的消息
#[derive(Debug, Clone)]
pub enum Message {
    /// 来自通道的服务端消息
    ServerMessage(ServerEnvelope),
    /// 用户交互
    UserAction(UserAction),
}

/// Iced 应用状态
pub struct IcedApp {
    pub renderer: IcedRenderer,
    message_rx: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<ServerEnvelope>>>>,
    action_tx: tokio::sync::mpsc::UnboundedSender<ClientEnvelope>,
}

impl IcedApp {
    pub fn new(
        renderer: IcedRenderer,
        message_rx: tokio::sync::mpsc::UnboundedReceiver<ServerEnvelope>,
        action_tx: tokio::sync::mpsc::UnboundedSender<ClientEnvelope>,
    ) -> Self {
        Self {
            renderer,
            message_rx: Arc::new(Mutex::new(Some(message_rx))),
            action_tx,
        }
    }

    pub fn create_channel() -> (
        tokio::sync::mpsc::UnboundedSender<ServerEnvelope>,
        tokio::sync::mpsc::UnboundedReceiver<ServerEnvelope>,
    ) {
        tokio::sync::mpsc::unbounded_channel()
    }

    pub fn create_action_channel() -> (
        tokio::sync::mpsc::UnboundedSender<ClientEnvelope>,
        tokio::sync::mpsc::UnboundedReceiver<ClientEnvelope>,
    ) {
        tokio::sync::mpsc::unbounded_channel()
    }

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
                    let envelope = ClientEnvelope::V1_0(
                        a2ui_core::message::V1_0ClientMessage::FunctionResponse(response),
                    );
                    let _ = self.action_tx.send(envelope);
                    Ok(true)
                }
                V1_0ServerMessage::Capabilities(_) => Ok(false),
            },
        }
    }
}

/// iced update 函数 — 处理消息并返回 Task
pub fn update(app: &mut IcedApp, message: Message) -> iced::Task<Message> {
    match message {
        Message::ServerMessage(envelope) => {
            match app.process_envelope(envelope) {
                Ok(_) => {}
                Err(e) => tracing::error!("处理消息失败: {}", e),
            }
            iced::Task::none()
        }
        Message::UserAction(action) => {
            use a2ui_core::prelude::DynamicValue;
            use std::collections::HashMap;
            match action {
                UserAction::Click { component_id } => {
                    let msg = a2ui_core::message::client_to_server::ActionMessage {
                        name: "click".into(),
                        surface_id: String::new(),
                        source_component_id: Some(component_id.as_str().to_string()),
                        context: HashMap::new(),
                        want_response: false,
                        response_path: None,
                        action_id: None,
                    };
                    let envelope =
                        ClientEnvelope::V1_0(a2ui_core::message::V1_0ClientMessage::Action(msg));
                    let _ = app.action_tx.send(envelope);
                }
                UserAction::TextInput {
                    component_id,
                    value,
                } => {
                    // 更新本地输入状态（iced 的受控组件需要）
                    app.renderer
                        .text_input_values
                        .borrow_mut()
                        .insert(component_id.as_str().to_string(), value.clone());
                    let mut ctx = HashMap::new();
                    ctx.insert(
                        "value".into(),
                        DynamicValue::Literal(serde_json::Value::String(value)),
                    );
                    let msg = a2ui_core::message::client_to_server::ActionMessage {
                        name: "text_input".into(),
                        surface_id: String::new(),
                        source_component_id: Some(component_id.as_str().to_string()),
                        context: ctx,
                        want_response: false,
                        response_path: None,
                        action_id: None,
                    };
                    let envelope =
                        ClientEnvelope::V1_0(a2ui_core::message::V1_0ClientMessage::Action(msg));
                    let _ = app.action_tx.send(envelope);
                }
                UserAction::CheckToggle {
                    component_id,
                    checked,
                } => {
                    app.renderer
                        .checkbox_values
                        .borrow_mut()
                        .insert(component_id.as_str().to_string(), checked);
                    let mut ctx = HashMap::new();
                    ctx.insert(
                        "checked".into(),
                        DynamicValue::Literal(serde_json::Value::Bool(checked)),
                    );
                    let msg = a2ui_core::message::client_to_server::ActionMessage {
                        name: "check_toggle".into(),
                        surface_id: String::new(),
                        source_component_id: Some(component_id.as_str().to_string()),
                        context: ctx,
                        want_response: false,
                        response_path: None,
                        action_id: None,
                    };
                    let envelope =
                        ClientEnvelope::V1_0(a2ui_core::message::V1_0ClientMessage::Action(msg));
                    let _ = app.action_tx.send(envelope);
                }
                UserAction::SliderChange {
                    component_id,
                    value,
                } => {
                    app.renderer
                        .slider_values
                        .borrow_mut()
                        .insert(component_id.as_str().to_string(), value);
                    let mut ctx = HashMap::new();
                    ctx.insert(
                        "value".into(),
                        DynamicValue::Literal(serde_json::Value::Number(
                            serde_json::Number::from_f64(value).unwrap_or(0.into()),
                        )),
                    );
                    let msg = a2ui_core::message::client_to_server::ActionMessage {
                        name: "slider_change".into(),
                        surface_id: String::new(),
                        source_component_id: Some(component_id.as_str().to_string()),
                        context: ctx,
                        want_response: false,
                        response_path: None,
                        action_id: None,
                    };
                    let envelope =
                        ClientEnvelope::V1_0(a2ui_core::message::V1_0ClientMessage::Action(msg));
                    let _ = app.action_tx.send(envelope);
                }
            }
            iced::Task::none()
        }
    }
}

/// iced view 函数 — 从组件树构建 Element（所有 widget 数据已 clone，返回 'static）
pub fn view(app: &IcedApp) -> iced::Element<'_, Message> {
    if app.renderer.surface_order.is_empty() {
        return padded_surface(
            iced::widget::text("A2UI Iced — 等待 Surface...")
                .size(24)
                .shaping(iced::widget::text::Shaping::Advanced)
                .into(),
        );
    }

    let surface_id = &app.renderer.surface_order[0];

    // 构建组件树（临时变量，但 Element 中所有数据已 clone 为 'static）
    if let Ok(root) = app.renderer.forest.build_tree(surface_id) {
        padded_surface(crate::widget_mapper::build_element_tree(
            &root,
            &app.renderer,
            surface_id,
        ))
    } else {
        padded_surface(
            iced::widget::text("构建组件树中...")
                .shaping(iced::widget::text::Shaping::Advanced)
                .into(),
        )
    }
}

fn padded_surface(content: iced::Element<'_, Message>) -> iced::Element<'_, Message> {
    iced::widget::container(content)
        .padding([16, 20])
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .clip(true)
        .into()
}

/// iced subscription — 等待服务端消息，有消息时才唤醒 update
pub fn subscription(app: &IcedApp) -> iced::Subscription<Message> {
    let message_rx = Arc::clone(&app.message_rx);

    iced::Subscription::run_with_id(
        "a2ui-iced-server-messages",
        iced::stream::channel(100, move |mut output| async move {
            use iced::futures::SinkExt;

            let mut receiver = {
                let mut guard = match message_rx.lock() {
                    Ok(guard) => guard,
                    Err(error) => {
                        tracing::error!("消息通道锁已损坏: {}", error);
                        return;
                    }
                };

                match guard.take() {
                    Some(receiver) => receiver,
                    None => return,
                }
            };

            while let Some(envelope) = receiver.recv().await {
                if output.send(Message::ServerMessage(envelope)).await.is_err() {
                    break;
                }
            }
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use a2ui_core::message::server_to_client::CreateSurface;
    use serde_json::json;

    #[test]
    fn test_iced_app_new() {
        let renderer = IcedRenderer::new();
        let (_msg_tx, msg_rx) = IcedApp::create_channel();
        let (action_tx, _action_rx) = IcedApp::create_action_channel();
        let app = IcedApp::new(renderer, msg_rx, action_tx);
        assert!(app.renderer.surfaces.is_empty());
    }

    #[test]
    fn test_process_envelope_create_surface() {
        let renderer = IcedRenderer::new();
        let (_msg_tx, msg_rx) = IcedApp::create_channel();
        let (action_tx, _action_rx) = IcedApp::create_action_channel();
        let mut app = IcedApp::new(renderer, msg_rx, action_tx);

        let envelope = ServerEnvelope::V1_0(V1_0ServerMessage::CreateSurface(CreateSurface {
            surface_id: "test".into(),
            catalog_id: "a2ui://catalogs/basic/v1".into(),
            surface_properties: None,
            send_data_model: false,
            components: Some(vec![Component::text(
                ComponentId::new("root").unwrap(),
                DynamicValue::Literal("Hello Iced".to_string()),
            )]),
            data_model: None,
        }));

        let result = app.process_envelope(envelope);
        assert!(result.is_ok());
        assert_eq!(app.renderer.surfaces.len(), 1);
    }

    #[test]
    fn test_process_envelope_call_function() {
        let mut renderer = IcedRenderer::new();
        renderer.register_function("echo", a2ui_renderer::CallableFrom::ClientOrRemote);
        let (_msg_tx, msg_rx) = IcedApp::create_channel();
        let (action_tx, mut action_rx) = IcedApp::create_action_channel();
        let mut app = IcedApp::new(renderer, msg_rx, action_tx);

        let envelope = ServerEnvelope::V1_0(V1_0ServerMessage::CallFunction(
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
        let response = action_rx.try_recv();
        assert!(response.is_ok());
    }
}
