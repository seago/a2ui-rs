use a2ui_core::message::Capabilities;
use a2ui_core::prelude::*;
use a2ui_renderer_tui::TuiRenderer;
use a2ui_transport::jsonl::JsonlTransport;
use a2ui_transport::{Transport, TransportError};
use clap::{Parser, Subcommand};
use tracing::{error, info, warn};

use a2ui_cli::process_server_envelope;

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
    // stdout 是 JSONL 协议输出通道（JsonlTransport 的 writer），
    // 日志必须写 stderr，否则会污染协议流导致对端反序列化失败
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

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

    async fn handshake(&mut self) -> anyhow::Result<Capabilities> {
        let client_caps = Capabilities {
            version: "1.0".to_string(),
            features: vec!["tui".to_string()],
        };
        match self {
            InputTransport::File(t) => Transport::handshake(t, client_caps)
                .await
                .map_err(Into::into),
            InputTransport::Stdin(t) => Transport::handshake(t, client_caps)
                .await
                .map_err(Into::into),
        }
    }

    /// 接收服务端信封消息（Agent → Renderer）。
    /// 流正常结束（EOF）返回 `Ok(None)`；真实 IO/协议错误向上传播。
    async fn receive(&mut self) -> anyhow::Result<Option<ServerEnvelope>> {
        let result = match self {
            InputTransport::File(t) => Transport::receive(t).await,
            InputTransport::Stdin(t) => Transport::receive(t).await,
        };
        match result {
            Ok(envelope) => Ok(Some(envelope)),
            Err(TransportError::Eof) => Ok(None),
            Err(e) => Err(e.into()),
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
        // 检测 stdin 是否为交互式终端（TTY）
        if atty::is(atty::Stream::Stdin) {
            eprintln!("Error: a2ui render requires piped JSONL input, not interactive terminal.");
            eprintln!("Usage:");
            eprintln!("  echo '{{\"version\":\"v1.0\",...}}' | a2ui render");
            eprintln!("  a2ui render --input messages.jsonl");
            std::process::exit(1);
        }
        info!("Input source: STDIN (piped)");
        InputTransport::Stdin(JsonlTransport::from_std())
    };

    let mut renderer = TuiRenderer::new();

    // 创建 ratatui Terminal（使用 stderr 作为后端输出）
    let backend = ratatui::backend::CrosstermBackend::new(std::io::stderr());
    let mut terminal = ratatui::Terminal::new(backend)?;

    transport.connect().await?;
    info!("Transport connected");

    // 执行能力协商握手
    let server_caps = transport.handshake().await?;
    info!(
        "Server capabilities: version={}, features={:?}",
        server_caps.version, server_caps.features
    );

    // 仅 STDIN 模式打印欢迎信息（stdout 是协议通道，人类可读提示走 stderr）
    if matches!(transport, InputTransport::Stdin(_)) {
        eprintln!("A2UI TUI Renderer ready. Send JSONL messages on stdin.");
        eprintln!("Waiting for createSurface...");
    }

    // 消息处理主循环。对端（Agent）间隔任意长时间发消息都是正常的，
    // 不设空闲超时——流结束由 receive() 的 EOF 语义（Ok(None)）判定
    loop {
        match transport.receive().await {
            Ok(Some(envelope)) => {
                if let Err(e) =
                    process_server_envelope(&mut renderer, envelope, &mut terminal).await
                {
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
