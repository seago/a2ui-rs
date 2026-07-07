//! A2UI Iced 渲染器 — 登录界面演示
//!
//! 展示一个完整的登录表单：用户名、密码（掩码）、记住密码、登录按钮，
//! 以及模拟的服务端登录响应流程。
//!
//! 运行：`cargo run --example login_iced -p a2ui-renderer-iced`

use a2ui_core::message::server_to_client::{CreateSurface, UpdateDataModel};
use a2ui_core::message::{V1_0ClientMessage, V1_0ServerMessage};
use a2ui_core::prelude::*;
use a2ui_core::{ClientEnvelope, ServerEnvelope};
use a2ui_renderer_iced::app::{self, IcedApp};
use a2ui_renderer_iced::{load_cjk_font, IcedRenderer};
use serde_json::json;
use std::time::Duration;

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
    renderer.register_function("required", a2ui_renderer::CallableFrom::ClientOnly);
    renderer.register_function("length", a2ui_renderer::CallableFrom::ClientOnly);

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

    // ── 构建登录界面组件树 ──

    let root: Component = serde_json::from_value(json!({
        "id": "root",
        "component": "Column",
        "children": [
            "title", "div1",
            "username_label", "username_field",
            "password_label", "password_field",
            "remember_cb", "div2",
            "login_btn", "div3",
            "status_text", "footer"
        ]
    }))
    .unwrap();

    let title: Component = serde_json::from_value(json!({
        "id": "title", "component": "Text",
        "text": "🔐  用户登录"
    }))
    .unwrap();

    let username_label: Component = serde_json::from_value(json!({
        "id": "username_label", "component": "Text",
        "text": "用户名"
    }))
    .unwrap();

    let username_field: Component = serde_json::from_value(json!({
        "id": "username_field", "component": "TextField",
        "value": "", "placeholder": "请输入用户名", "variant": "shortText"
    }))
    .unwrap();

    let password_label: Component = serde_json::from_value(json!({
        "id": "password_label", "component": "Text",
        "text": "密码"
    }))
    .unwrap();

    let password_field: Component = serde_json::from_value(json!({
        "id": "password_field", "component": "TextField",
        "value": "", "placeholder": "请输入密码", "variant": "obscured"
    }))
    .unwrap();

    let remember_cb: Component = serde_json::from_value(json!({
        "id": "remember_cb", "component": "CheckBox",
        "value": false, "label": "记住密码"
    }))
    .unwrap();

    let btn_label = Component::text(
        ComponentId::new("btn_label").unwrap(),
        DynamicValue::Literal("登  录".to_string()),
    );
    let login_btn: Component = serde_json::from_value(json!({
        "id": "login_btn", "component": "Button",
        "child": "btn_label", "variant": "primary"
    }))
    .unwrap();

    let status_text: Component = serde_json::from_value(json!({
        "id": "status_text", "component": "Text",
        "text": { "path": "/login_status" }
    }))
    .unwrap();

    let div1: Component =
        serde_json::from_value(json!({"id": "div1", "component": "Divider"})).unwrap();
    let div2: Component =
        serde_json::from_value(json!({"id": "div2", "component": "Divider"})).unwrap();
    let div3: Component =
        serde_json::from_value(json!({"id": "div3", "component": "Divider"})).unwrap();

    let footer: Component = serde_json::from_value(json!({
        "id": "footer", "component": "Text",
        "text": "A2UI Protocol v1.0 · Iced · 演示用途"
    }))
    .unwrap();

    let all_components = vec![
        root,
        title,
        div1,
        div2,
        div3,
        username_label,
        username_field,
        password_label,
        password_field,
        remember_cb,
        btn_label,
        login_btn,
        status_text,
        footer,
    ];

    // ── 后台线程：发送 createSurface ──
    let msg_tx_clone = msg_tx.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        let envelope = ServerEnvelope::V1_0(V1_0ServerMessage::CreateSurface(CreateSurface {
            surface_id: "login".into(),
            catalog_id: "a2ui://catalogs/basic/v1".into(),
            surface_properties: Some(json!({"agentDisplayName": "A2UI Login"})),
            send_data_model: false,
            components: Some(all_components),
            data_model: Some(json!({
                "login_status": "",
                "credentials": { "username": "", "password": "" }
            })),
        }));
        msg_tx_clone.send(envelope).ok();
    });

    // ── 后台线程：处理 action，模拟登录逻辑 ──
    let msg_tx_clone2 = msg_tx.clone();
    std::thread::spawn(move || {
        while let Some(envelope) = action_rx.blocking_recv() {
            let action = match envelope {
                ClientEnvelope::V1_0 {
                    message: V1_0ClientMessage::Action(a),
                    ..
                } => a,
                _ => continue,
            };
            tracing::info!("收到 action: {:?}", action);

            if action.name == "click" {
                msg_tx_clone2
                    .send(ServerEnvelope::V1_0(V1_0ServerMessage::UpdateDataModel(
                        UpdateDataModel {
                            surface_id: "login".into(),
                            path: Some("/login_status".into()),
                            value: Some(json!("⏳ 正在登录...")),
                        },
                    )))
                    .ok();

                let tx = msg_tx_clone2.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(1500));
                    let status = "✅ 登录成功！欢迎回来";
                    tx.send(ServerEnvelope::V1_0(V1_0ServerMessage::UpdateDataModel(
                        UpdateDataModel {
                            surface_id: "login".into(),
                            path: Some("/login_status".into()),
                            value: Some(json!(status)),
                        },
                    )))
                    .ok();
                });
            }
        }
    });

    let app_builder = iced::application("A2UI 登录演示", app::update, app::view)
        .subscription(app::subscription)
        .window_size(iced::Size::new(420.0, 520.0));

    let app_builder = if let Some(font) = cjk_font {
        app_builder
            .font(font.bytes)
            .default_font(iced::Font::with_name(font.family))
    } else {
        app_builder
    };

    app_builder.run_with(move || (app, iced::Task::none()))
}
