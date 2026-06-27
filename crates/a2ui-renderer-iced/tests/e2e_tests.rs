use a2ui_core::message::server_to_client::CreateSurface;
use a2ui_core::prelude::*;
use a2ui_renderer::Renderer;
use a2ui_renderer_iced::IcedRenderer;
use serde_json::json;

#[test]
fn test_iced_full_surface_lifecycle() {
    let renderer = IcedRenderer::new();

    let (_msg_tx, msg_rx) = a2ui_renderer_iced::app::IcedApp::create_channel();
    let (action_tx, _action_rx) = a2ui_renderer_iced::app::IcedApp::create_action_channel();
    let app = a2ui_renderer_iced::app::IcedApp::new(renderer, msg_rx, action_tx);

    assert!(app.renderer.surfaces.is_empty());
    assert!(app.renderer.surface_order.is_empty());
}

#[test]
fn test_iced_create_surface_adds_to_order() {
    let mut renderer = IcedRenderer::new();

    let comp = Component::text(
        ComponentId::new("root").unwrap(),
        DynamicValue::Literal("Test".to_string()),
    );

    let msg = CreateSurface {
        surface_id: "s1".into(),
        catalog_id: "basic".into(),
        surface_properties: None,
        send_data_model: false,
        components: Some(vec![comp]),
        data_model: None,
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let _handle = renderer.create_surface(msg).await.unwrap();
        assert!(!renderer.surfaces.is_empty());
        assert_eq!(renderer.surface_order.len(), 1);
        assert_eq!(renderer.surface_order[0], "s1");
    });
}

#[test]
fn test_iced_widget_mapper_all_types() {
    use a2ui_renderer_iced::widget_mapper;
    let renderer = IcedRenderer::new();

    let types = [
        ("Text", json!({"component":"Text","id":"t","text":"hi"})),
        ("Divider", json!({"component":"Divider","id":"d"})),
        ("Icon", json!({"component":"Icon","id":"ic","name":"★"})),
        ("Video", json!({"component":"Video","id":"v","url":"x"})),
    ];

    for (_name, json_val) in &types {
        let comp: Component = serde_json::from_value(json_val.clone()).unwrap();
        let node = a2ui_renderer::component_forest::ComponentTreeNode::new(comp);
        let _el = widget_mapper::build_element_tree(&node, &renderer, "s1");
    }
}
