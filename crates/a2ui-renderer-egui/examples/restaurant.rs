//! A2UI GUI 渲染器 — 餐厅列表演示
//!
//! 使用 List + template 从 Data Model 动态渲染餐厅卡片。
//! 数据在 CreateSurface 时一并发送，模板在 Surface 创建阶段展开。

use a2ui_core::message::server_to_client::CreateSurface;
use a2ui_core::prelude::*;
use a2ui_core::ServerEnvelope;
use a2ui_renderer_egui::{A2uiApp, GuiRenderer};
use serde_json::json;

const HEADER_HEIGHT: f32 = 124.0;

/// 加载系统中文字体，使 egui 能正确渲染中文
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
    // Basic Catalog 已由 CatalogRegistry::with_defaults() 自动加载

    let (msg_tx, msg_rx) = A2uiApp::create_channel();
    let (action_tx, mut action_rx) = A2uiApp::create_action_channel();

    let app = A2uiApp::new(renderer, msg_rx, action_tx);

    let restaurants = json!([
        {
            "name": "Xi'an Famous Foods",
            "rating": "★★★★☆ 4.4",
            "detail": "招牌手扯面，辣味扎实，适合想吃重口味面食的时候。",
            "infoLink": "https://www.xianfoods.com/",
            "imageUrl": "https://a2ui-composer.ag-ui.com/images/restaurant_finder/shrimpchowmein.jpeg",
            "address": "81 St Marks Pl, New York, NY 10003"
        },
        {
            "name": "Han Dynasty",
            "rating": "★★★★☆ 4.3",
            "detail": "川味菜选择多，麻婆豆腐和干锅类是稳定选择。",
            "infoLink": "https://www.handynasty.net/",
            "imageUrl": "https://a2ui-composer.ag-ui.com/images/restaurant_finder/mapotofu.jpeg",
            "address": "90 3rd Ave, New York, NY 10003"
        },
        {
            "name": "RedFarm",
            "rating": "★★★★☆ 4.2",
            "detail": "现代中餐风格，更适合聚餐和分享小盘菜。",
            "infoLink": "https://www.redfarmnyc.com/",
            "imageUrl": "https://a2ui-composer.ag-ui.com/images/restaurant_finder/beefbroccoli.jpeg",
            "address": "529 Hudson St, New York, NY 10014"
        },
        {
            "name": "Mott 32",
            "rating": "★★★★★ 4.6",
            "detail": "更精致的粤菜与中餐体验，环境正式，适合约会或商务餐。",
            "infoLink": "https://mott32.com/newyork/",
            "imageUrl": "https://a2ui-composer.ag-ui.com/images/restaurant_finder/springrolls.jpeg",
            "address": "111 W 57th St, New York, NY 10019"
        },
        {
            "name": "Hwa Yuan Szechuan",
            "rating": "★★★★☆ 4.4",
            "detail": "老牌川菜馆，冷面和经典川菜都比较有代表性。",
            "infoLink": "https://hwayuannyc.com/",
            "imageUrl": "https://a2ui-composer.ag-ui.com/images/restaurant_finder/kungpao.jpeg",
            "address": "40 E Broadway, New York, NY 10002"
        }
    ]);

    // 后台线程：发送 A2UI 消息
    let msg_tx_clone = msg_tx.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(100));

        let card_name: Component = serde_json::from_value(json!({
            "component": "Text",
            "id": "card_name",
            "text": {"path": "name"},
            "style": {"fontSize": 22, "strong": true}
        }))
        .unwrap();
        let card_rating: Component = serde_json::from_value(json!({
            "component": "Text",
            "id": "card_rating",
            "text": {"path": "rating"},
            "style": {"fontSize": 15, "color": "#b8860b"}
        }))
        .unwrap();
        let card_detail: Component = serde_json::from_value(json!({
            "component": "Text",
            "id": "card_detail",
            "text": {"path": "detail"},
            "style": {"fontSize": 16}
        }))
        .unwrap();
        let card_address_icon: Component = serde_json::from_value(json!({
            "component": "Icon",
            "id": "card_address_icon",
            "name": "📍",
            "style": {"fontSize": 16, "color": "#828282"}
        }))
        .unwrap();
        let card_address: Component = serde_json::from_value(json!({
            "component": "Text",
            "id": "card_address",
            "text": {"path": "address"},
            "style": {"fontSize": 14, "color": "#6e6e6e"}
        }))
        .unwrap();
        let card_address_row: Component = serde_json::from_value(json!({
            "component": "Row",
            "id": "card_address_row",
            "children": ["card_address_icon", "card_address"],
            "style": {"spacing": {"x": 6, "y": 0}}
        }))
        .unwrap();
        let card_link_icon: Component = serde_json::from_value(json!({
            "component": "Icon",
            "id": "card_link_icon",
            "name": "↗",
            "style": {"fontSize": 16, "color": "#1976d2"}
        }))
        .unwrap();
        let card_link: Component = serde_json::from_value(json!({
            "component": "Text",
            "id": "card_link",
            "text": {"path": "infoLink"},
            "style": {"fontSize": 14, "color": "#1976d2"}
        }))
        .unwrap();
        let card_link_row: Component = serde_json::from_value(json!({
            "component": "Row",
            "id": "card_link_row",
            "children": ["card_link_icon", "card_link"],
            "style": {"spacing": {"x": 6, "y": 0}}
        }))
        .unwrap();

        let card_image: Component = serde_json::from_value(json!({
            "component": "Image",
            "id": "card_image",
            "url": {"path": "imageUrl"},
            "width": "fill",
            "height": 140,
            "style": {"radius": 10}
        }))
        .unwrap();

        let card_body = Component::column(
            ComponentId::new("card_body").unwrap(),
            vec![
                ComponentId::new("card_image").unwrap(),
                ComponentId::new("card_name").unwrap(),
                ComponentId::new("card_rating").unwrap(),
                ComponentId::new("card_detail").unwrap(),
                ComponentId::new("card_address_row").unwrap(),
                ComponentId::new("card_link_row").unwrap(),
            ],
        );

        let card_template: Component = serde_json::from_value(json!({
            "component": "Card",
            "id": "card_template",
            "child": "card_body",
            "style": {
                "fill": "#fafafa",
                "padding": 12,
                "spacing": {"x": 0, "y": 10}
            }
        }))
        .unwrap();

        let root = Component::list(
            ComponentId::new("root").unwrap(),
            a2ui_core::component::ChildList::object(
                ComponentId::new("card_template").unwrap(),
                "/items",
            ),
        );

        // CreateSurface 一步到位：组件 + 数据一起发送
        let envelope = ServerEnvelope::V1_0(a2ui_core::message::V1_0ServerMessage::CreateSurface(
            CreateSurface {
                surface_id: "restaurants".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: Some(json!({"agentDisplayName": "Szechuan Restaurant Finder"})),
                send_data_model: true,
                components: Some(vec![
                    root,
                    card_template,
                    card_body,
                    card_image,
                    card_name,
                    card_rating,
                    card_detail,
                    card_address_icon,
                    card_address,
                    card_address_row,
                    card_link_icon,
                    card_link,
                    card_link_row,
                ]),
                data_model: Some(json!({
                    "title": "纽约川菜餐厅列表",
                    "subtitle": "按评分、菜品风格和位置快速浏览",
                    "items": restaurants
                })),
            },
        ));
        msg_tx_clone.send(envelope).ok();
    });

    // 监听 action 消息
    std::thread::spawn(move || {
        while let Some(action) = action_rx.blocking_recv() {
            tracing::info!("收到 action: {:?}", action);
        }
    });

    // 启动桌面窗口
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([520.0, 720.0])
            .with_title("A2UI 餐厅列表演示"),
        ..Default::default()
    };

    eframe::run_native(
        "A2UI 餐厅列表演示",
        options,
        Box::new(|cc| {
            setup_chinese_fonts(cc);
            Box::new(RestaurantApp { inner: app })
        }),
    )
    .map_err(|e| format!("eframe 错误: {}", e))?;

    Ok(())
}

struct RestaurantApp {
    inner: A2uiApp,
}

impl eframe::App for RestaurantApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("restaurant_header")
            .exact_height(HEADER_HEIGHT)
            .show_separator_line(false)
            .frame(egui::Frame::default().fill(egui::Color32::WHITE))
            .show(ctx, |ui| {
                ui.add_space(18.0);
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new("纽约川菜餐厅列表").size(28.0).strong());
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("按评分、菜品风格和位置快速浏览").size(18.0));
                });
                ui.add_space(14.0);
            });

        self.inner.update(ctx, frame);
    }
}
