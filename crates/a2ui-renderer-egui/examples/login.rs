//! A2UI GUI 渲染器 — 登录界面演示
//!
//! 展示一个完整的登录表单：用户名、密码（掩码）、记住密码、登录按钮，
//! 以及模拟的服务端登录响应流程。
//!
//! 运行：`cargo run --example login -p a2ui-renderer-egui`

use a2ui_core::message::server_to_client::{CreateSurface, UpdateDataModel};
use a2ui_core::message::{V1_0ClientMessage, V1_0ServerMessage};
use a2ui_core::prelude::*;
use a2ui_core::{ClientEnvelope, ServerEnvelope};
use a2ui_renderer_egui::{A2uiApp, GuiRenderer};
use serde_json::json;
use std::time::Duration;

/// 加载系统中文字体，使 egui 能正确渲染中文
fn setup_chinese_fonts(cc: &eframe::CreationContext) {
    // macOS / Linux / Windows 常见中文字体路径
    let font_paths = [
        "/System/Library/Fonts/PingFang.ttc",            // macOS 苹方
        "/System/Library/Fonts/STHeiti Light.ttc",       // macOS 黑体
        "/System/Library/Fonts/Supplemental/Songti.ttc", // macOS 宋体
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", // Linux Noto CJK
        "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf", // Linux Droid
        "C:\\Windows\\Fonts\\msyh.ttc",                  // Windows 微软雅黑
        "C:\\Windows\\Fonts\\simsun.ttc",                // Windows 宋体
    ];

    for path in &font_paths {
        if let Ok(data) = std::fs::read(path) {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "ChineseFont".to_owned(),
                egui::FontData::from_owned(data).tweak(egui::FontTweak {
                    scale: 1.0,
                    ..Default::default()
                }),
            );
            // 将中文字体放在 Proportional 家族最前面，fallback 到默认字体
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "ChineseFont".to_owned());

            cc.egui_ctx.set_fonts(fonts);
            tracing::info!("已加载中文字体: {}", path);
            return;
        }
    }
    tracing::warn!("未找到系统中文字体，中文将显示为乱码");
}

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
    renderer.register_function("required", a2ui_renderer::CallableFrom::ClientOnly);
    renderer.register_function("length", a2ui_renderer::CallableFrom::ClientOnly);
    renderer.register_function("openUrl", a2ui_renderer::CallableFrom::ClientOnly);

    // Basic Catalog 已由 CatalogRegistry::with_defaults() 自动加载
    // 创建 channel
    let (msg_tx, msg_rx) = A2uiApp::create_channel();
    let (action_tx, mut action_rx) = A2uiApp::create_action_channel();

    // 创建 App
    let app = A2uiApp::new(renderer, msg_rx, action_tx);

    // ============================================================
    // 构建登录界面组件树
    // ============================================================

    // --- 根容器：Column ---
    let root: Component = serde_json::from_value(json!({
        "id": "root",
        "component": "Column",
        "children": { "children": [
            "title",
            "div1",
            "username_label",
            "username_field",
            "password_label",
            "password_field",
            "remember_cb",
            "div2",
            "login_btn",
            "div3",
            "status_text",
            "footer"
        ]}
    }))?;

    // --- 标题 ---
    let title: Component = serde_json::from_value(json!({
        "id": "title",
        "component": "Text",
        "text": "🔐  用户登录"
    }))?;

    // --- 用户名行 ---
    let username_label: Component = serde_json::from_value(json!({
        "id": "username_label",
        "component": "Text",
        "text": "用户名"
    }))?;

    let username_field: Component = serde_json::from_value(json!({
        "id": "username_field",
        "component": "TextField",
        "value": "",
        "placeholder": "请输入用户名",
        "variant": "shortText"
    }))?;

    // --- 密码行 ---
    let password_label: Component = serde_json::from_value(json!({
        "id": "password_label",
        "component": "Text",
        "text": "密码"
    }))?;

    let password_field: Component = serde_json::from_value(json!({
        "id": "password_field",
        "component": "TextField",
        "value": "",
        "placeholder": "请输入密码",
        "variant": "obscured"
    }))?;

    // --- 记住密码 ---
    let remember_cb: Component = serde_json::from_value(json!({
        "id": "remember_cb",
        "component": "CheckBox",
        "checked": false,
        "label": "记住密码"
    }))?;

    // --- 登录按钮 ---
    let btn_label: Component = Component::text(
        ComponentId::new("btn_label").unwrap(),
        DynamicValue::Literal("登  录".to_string()),
    );
    let login_btn: Component = serde_json::from_value(json!({
        "id": "login_btn",
        "component": "Button",
        "child": "btn_label",
        "variant": "primary"
    }))?;

    // --- 状态文本（绑定到 /login_status 路径） ---
    let status_text: Component = serde_json::from_value(json!({
        "id": "status_text",
        "component": "Text",
        "text": { "path": "/login_status" }
    }))?;

    // --- 分割线 ---
    let div1: Component = serde_json::from_value(json!({
        "id": "div1", "component": "Divider"
    }))?;
    let div2: Component = serde_json::from_value(json!({
        "id": "div2", "component": "Divider"
    }))?;
    let div3: Component = serde_json::from_value(json!({
        "id": "div3", "component": "Divider"
    }))?;

    // --- 页脚 ---
    let footer: Component = serde_json::from_value(json!({
        "id": "footer",
        "component": "Text",
        "text": "A2UI Protocol v1.0 · 演示用途 · 不发送真实数据"
    }))?;

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

    // ============================================================
    // 后台线程：模拟服务端行为
    // ============================================================
    let msg_tx_clone = msg_tx.clone();
    std::thread::spawn(move || {
        // 等待 App 初始化完成
        std::thread::sleep(Duration::from_millis(200));

        // 1. 发送 createSurface（附带完整组件树）
        let envelope = ServerEnvelope::V1_0(V1_0ServerMessage::CreateSurface(CreateSurface {
            surface_id: "login".into(),
            catalog_id: "a2ui://catalogs/basic/v1".into(),
            surface_properties: Some(json!({"agentDisplayName": "A2UI Login Demo"})),
            send_data_model: false,
            components: Some(all_components),
            data_model: Some(json!({
                "login_status": "",
                "credentials": {
                    "username": "",
                    "password": ""
                }
            })),
        }));
        msg_tx_clone.send(envelope).ok();
    });

    // ============================================================
    // 后台线程：处理 action 消息，模拟登录逻辑
    // ============================================================
    let msg_tx_clone2 = msg_tx.clone();
    std::thread::spawn(move || {
        while let Some(envelope) = action_rx.blocking_recv() {
            // 提取 action 消息
            let action = match envelope {
                ClientEnvelope::V1_0 {
                    message: V1_0ClientMessage::Action(a),
                    ..
                } => a,
                _ => continue,
            };

            tracing::info!("收到 action: {:?}", action);

            if action.name == "click" {
                // 步骤 1：显示"正在登录..."
                msg_tx_clone2
                    .send(ServerEnvelope::V1_0(V1_0ServerMessage::UpdateDataModel(
                        UpdateDataModel {
                            surface_id: "login".into(),
                            path: Some("/login_status".into()),
                            value: Some(json!("⏳ 正在登录...")),
                        },
                    )))
                    .ok();

                // 步骤 2：模拟网络延迟后显示结果
                let tx = msg_tx_clone2.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(1500));

                    let success = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                        % 2
                        == 0;

                    let status = if success {
                        "✅ 登录成功！欢迎回来"
                    } else {
                        "❌ 登录失败：用户名或密码错误"
                    };

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

    // ============================================================
    // 启动 eframe 窗口
    // ============================================================
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([420.0, 520.0])
            .with_title("A2UI 登录演示"),
        ..Default::default()
    };

    eframe::run_native(
        "A2UI 登录演示",
        options,
        Box::new(|cc| {
            setup_chinese_fonts(cc);
            Box::new(app)
        }),
    )
    .map_err(|e| format!("eframe 错误: {}", e))?;

    Ok(())
}
