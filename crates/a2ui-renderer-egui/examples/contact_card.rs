//! A2UI egui 渲染器 — 联系人卡片演示
//!
//! 展示单个联系人信息：头像、姓名、职位、团队、地点、联系方式等。
//! 运行：`cargo run --example contact_card -p a2ui-renderer-egui`

use a2ui_core::message::server_to_client::{CreateSurface, UpdateDataModel};
use a2ui_core::prelude::json;
use a2ui_core::prelude::*;
use a2ui_core::ServerEnvelope;
use a2ui_renderer_egui::{A2uiApp, GuiRenderer};

fn setup_chinese_fonts(cc: &eframe::CreationContext) {
    let font_paths = [
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
        "/System/Library/Fonts/Supplemental/Songti.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\simsun.ttc",
    ];
    for path in &font_paths {
        if let Ok(data) = std::fs::read(path) {
            let mut fonts = egui::FontDefinitions::default();
            fonts
                .font_data
                .insert("ChineseFont".to_owned(), egui::FontData::from_owned(data));
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "ChineseFont".to_owned());
            cc.egui_ctx.set_fonts(fonts);
            return;
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let rt = tokio::runtime::Runtime::new()?;
    let _guard = rt.enter();

    let mut renderer = GuiRenderer::new();
    renderer.register_function("formatString", a2ui_renderer::CallableFrom::ClientOrRemote);

    let catalog = a2ui_core::Catalog::new("basic").with_instructions("Basic catalog");
    renderer.register_catalog(catalog).ok();

    let (msg_tx, msg_rx) = A2uiApp::create_channel();
    let (action_tx, mut action_rx) = A2uiApp::create_action_channel();
    let app = A2uiApp::new(renderer, msg_rx, action_tx);

    // ── 构建联系人卡片组件 ──

    // 头像 Image（先放占位，updateDataModel 时填充真实 URL）
    let avatar: Component = Component::from_value(json!({
        "component": "Image",
        "id": "avatar",
        "url": { "path": "/imageUrl" }
    }))
    .unwrap();

    // 姓名
    let name_text: Component = Component::from_value(json!({
        "component": "Text",
        "id": "name",
        "text": { "path": "/name" }
    }))
    .unwrap();

    // 职位
    let title_text: Component = Component::from_value(json!({
        "component": "Text",
        "id": "title",
        "text": { "path": "/title" }
    }))
    .unwrap();

    // 团队
    let team_text: Component = Component::from_value(json!({
        "component": "Text",
        "id": "team",
        "text": { "path": "/team" }
    }))
    .unwrap();

    // 地点
    let location_text: Component = Component::from_value(json!({
        "component": "Text",
        "id": "location",
        "text": { "path": "/location" }
    }))
    .unwrap();

    // 分隔线
    let div1: Component =
        Component::from_value(json!({"component": "Divider", "id": "div1"})).unwrap();

    // 联系方式标签
    let contact_label: Component = Component::from_value(json!({
        "component": "Text",
        "id": "contact_label",
        "text": "📧 联系方式"
    }))
    .unwrap();

    // 邮箱
    let email_text: Component = Component::from_value(json!({
        "component": "Text",
        "id": "email",
        "text": { "path": "/email" }
    }))
    .unwrap();

    // 手机
    let mobile_text: Component = Component::from_value(json!({
        "component": "Text",
        "id": "mobile",
        "text": { "path": "/mobile" }
    }))
    .unwrap();

    let div2: Component =
        Component::from_value(json!({"component": "Divider", "id": "div2"})).unwrap();

    // 日历状态
    let calendar_text: Component = Component::from_value(json!({
        "component": "Text",
        "id": "calendar",
        "text": { "path": "/calendar" }
    }))
    .unwrap();

    // 根容器：Card 包裹的 Column
    let inner_column: Component = Component::from_value(json!({
        "component": "Column",
        "id": "inner_col",
        "children": [
            "avatar", "name", "title", "team", "location",
            "div1", "contact_label", "email", "mobile",
            "div2", "calendar"
        ]
    }))
    .unwrap();

    let root: Component = Component::from_value(json!({
        "component": "Card",
        "id": "root",
        "child": "inner_col"
    }))
    .unwrap();

    let all_components = vec![
        root,
        inner_column,
        avatar,
        name_text,
        title_text,
        team_text,
        location_text,
        div1,
        contact_label,
        email_text,
        mobile_text,
        div2,
        calendar_text,
    ];

    // ── 后台线程：发送 CreateSurface + updateDataModel ──
    let msg_tx_clone = msg_tx.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(100));

        // 步骤 1：创建 Surface（带占位 data model）
        let envelope = ServerEnvelope::V1_0(a2ui_core::message::V1_0ServerMessage::CreateSurface(
            CreateSurface {
                surface_id: "contact-card".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: Some(json!({"agentDisplayName": "Contact Card"})),
                send_data_model: false,
                components: Some(all_components),
                data_model: Some(json!({
                    "name": "Loading...",
                    "title": "",
                    "team": "",
                    "location": "",
                    "imageUrl": "",
                    "email": "",
                    "mobile": "",
                    "calendar": ""
                })),
            },
        ));
        msg_tx_clone.send(envelope).ok();

        // 步骤 2：发送 updateDataModel 填充真实数据
        std::thread::sleep(std::time::Duration::from_millis(200));
        let base = "https://a2ui-composer.ag-ui.com";
        let envelope = ServerEnvelope::V1_0(
            a2ui_core::message::V1_0ServerMessage::UpdateDataModel(UpdateDataModel {
                surface_id: "contact-card".into(),
                path: Some("/".into()),
                value: Some(json!({
                    "name": "Alex Jordan",
                    "title": "Product Marketing Manager",
                    "team": "Team Macally",
                    "location": "New York",
                    "email": "alex.jordan@example.com",
                    "mobile": "+1 (415) 171-1080",
                    "calendar": "Free until 4:00 PM",
                    "imageUrl": format!("{}/images/contact_lookup/profile1.png", base)
                })),
            }),
        );
        msg_tx_clone.send(envelope).ok();
    });

    // ── 监听 action ──
    std::thread::spawn(move || {
        while let Some(action) = action_rx.blocking_recv() {
            tracing::info!("收到 action: {:?}", action);
        }
    });

    // ── 启动窗口 ──
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 560.0])
            .with_title("Contact Card — A2UI egui"),
        ..Default::default()
    };

    eframe::run_native(
        "Contact Card",
        options,
        Box::new(|cc| {
            setup_chinese_fonts(cc);
            Box::new(app)
        }),
    )
    .map_err(|e| format!("eframe 错误: {}", e))?;

    Ok(())
}
