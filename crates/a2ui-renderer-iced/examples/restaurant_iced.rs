//! A2UI Iced 渲染器 — 餐厅列表演示
//!
//! 使用 List + template 从 Data Model 动态渲染餐厅卡片。
//!
//! 运行：`cargo run --example restaurant_iced -p a2ui-renderer-iced`

use a2ui_core::message::server_to_client::CreateSurface;
use a2ui_core::prelude::*;
use a2ui_core::ServerEnvelope;
use a2ui_renderer_iced::app::{self, IcedApp};
use a2ui_renderer_iced::{load_cjk_font, IcedRenderer};
use serde_json::json;

const HEADER_HEIGHT: u16 = 112;

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

    let msg_tx_clone = msg_tx.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(100));

        let title = Component::text(
            ComponentId::new("title_text").unwrap(),
            DynamicValue::Path {
                path: "/title".into(),
            },
        );
        let subtitle = Component::text(
            ComponentId::new("subtitle_text").unwrap(),
            DynamicValue::Path {
                path: "/subtitle".into(),
            },
        );

        let card_name = Component::text(
            ComponentId::new("card_name").unwrap(),
            DynamicValue::Path {
                path: "name".into(),
            },
        );
        let card_rating = Component::text(
            ComponentId::new("card_rating").unwrap(),
            DynamicValue::Path {
                path: "rating".into(),
            },
        );
        let card_detail = Component::text(
            ComponentId::new("card_detail").unwrap(),
            DynamicValue::Path {
                path: "detail".into(),
            },
        );
        let card_address = Component::text(
            ComponentId::new("card_address").unwrap(),
            DynamicValue::Path {
                path: "address".into(),
            },
        );
        let card_link = Component::text(
            ComponentId::new("card_link").unwrap(),
            DynamicValue::Path {
                path: "infoLink".into(),
            },
        );

        let card_image: Component = serde_json::from_value(json!({
            "component": "Image",
            "id": "card_image",
            "url": {"path": "imageUrl"},
            "width": "fill",
            "height": 140
        }))
        .unwrap();

        let card_body: Component = serde_json::from_value(json!({
            "component": "Column",
            "id": "card_body",
            "children": [
                "card_image",
                "card_name",
                "card_rating",
                "card_detail",
                "card_address",
                "card_link"
            ]
        }))
        .unwrap();

        let card_template: Component = serde_json::from_value(json!({
            "component": "Card",
            "id": "card_template",
            "child": "card_body"
        }))
        .unwrap();

        let list: Component = serde_json::from_value(json!({
            "component": "List",
            "id": "restaurant_list",
            "children": {
                "template": "card_template",
                "path": "/items"
            }
        }))
        .unwrap();

        let root: Component = serde_json::from_value(json!({
            "component": "Column",
            "id": "root",
            "children": ["restaurant_list"]
        }))
        .unwrap();

        let header: Component = serde_json::from_value(json!({
            "component": "Card",
            "id": "header_card",
            "child": "header_body"
        }))
        .unwrap();

        let header_divider: Component = serde_json::from_value(json!({
            "component": "Divider",
            "id": "header_divider"
        }))
        .unwrap();

        let header_body: Component = serde_json::from_value(json!({
            "component": "Column",
            "id": "header_body",
            "children": [
                "title_text",
                "subtitle_text",
                "header_divider"
            ]
        }))
        .unwrap();

        let envelope = ServerEnvelope::V1_0(a2ui_core::message::V1_0ServerMessage::CreateSurface(
            CreateSurface {
                surface_id: "restaurants".into(),
                catalog_id: "basic".into(),
                surface_properties: Some(json!({"agentDisplayName": "A2UI Restaurant List"})),
                send_data_model: true,
                components: Some(vec![
                    root,
                    header,
                    title,
                    subtitle,
                    header_divider,
                    header_body,
                    list,
                    card_template,
                    card_body,
                    card_image,
                    card_name,
                    card_rating,
                    card_detail,
                    card_address,
                    card_link,
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

    std::thread::spawn(move || {
        while let Some(action) = action_rx.blocking_recv() {
            tracing::info!("收到 action: {:?}", action);
        }
    });

    let app_builder = iced::application("A2UI 餐厅列表演示", app::update, view)
        .subscription(app::subscription)
        .window_size(iced::Size::new(520.0, 720.0));

    let app_builder = if let Some(font) = cjk_font {
        app_builder
            .font(font.bytes)
            .default_font(iced::Font::with_name(font.family))
    } else {
        app_builder
    };

    app_builder.run_with(move || (app, iced::Task::none()))
}

fn view(app: &IcedApp) -> iced::Element<'_, app::Message> {
    let base = iced::widget::container(app::view(app))
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .padding(iced::Padding::default().top(HEADER_HEIGHT))
        .into();

    let header = iced::widget::container(
        iced::widget::column![
            iced::widget::text("纽约川菜餐厅列表")
                .size(28)
                .shaping(iced::widget::text::Shaping::Advanced),
            iced::widget::text("按评分、菜品风格和位置快速浏览")
                .size(18)
                .shaping(iced::widget::text::Shaping::Advanced),
            iced::widget::horizontal_rule(1),
        ]
        .spacing(10)
        .width(iced::Length::Fill),
    )
    .width(iced::Length::Fill)
    .padding([16, 20])
    .style(|_| iced::widget::container::background(iced::Color::WHITE))
    .into();

    iced::widget::stack([base, header])
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .into()
}
