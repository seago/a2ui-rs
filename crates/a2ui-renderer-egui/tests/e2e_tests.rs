use a2ui_core::message::server_to_client::{CreateSurface, DeleteSurface, UpdateComponents};
use a2ui_core::prelude::*;
use a2ui_renderer::{Renderer, UserEvent};
use a2ui_renderer_egui::GuiRenderer;
use serde_json::json;

#[tokio::test]
async fn test_gui_full_flow() {
    let mut renderer = GuiRenderer::new();
    let comp: Component = serde_json::from_value(json!({
        "id": "root",
        "component": "Text",
        "text": {"path": "/greeting"}
    }))
    .unwrap();

    renderer
        .create_surface(CreateSurface {
            surface_id: "s1".to_string(),
            catalog_id: "a2ui://catalogs/basic/v1".to_string(),
            surface_properties: None,
            send_data_model: false,
            components: Some(vec![comp]),
            data_model: Some(json!({"greeting": "Hello GUI"})),
        })
        .await
        .expect("createSurface 应成功");

    // surface 已登记
    assert!(!renderer.core.surfaces().is_empty(), "创建后 surfaces 非空");
    assert!(
        renderer.core.surfaces().values().any(|id| id == "s1"),
        "surfaces 应包含 s1"
    );
    // 数据绑定已建立且能解析初始数据
    let binding = renderer.core.binding("s1").expect("s1 的 binding 应存在");
    assert_eq!(binding.get("/greeting"), Some(&json!("Hello GUI")));
    // 组件已进入 forest，root 可检出
    assert_eq!(
        renderer
            .core
            .forest()
            .get_root("s1")
            .expect("root 组件应存在")
            .id()
            .as_str(),
        "root"
    );
}

#[tokio::test]
async fn test_gui_component_lifecycle() {
    let mut renderer = GuiRenderer::new();

    let root = Component::column(
        ComponentId::new("root").unwrap(),
        vec![
            ComponentId::new("text1").unwrap(),
            ComponentId::new("text2").unwrap(),
        ],
    );
    let text1 = Component::text(
        ComponentId::new("text1").unwrap(),
        DynamicValue::Literal("Line 1".to_string()),
    );
    let text2 = Component::text(
        ComponentId::new("text2").unwrap(),
        DynamicValue::Literal("Line 2".to_string()),
    );

    // create：三个组件入 forest
    renderer
        .create_surface(CreateSurface {
            surface_id: "s1".to_string(),
            catalog_id: "a2ui://catalogs/basic/v1".to_string(),
            surface_properties: None,
            send_data_model: false,
            components: Some(vec![root, text1, text2]),
            data_model: None,
        })
        .await
        .expect("createSurface 应成功");
    assert_eq!(renderer.core.forest().component_count("s1"), 3);
    let tree = renderer.core.forest().build_tree("s1").unwrap();
    assert_eq!(tree.component.id().as_str(), "root");
    assert_eq!(tree.children.len(), 2);

    // update：改写 text1 的文案
    let text1_v2 = Component::text(
        ComponentId::new("text1").unwrap(),
        DynamicValue::Literal("Line 1 updated".to_string()),
    );
    renderer
        .update_components(UpdateComponents {
            surface_id: "s1".to_string(),
            components: vec![text1_v2],
        })
        .await
        .expect("updateComponents 应成功");
    let updated = renderer
        .core
        .forest()
        .get("s1", &ComponentId::new("text1").unwrap())
        .expect("text1 应仍在 forest");
    assert_eq!(
        updated.properties().get("text"),
        Some(&json!("Line 1 updated")),
        "updateComponents 应就地更新组件属性"
    );

    // delete：surface 与组件全部移除
    renderer
        .delete_surface(DeleteSurface {
            surface_id: "s1".to_string(),
        })
        .await
        .expect("deleteSurface 应成功");
    assert!(
        renderer.core.surfaces().is_empty(),
        "删除后 surfaces 应为空"
    );
    assert_eq!(renderer.core.forest().component_count("s1"), 0);
}

#[tokio::test]
async fn test_gui_input_events() {
    // Click 无声明 action 的组件：不产生消息，但事件被正常消化
    let mut renderer = GuiRenderer::new();
    let comp = Component::text(
        ComponentId::new("btn1").unwrap(),
        DynamicValue::Literal("hi".to_string()),
    );
    renderer
        .create_surface(CreateSurface {
            surface_id: "s1".to_string(),
            catalog_id: "a2ui://catalogs/basic/v1".to_string(),
            surface_properties: None,
            send_data_model: false,
            components: Some(vec![comp]),
            data_model: None,
        })
        .await
        .unwrap();

    let click_event = UserEvent::Click {
        component_id: ComponentId::new("btn1").unwrap(),
    };
    let envelope = renderer.handle_user_event(click_event).await.unwrap();
    assert!(envelope.is_none(), "无声明 action 的 Click 不应发消息");

    let key_event = UserEvent::KeyPress {
        key: "Enter".into(),
    };
    let envelope = renderer.handle_user_event(key_event).await.unwrap();
    assert!(envelope.is_none(), "无焦点时 Enter 不应发消息");
}
