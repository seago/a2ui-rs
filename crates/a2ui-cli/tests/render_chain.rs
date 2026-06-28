use a2ui_core::prelude::*;
use a2ui_renderer_tui::TuiRenderer;
use ratatui::backend::TestBackend;

#[tokio::test]
async fn test_process_server_envelope_calls_render_frame() {
    let mut renderer = TuiRenderer::new();
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();

    let comp = Component::text(
        ComponentId::new("root").unwrap(),
        DynamicValue::Literal("Hello".into()),
    );

    let msg = a2ui_core::message::server_to_client::CreateSurface {
        surface_id: "s1".into(),
        catalog_id: "a2ui://catalogs/basic/v1".into(),
        surface_properties: None,
        send_data_model: false,
        components: Some(vec![comp]),
        data_model: None,
    };

    let envelope =
        a2ui_core::ServerEnvelope::V1_0(a2ui_core::message::V1_0ServerMessage::CreateSurface(msg));

    a2ui_cli::process_server_envelope(&mut renderer, envelope, &mut terminal)
        .await
        .unwrap();

    let buf = terminal.backend().buffer();
    assert!(buf.area().width > 0);
}
