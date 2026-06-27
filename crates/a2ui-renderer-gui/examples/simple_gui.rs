//! A2UI GUI 渲染器 -- 简单演示
//!
//! 创建一个带 Text 组件的 Surface 并在桌面窗口中渲染

use a2ui_core::message::server_to_client::CreateSurface;
use a2ui_core::prelude::*;
use a2ui_core::ServerEnvelope;
use a2ui_renderer_gui::{A2uiApp, GuiRenderer};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化 tracing 日志
    tracing_subscriber::fmt::init();

    // 创建 tokio runtime
    let rt = tokio::runtime::Runtime::new()?;
    let _guard = rt.enter();

    // 创建渲染器并注册函数
    let mut renderer = GuiRenderer::new();
    renderer.register_function("echo", a2ui_renderer::CallableFrom::ClientOrRemote);
    renderer.register_function("formatString", a2ui_renderer::CallableFrom::ClientOrRemote);

    // 注册 Basic Catalog
    let catalog: a2ui_core::Catalog = serde_json::from_value(json!({
        "catalogId": "basic",
        "instructions": "Basic catalog",
        "components": {},
        "functions": {}
    }))?;
    renderer.register_catalog(catalog).ok();

    // 创建 channel
    let (msg_tx, msg_rx) = A2uiApp::create_channel();
    let (action_tx, mut action_rx) = A2uiApp::create_action_channel();

    // 创建 App
    let app = A2uiApp::new(renderer, msg_rx, action_tx);

    // 在后台线程中发送 A2UI 消息（模拟服务端）
    let msg_tx_clone = msg_tx.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(100));

        // 创建 Surface
        let root = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello A2UI GUI!".to_string()),
        );

        let envelope = ServerEnvelope::V1_0(
            a2ui_core::message::V1_0ServerMessage::CreateSurface(CreateSurface {
                surface_id: "demo".into(),
                catalog_id: "basic".into(),
                surface_properties: Some(json!({"agentDisplayName": "A2UI Demo"})),
                send_data_model: false,
                components: Some(vec![root]),
                data_model: None,
            }),
        );
        msg_tx_clone.send(envelope).ok();
    });

    // 后台线程监听 action 消息
    std::thread::spawn(move || {
        while let Some(action) = action_rx.blocking_recv() {
            tracing::info!("收到 action: {:?}", action);
        }
    });

    // 启动 eframe 窗口
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title("A2UI GUI Demo"),
        ..Default::default()
    };

    eframe::run_native(
        "A2UI GUI Demo",
        options,
        Box::new(|_cc| Box::new(app)),
    )
    .map_err(|e| format!("eframe 错误: {}", e))?;

    Ok(())
}
