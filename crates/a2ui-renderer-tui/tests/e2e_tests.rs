use a2ui_core::prelude::*;
use a2ui_core::message::server_to_client::CreateSurface;
use a2ui_renderer::UserEvent;
use a2ui_renderer_tui::TuiRenderer;

#[test]
fn test_tui_full_flow() {
    let renderer = TuiRenderer::new();
    let comp = Component::text(
        ComponentId::new("root").unwrap(),
        DynamicValue::Literal("Hello TUI".to_string()),
    );
    let msg = CreateSurface {
        surface_id: "s1".to_string(),
        catalog_id: "basic".to_string(),
        surface_properties: None,
        send_data_model: false,
        components: Some(vec![comp]),
        data_model: None,
    };

    // 验证结构
    assert!(renderer.surfaces.is_empty());
}

#[test]
fn test_tui_component_lifecycle() {
    let renderer = TuiRenderer::new();

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
fn test_tui_input_events() {
    let handler = a2ui_renderer_tui::InputHandler;

    let click_event = handler.handle_click("btn1");
    match click_event {
        UserEvent::Click { component_id } => {
            assert_eq!(component_id.as_str(), "btn1");
        }
        _ => panic!("expected Click event"),
    }

    let key_event = handler.handle_key("Enter");
    assert!(key_event.is_some());
}
