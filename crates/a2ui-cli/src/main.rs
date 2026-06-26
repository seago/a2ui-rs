use a2ui_core::prelude::*;
use a2ui_renderer::Renderer;
use a2ui_renderer_tui::TuiRenderer;
use a2ui_transport::jsonl::JsonlTransport;
use a2ui_transport::Transport;
use clap::{Parser, Subcommand};
use tracing::{error, info, warn};

/// A2UI CLI — 渲染 A2UI 协议的 UI 表面
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// 从 STDIN 读取 JSONL 流并渲染到终端
    Render {
        /// 输入文件（省略则从 STDIN 读取）
        #[arg(short, long)]
        input: Option<std::path::PathBuf>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Render { input } => {
            run_render(input).await?;
        }
    }

    Ok(())
}

/// 统一的 transport 包装，支持文件输入和 STDIN 两种模式
enum InputTransport {
    File(JsonlTransport<tokio::fs::File, tokio::io::Stdout>),
    Stdin(JsonlTransport<tokio::io::Stdin, tokio::io::Stdout>),
}

impl InputTransport {
    async fn connect(&mut self) -> anyhow::Result<()> {
        match self {
            InputTransport::File(t) => Transport::connect(t).await?,
            InputTransport::Stdin(t) => Transport::connect(t).await?,
        }
        Ok(())
    }

    /// 接收服务端信封消息（Agent → Renderer）
    async fn receive(&mut self) -> anyhow::Result<Option<ServerEnvelope>> {
        match self {
            InputTransport::File(t) => match Transport::receive(t).await {
                Ok(envelope) => Ok(Some(envelope)),
                Err(e) => {
                    // EOF 或读取错误
                    if e.to_string().contains("EOF") || e.to_string().contains("read error") {
                        Ok(None)
                    } else {
                        Err(e.into())
                    }
                }
            },
            InputTransport::Stdin(t) => match Transport::receive(t).await {
                Ok(envelope) => Ok(Some(envelope)),
                Err(e) => {
                    if e.to_string().contains("EOF") || e.to_string().contains("read error") {
                        Ok(None)
                    } else {
                        Err(e.into())
                    }
                }
            },
        }
    }

    #[allow(dead_code)]
    async fn send_envelope(&mut self, envelope: ClientEnvelope) -> anyhow::Result<()> {
        match self {
            InputTransport::File(t) => Transport::send(t, envelope).await?,
            InputTransport::Stdin(t) => Transport::send(t, envelope).await?,
        }
        Ok(())
    }

    async fn close(&mut self) -> anyhow::Result<()> {
        match self {
            InputTransport::File(t) => Transport::close(t).await?,
            InputTransport::Stdin(t) => Transport::close(t).await?,
        }
        Ok(())
    }
}

async fn run_render(input: Option<std::path::PathBuf>) -> anyhow::Result<()> {
    info!("Starting A2UI renderer");

    let mut transport = if let Some(path) = input {
        info!("Input source: file {}", path.display());
        let file = std::fs::File::open(path)?;
        let reader = tokio::fs::File::from_std(file);
        let writer = tokio::io::stdout();
        InputTransport::File(JsonlTransport::new(reader, writer))
    } else {
        info!("Input source: STDIN");
        InputTransport::Stdin(JsonlTransport::from_std())
    };

    let mut renderer = TuiRenderer::new();
    transport.connect().await?;
    info!("Transport connected");

    // 仅 STDIN 模式打印欢迎信息
    if matches!(transport, InputTransport::Stdin(_)) {
        println!("A2UI TUI Renderer ready. Send JSONL messages on stdin.");
        println!("Waiting for createSurface...");
    }

    // 消息处理主循环
    loop {
        match transport.receive().await {
            Ok(Some(envelope)) => {
                if let Err(e) = process_server_envelope(&mut renderer, envelope).await {
                    error!("Error processing message: {}", e);
                }
            }
            Ok(None) => {
                info!("Input stream closed");
                break;
            }
            Err(e) => {
                warn!("Transport receive error: {}", e);
                break;
            }
        }
    }

    transport.close().await?;
    info!("Transport closed");
    Ok(())
}

/// 处理服务端信封消息，调用渲染器对应方法
async fn process_server_envelope(
    renderer: &mut TuiRenderer,
    envelope: ServerEnvelope,
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
        },
    }

    // 渲染更新
    renderer.render().await?;

    Ok(())
}
