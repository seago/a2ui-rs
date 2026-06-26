use a2ui_core::prelude::*;
use a2ui_renderer_tui::TuiRenderer;
use a2ui_transport::JsonlTransport;
use clap::{Parser, Subcommand};
use std::io;
use tracing::info;

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
    // 初始化 tracing
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Render { input } => {
            run_render(input).await?;
        }
    }

    Ok(())
}

async fn run_render(input: Option<std::path::PathBuf>) -> anyhow::Result<()> {
    info!("Starting A2UI renderer");

    let mut renderer = TuiRenderer::new();

    // 简化实现：初始化 transport 占位（实际渲染逻辑在后续迭代中完成）
    if input.is_some() {
        let path = input.unwrap();
        let file = std::fs::File::open(path)?;
        let reader = tokio::fs::File::from_std(file);
        let _transport = JsonlTransport::new(reader, tokio::io::stdout());
        let _ = _transport;
    } else {
        let _transport = JsonlTransport::from_std();
        let _ = _transport;
    }

    info!("Transport initialized, waiting for messages...");

    // 简化实现：只打印欢迎信息
    println!("A2UI TUI Renderer ready. Send JSONL messages on stdin.");
    println!("Waiting for createSurface...");

    Ok(())
}
