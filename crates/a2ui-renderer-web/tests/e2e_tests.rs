use a2ui_core::message::server_to_client::CreateSurface;
use a2ui_core::prelude::*;
use a2ui_core::ComponentId;
use a2ui_renderer::Renderer;
use a2ui_renderer_web::html_builder::RenderableHtmlWidget;
use a2ui_renderer_web::HtmlBuilder;
use a2ui_renderer_web::WebRenderer;

#[test]
fn test_web_renderer_new() {
    let renderer = WebRenderer::new();
    assert!(renderer.surfaces.is_empty());
}

#[test]
fn test_web_render_text_to_html() {
    let html = HtmlBuilder.render(&RenderableHtmlWidget::Text {
        id: ComponentId::new("t1").unwrap(),
        text: "Hello Web".to_string(),
        variant: "body".to_string(),
    });
    assert!(html.contains("Hello Web"));
    assert!(html.contains("<p"));
}

#[test]
fn test_web_render_button_to_html() {
    let html = HtmlBuilder.render(&RenderableHtmlWidget::Button {
        id: ComponentId::new("btn1").unwrap(),
        label: "Click".to_string(),
        variant: "primary".to_string(),
    });
    assert!(html.contains("Click"));
    assert!(html.contains("<button"));
    assert!(html.contains("primary"));
}

#[test]
fn test_html_escape() {
    let html = HtmlBuilder.render(&RenderableHtmlWidget::Text {
        id: ComponentId::new("t1").unwrap(),
        text: "<script>alert('xss')</script>".to_string(),
        variant: "body".to_string(),
    });
    assert!(!html.contains("<script>"));
    assert!(html.contains("&lt;script&gt;"));
}

#[test]
fn test_render_page_contains_doctype() {
    let html = HtmlBuilder.render_page("<p>Content</p>", "Test");
    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("<title>Test</title>"));
}

#[tokio::test]
async fn test_web_renderer_create_and_render() {
    let mut renderer = WebRenderer::new();
    let comp = Component::text(
        ComponentId::new("root").unwrap(),
        DynamicValue::Literal("E2E Test".into()),
    );
    renderer
        .create_surface(CreateSurface {
            surface_id: "e2e".into(),
            catalog_id: "a2ui://catalogs/basic/v1".into(),
            surface_properties: None,
            send_data_model: false,
            components: Some(vec![comp]),
            data_model: None,
        })
        .await
        .unwrap();

    let html = renderer.render_surface_html("e2e");
    assert!(html.is_some());
    assert!(html.unwrap().contains("E2E Test"));
}

#[tokio::test]
async fn test_web_renderer_incremental_update() {
    let mut renderer = WebRenderer::new();
    let comp = Component::text(
        ComponentId::new("root").unwrap(),
        DynamicValue::Path {
            path: "/value".into(),
        },
    );
    renderer
        .create_surface(CreateSurface {
            surface_id: "incr".into(),
            catalog_id: "a2ui://catalogs/basic/v1".into(),
            surface_properties: None,
            send_data_model: false,
            components: Some(vec![comp]),
            data_model: Some(serde_json::json!({"value": "initial"})),
        })
        .await
        .unwrap();

    // Verify initial render
    let html = renderer.render_surface_html("incr").unwrap();
    assert!(html.contains("initial"));

    // Update data model
    renderer
        .update_data_model(a2ui_core::message::server_to_client::UpdateDataModel {
            surface_id: "incr".into(),
            path: Some("/value".into()),
            value: Some(serde_json::json!("updated")),
        })
        .await
        .unwrap();

    // After re-render, should show updated value
    let html = renderer.render_surface_html("incr").unwrap();
    assert!(html.contains("updated"));
}

#[tokio::test]
async fn test_web_renderer_full_end_to_end() {
    let mut renderer = WebRenderer::new();

    // Create a surface with a proper component tree
    let root = Component::column(
        ComponentId::new("root").unwrap(),
        vec![ComponentId::new("greeting").unwrap(), ComponentId::new("btn").unwrap()],
    );
    let text = Component::text(
        ComponentId::new("greeting").unwrap(),
        DynamicValue::Literal("Welcome!".into()),
    );
    let button: Component = serde_json::from_str(
        r#"{"id":"btn","component":"Button","child":"lbl","text":"Submit","variant":"primary"}"#,
    )
    .unwrap();
    let label = Component::text(
        ComponentId::new("lbl").unwrap(),
        DynamicValue::Literal("Submit".into()),
    );

    renderer
        .create_surface(CreateSurface {
            surface_id: "app".into(),
            catalog_id: "a2ui://catalogs/basic/v1".into(),
            surface_properties: None,
            send_data_model: false,
            components: Some(vec![root, text, button, label]),
            data_model: None,
        })
        .await
        .unwrap();

    // Render as complete HTML page
    let full_html = renderer.render_all_html();
    assert!(full_html.contains("Welcome!"));
    assert!(full_html.contains("Submit"));

    // Delete surface
    renderer
        .delete_surface(a2ui_core::message::server_to_client::DeleteSurface {
            surface_id: "app".into(),
        })
        .await
        .unwrap();
    assert!(renderer.surfaces.values().all(|s| s != "app"));
}
