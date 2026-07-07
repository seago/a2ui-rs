//! A2UI Web 渲染器 — 完整 HTML 页面生成演示
//!
//! 创建一个包含多种组件的演示 Surface，渲染为 HTML 文件。
//!
//! 运行：`cargo run --example simple_web -p a2ui-renderer-web`
//! 输出：当前目录下的 `a2ui_demo.html`

use a2ui_core::message::server_to_client::CreateSurface;
use a2ui_core::prelude::*;
use a2ui_renderer::Renderer;
use a2ui_renderer_web::WebRenderer;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化渲染器
    let mut renderer = WebRenderer::new();

    // 注册 Basic Catalog 函数
    renderer.register_function("required", a2ui_renderer::CallableFrom::ClientOnly);
    renderer.register_function("regex", a2ui_renderer::CallableFrom::ClientOnly);
    renderer.register_function("email", a2ui_renderer::CallableFrom::ClientOnly);
    renderer.register_function("length", a2ui_renderer::CallableFrom::ClientOnly);
    renderer.register_function("numeric", a2ui_renderer::CallableFrom::ClientOnly);
    renderer.register_function("formatString", a2ui_renderer::CallableFrom::ClientOrRemote);
    renderer.register_function("formatNumber", a2ui_renderer::CallableFrom::ClientOrRemote);
    renderer.register_function(
        "formatCurrency",
        a2ui_renderer::CallableFrom::ClientOrRemote,
    );
    renderer.register_function("formatDate", a2ui_renderer::CallableFrom::ClientOrRemote);
    renderer.register_function("pluralize", a2ui_renderer::CallableFrom::ClientOrRemote);
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
            ComponentId::new("header").unwrap(),
            ComponentId::new("divider1").unwrap(),
            ComponentId::new("card1").unwrap(),
            ComponentId::new("divider2").unwrap(),
            ComponentId::new("form_row").unwrap(),
            ComponentId::new("divider3").unwrap(),
            ComponentId::new("actions_row").unwrap(),
            ComponentId::new("divider4").unwrap(),
            ComponentId::new("list_section").unwrap(),
            ComponentId::new("footer").unwrap(),
        ],
    );

    // 标题
    let header: Component = serde_json::from_value(json!({
        "id": "header",
        "component": "Text",
        "text": "🌐 A2UI Web Renderer Demo"
    }))?;

    // 分割线
    let divider1: Component = serde_json::from_value(json!({
        "id": "divider1", "component": "Divider"
    }))?;
    let divider2: Component = serde_json::from_value(json!({
        "id": "divider2", "component": "Divider"
    }))?;
    let divider3: Component = serde_json::from_value(json!({
        "id": "divider3", "component": "Divider"
    }))?;
    let divider4: Component = serde_json::from_value(json!({
        "id": "divider4", "component": "Divider"
    }))?;

    // 卡片（内嵌文本描述）
    let card_text: Component = serde_json::from_value(json!({
        "id": "card_text",
        "component": "Text",
        "text": "这是 A2UI Web 渲染器的演示页面。所有组件均由 Rust 后端渲染为语义化 HTML，支持 XSS 安全转义。"
    }))?;
    let card1: Component = serde_json::from_value(json!({
        "id": "card1",
        "component": "Card",
        "child": "card_text"
    }))?;

    // 表单行（输入组件）
    let name_input: Component = serde_json::from_value(json!({
        "id": "name_input",
        "component": "TextField",
        "value": "",
        "placeholder": "请输入姓名",
        "variant": "shortText"
    }))?;
    let email_input: Component = serde_json::from_value(json!({
        "id": "email_input",
        "component": "TextField",
        "value": "",
        "placeholder": "请输入邮箱",
        "variant": "shortText"
    }))?;
    let agree_cb: Component = serde_json::from_value(json!({
        "id": "agree_cb",
        "component": "CheckBox",
        "checked": true,
        "label": "同意服务条款"
    }))?;
    let volume_slider: Component = serde_json::from_value(json!({
        "id": "volume_slider",
        "component": "Slider",
        "value": 75,
        "min": 0,
        "max": 100
    }))?;
    let form_row: Component = serde_json::from_value(json!({
        "id": "form_row",
        "component": "Row",
        "children": { "children": [
            "name_input", "email_input", "agree_cb", "volume_slider"
        ]}
    }))?;

    // 操作按钮行（声明式 server action：点击才发送对应语义名的 action 消息）
    let submit_btn: Component = serde_json::from_value(json!({
        "id": "submit_btn",
        "component": "Button",
        "child": "submit_label",
        "variant": "primary",
        "action": { "event": { "name": "form_submit" } }
    }))?;
    let submit_label: Component = Component::text(
        ComponentId::new("submit_label").unwrap(),
        DynamicValue::Literal("提交".to_string()),
    );
    let cancel_btn: Component = serde_json::from_value(json!({
        "id": "cancel_btn",
        "component": "Button",
        "child": "cancel_label",
        "variant": "default",
        "action": { "event": { "name": "form_cancel" } }
    }))?;
    let cancel_label: Component = Component::text(
        ComponentId::new("cancel_label").unwrap(),
        DynamicValue::Literal("取消".to_string()),
    );
    let img_placeholder: Component = serde_json::from_value(json!({
        "id": "img_placeholder",
        "component": "Image",
        "url": "https://via.placeholder.com/80x80.png?text=A2UI"
    }))?;
    let actions_row: Component = serde_json::from_value(json!({
        "id": "actions_row",
        "component": "Row",
        "children": { "children": [
            "submit_btn", "cancel_btn", "img_placeholder"
        ]}
    }))?;

    // 列表部分
    let item1: Component = serde_json::from_value(json!({
        "id": "item1",
        "component": "Text",
        "text": "✅  服务端渲染 — Rust 后端直接输出 HTML"
    }))?;
    let item2: Component = serde_json::from_value(json!({
        "id": "item2",
        "component": "Text",
        "text": "🔒  XSS 安全 — 所有输出经过 HTML 转义"
    }))?;
    let item3: Component = serde_json::from_value(json!({
        "id": "item3",
        "component": "Text",
        "text": "🎨  语义化 HTML — 18 个组件完整映射"
    }))?;
    let list_section: Component = serde_json::from_value(json!({
        "id": "list_section",
        "component": "List",
        "children": { "children": ["item1", "item2", "item3"] }
    }))?;

    // 页脚
    let footer: Component = serde_json::from_value(json!({
        "id": "footer",
        "component": "Text",
        "text": "Powered by A2UI Protocol v1.0 · a2ui-rs · 2026"
    }))?;

    // 创建 Surface
    renderer
        .create_surface(CreateSurface {
            surface_id: "demo".into(),
            catalog_id: "a2ui://catalogs/basic/v1".into(),
            surface_properties: Some(json!({"agentDisplayName": "A2UI Web Demo"})),
            send_data_model: false,
            components: Some(vec![
                root,
                header,
                divider1,
                divider2,
                divider3,
                divider4,
                card_text,
                card1,
                name_input,
                email_input,
                agree_cb,
                volume_slider,
                form_row,
                submit_btn,
                submit_label,
                cancel_btn,
                cancel_label,
                img_placeholder,
                actions_row,
                item1,
                item2,
                item3,
                list_section,
                footer,
            ]),
            data_model: None,
        })
        .await?;

    // 渲染为完整 HTML 页面
    let html = renderer.render_all_html();

    // 写入文件
    let path = "a2ui_demo.html";
    std::fs::write(path, &html)?;
    println!("✅ HTML 页面已生成: {}", path);
    println!(
        "   在浏览器中打开 file://{}/{}",
        std::env::current_dir()?.display(),
        path
    );

    // 同时输出到 stdout
    println!("\n--- HTML 预览（前 500 字符）---");
    println!("{}", &html[..html.len().min(500)]);
    println!("... (总共 {} 字节)", html.len());

    Ok(())
}
