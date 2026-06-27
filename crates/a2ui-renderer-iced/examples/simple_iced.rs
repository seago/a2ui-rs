//! A2UI Iced 渲染器 — 简单演示
//!
//! 创建一个带 Text 组件的 Surface 并在桌面窗口中渲染

use a2ui_core::message::server_to_client::CreateSurface;
use a2ui_core::prelude::*;
use a2ui_core::ServerEnvelope;
use a2ui_renderer_iced::app::{self, IcedApp};
use a2ui_renderer_iced::{load_cjk_font, IcedRenderer};
use serde_json::json;

fn main() -> iced::Result {
    tracing_subscriber::fmt::init();
    let cjk_font = load_cjk_font();
    if let Some(font) = &cjk_font {
        tracing::info!("已加载中文字体: {}", font.path);
    } else {
        tracing::warn!("未找到系统中文字体");
    }

    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();

    let mut renderer = IcedRenderer::new();
    renderer.register_function("echo", a2ui_renderer::CallableFrom::ClientOrRemote);

    let catalog: a2ui_core::Catalog = serde_json::from_value(json!({
        "catalogId": "basic",
        "instructions": "Basic catalog",
        "components": {},
        "functions": {}
    }))
    .unwrap();
    renderer.register_catalog(catalog).ok();

    let (msg_tx, msg_rx) = IcedApp::create_channel();
    let (action_tx, mut action_rx) = IcedApp::create_action_channel();

    let app = IcedApp::new(renderer, msg_rx, action_tx);

    // 后台线程发送消息
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(100));

        let root = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello A2UI Iced!".to_string()),
        );

        let envelope = ServerEnvelope::V1_0(a2ui_core::message::V1_0ServerMessage::CreateSurface(
            CreateSurface {
                surface_id: "demo".into(),
                catalog_id: "basic".into(),
                surface_properties: Some(json!({"agentDisplayName": "A2UI Iced Demo"})),
                send_data_model: false,
                components: Some(vec![root]),
                data_model: None,
            },
        ));
        msg_tx.send(envelope).ok();
    });

    // 后台监听 action
    std::thread::spawn(move || {
        while let Some(action) = action_rx.blocking_recv() {
            tracing::info!("收到 action: {:?}", action);
        }
    });

    let app_builder = iced::application("A2UI Iced Demo", app::update, app::view)
        .subscription(app::subscription)
        .window_size(iced::Size::new(400.0, 300.0));

    let app_builder = if let Some(font) = cjk_font {
        app_builder
            .font(font.bytes)
            .default_font(iced::Font::with_name(font.family))
    } else {
        app_builder
    };

    app_builder.run_with(move || (app, iced::Task::none()))
}
