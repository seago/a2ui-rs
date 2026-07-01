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
        catalog_id: "a2ui://catalogs/basic/v1".into(),
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

#[test]
fn test_iced_widget_mapper_dynamic_form_controls() {
    use a2ui_renderer::DataBinding;
    use a2ui_renderer_iced::widget_mapper;

    let mut renderer = IcedRenderer::new();
    renderer.data_bindings.insert(
        "s1".to_string(),
        DataBinding::new(DataModel::new(json!({
            "form": {
                "username": "Alice",
                "placeholder": "请输入用户名",
                "remember": true,
                "volume": 42.0
            }
        }))),
    );

    let root: Component = serde_json::from_value(json!({
        "component": "Column",
        "id": "root",
        "children": ["username", "remember", "volume"]
    }))
    .unwrap();
    let username: Component = serde_json::from_value(json!({
        "component": "TextField",
        "id": "username",
        "value": {"path": "/form/username"},
        "placeholder": {"path": "/form/placeholder"},
        "variant": "shortText"
    }))
    .unwrap();
    let remember: Component = serde_json::from_value(json!({
        "component": "CheckBox",
        "id": "remember",
        "value": {"path": "/form/remember"},
        "label": "记住密码"
    }))
    .unwrap();
    let volume: Component = serde_json::from_value(json!({
        "component": "Slider",
        "id": "volume",
        "value": {"path": "/form/volume"},
        "min": 0,
        "max": 100
    }))
    .unwrap();

    let node = a2ui_renderer::component_forest::ComponentTreeNode::new(root).with_children(vec![
        a2ui_renderer::component_forest::ComponentTreeNode::new(username),
        a2ui_renderer::component_forest::ComponentTreeNode::new(remember),
        a2ui_renderer::component_forest::ComponentTreeNode::new(volume),
    ]);

    let _el = widget_mapper::build_element_tree(&node, &renderer, "s1");
    assert_eq!(renderer.profile_snapshot().element_builds, 4);
}
