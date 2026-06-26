use a2ui_core::prelude::*;
use a2ui_renderer::Renderer;
use a2ui_renderer_tui::TuiRenderer;
use ratatui::backend::Backend;
use tracing::info;

/// 处理服务端信封消息，调用渲染器对应方法
pub async fn process_server_envelope(
    renderer: &mut TuiRenderer,
    envelope: ServerEnvelope,
    terminal: &mut ratatui::Terminal<impl Backend>,
) -> anyhow::Result<()> {
    use a2ui_core::message::V1_0ServerMessage;

    match envelope {
        ServerEnvelope::V1_0(message) => match message {
            V1_0ServerMessage::CreateSurface(msg) => {
                info!("CreateSurface: id={}", msg.surface_id);
                let _handle = renderer.create_surface(msg).await?;
                info!("Surface created");
            }
            V1_0ServerMessage::UpdateComponents(msg) => {
                info!("UpdateComponents: surface={}", msg.surface_id);
                renderer.update_components(msg).await?;
            }
            V1_0ServerMessage::UpdateDataModel(msg) => {
                info!("UpdateDataModel: surface={}", msg.surface_id);
                renderer.update_data_model(msg).await?;
            }
            V1_0ServerMessage::DeleteSurface(msg) => {
                info!("DeleteSurface: id={}", msg.surface_id);
                renderer.delete_surface(msg).await?;
            }
            V1_0ServerMessage::ActionResponse(msg) => {
                info!("ActionResponse: id={}", msg.action_id);
                renderer.action_response(msg).await?;
            }
            V1_0ServerMessage::CallFunction(msg) => {
                info!("CallFunction: id={}", msg.function_call_id);
                let response = renderer.call_function(msg).await?;
                info!("Function response: call={}", response.call);
            }
            V1_0ServerMessage::Capabilities(msg) => {
                info!(
                    "Capabilities: version={}, features={:?}",
                    msg.version, msg.features
                );
            }
        },
    }

    // 渲染更新
    renderer.render_frame(terminal).await?;

    Ok(())
}
