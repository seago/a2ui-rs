//! A2UI TUI 渲染器 — 交互式终端演示
//!
//! 在终端中渲染 A2UI 组件树，按 `q` 退出。
//!
//! 运行：`cargo run --example simple_tui -p a2ui-renderer-tui`

use a2ui_core::message::server_to_client::CreateSurface;
use a2ui_core::prelude::*;
use a2ui_renderer::Renderer;
use a2ui_renderer_tui::TuiRenderer;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化渲染器
    let mut renderer = TuiRenderer::new();

    // 注册函数
    renderer.register_function("required", a2ui_renderer::CallableFrom::ClientOnly);
    renderer.register_function("formatString", a2ui_renderer::CallableFrom::ClientOrRemote);
    renderer.register_function("openUrl", a2ui_renderer::CallableFrom::ClientOnly);

    // 注册 Basic Catalog
    let catalog: a2ui_core::Catalog = serde_json::from_value(json!({
        "catalogId": "basic",
        "instructions": "Basic catalog",
        "components": {},
        "functions": {}
    }))?;
    renderer.register_catalog(catalog).ok();

    // 构建演示组件树
    let root = Component::column(
        ComponentId::new("root").unwrap(),
        vec![
            ComponentId::new("title").unwrap(),
            ComponentId::new("subtitle").unwrap(),
            ComponentId::new("div1").unwrap(),
            ComponentId::new("tf").unwrap(),
            ComponentId::new("cb").unwrap(),
            ComponentId::new("sl").unwrap(),
            ComponentId::new("btn").unwrap(),
            ComponentId::new("div2").unwrap(),
            ComponentId::new("item1").unwrap(),
            ComponentId::new("item2").unwrap(),
            ComponentId::new("item3").unwrap(),
            ComponentId::new("footer").unwrap(),
        ],
    );

    let title: Component = serde_json::from_value(json!({
        "id": "title",
        "component": "Text",
        "text": "🖥  A2UI TUI Renderer Demo"
    }))?;
    let subtitle: Component = serde_json::from_value(json!({
        "id": "subtitle",
        "component": "Text",
        "text": "终端界面渲染演示 · 按 q 退出"
    }))?;
    let div1: Component = serde_json::from_value(json!({
        "id": "div1", "component": "Divider"
    }))?;
    let div2: Component = serde_json::from_value(json!({
        "id": "div2", "component": "Divider"
    }))?;
    let tf: Component = serde_json::from_value(json!({
        "id": "tf",
        "component": "TextField",
        "value": "Hello TUI",
        "placeholder": "输入文本..."
    }))?;
    let cb: Component = serde_json::from_value(json!({
        "id": "cb",
        "component": "CheckBox",
        "checked": true,
        "label": "启用通知"
    }))?;
    let sl: Component = serde_json::from_value(json!({
        "id": "sl",
        "component": "Slider",
        "value": 60,
        "min": 0,
        "max": 100
    }))?;
    let btn_label = Component::text(
        ComponentId::new("btn_label").unwrap(),
        DynamicValue::Literal("点击我".to_string()),
    );
    let btn: Component = serde_json::from_value(json!({
        "id": "btn",
        "component": "Button",
        "child": "btn_label",
        "variant": "primary"
    }))?;
    let item1: Component = serde_json::from_value(json!({
        "id": "item1",
        "component": "Text",
        "text": "  ✅  ratatui 终端渲染"
    }))?;
    let item2: Component = serde_json::from_value(json!({
        "id": "item2",
        "component": "Text",
        "text": "  ⌨   键盘事件 → action 消息"
    }))?;
    let item3: Component = serde_json::from_value(json!({
        "id": "item3",
        "component": "Text",
        "text": "  🔄  增量渲染 + DependencyGraph"
    }))?;
    let footer: Component = serde_json::from_value(json!({
        "id": "footer",
        "component": "Text",
        "text": "A2UI Protocol v1.0 · a2ui-rs"
    }))?;

    // 创建 Surface
    renderer
        .create_surface(CreateSurface {
            surface_id: "demo".into(),
            catalog_id: "basic".into(),
            surface_properties: Some(json!({"agentDisplayName": "A2UI TUI Demo"})),
            send_data_model: false,
            components: Some(vec![
                root,
                title, subtitle, div1, div2,
                tf, cb, sl, btn_label, btn,
                item1, item2, item3,
                footer,
            ]),
            data_model: None,
        })
        .await?;

    // 设置终端
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    // 事件循环：每帧渲染 + 非阻塞按键检测
    loop {
        // 渲染当前帧（render_frame 内部调用 terminal.draw）
        renderer.render_frame(&mut terminal).await.ok();

        // 非阻塞检查退出键
        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    // 方向键、Tab 等可由渲染器处理
                    _ => {}
                }
            }
        }
    }

    // 恢复终端
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    println!("✅ TUI 演示结束");

    Ok(())
}
