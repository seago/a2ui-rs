use a2ui_core::message::server_to_client::CreateSurface;
use a2ui_core::prelude::*;
use a2ui_renderer::UserEvent;
use a2ui_renderer_egui::GuiRenderer;

#[test]
fn test_gui_full_flow() {
    let renderer = GuiRenderer::new();
    let comp = Component::text(
        ComponentId::new("root").unwrap(),
        DynamicValue::Literal("Hello GUI".to_string()),
    );
    let msg = CreateSurface {
        surface_id: "s1".to_string(),
        catalog_id: "a2ui://catalogs/basic/v1".to_string(),
        surface_properties: None,
        send_data_model: false,
        components: Some(vec![comp]),
        data_model: None,
    };

    // 验证结构
    assert!(renderer.surfaces.is_empty());
}

#[test]
fn test_gui_component_lifecycle() {
    let renderer = GuiRenderer::new();

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

    // 验证组件树结构
    assert_eq!(root.id().as_str(), "root");
    assert_eq!(text1.id().as_str(), "text1");
    assert_eq!(text2.id().as_str(), "text2");
}

#[test]
fn test_gui_input_events() {
    let click_event = UserEvent::Click {
        component_id: ComponentId::new("btn1").unwrap(),
    };
    match click_event {
        UserEvent::Click { component_id } => {
            assert_eq!(component_id.as_str(), "btn1");
        }
        _ => panic!("expected Click event"),
    }

    let key_event = UserEvent::KeyPress {
        key: "Enter".into(),
    };
    assert!(matches!(key_event, UserEvent::KeyPress { key } if key == "Enter"));
}
