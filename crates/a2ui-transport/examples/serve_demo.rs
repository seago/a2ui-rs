//! WebSocket 服务端演示 —— 供 B2 交互式 Web 渲染器 M1.5 端到端手测使用。
//!
//! 运行：
//! ```bash
//! cargo run -p a2ui-transport --example serve_demo
//! ```
//!
//! 行为：
//! 1. 在 `127.0.0.1:8765` 监听 WebSocket 连接（log 输出实际地址）。
//! 2. 每接入一个连接，推送一条内嵌完整组件树 + dataModel 的 `createSurface`
//!    消息（Basic Catalog，构成一个简单表单：标题 Text + 绑定 `/form/name`
//!    的 TextField + primary Button，按钮以规范嵌套形状声明
//!    `action.event`（name="submit"、wantResponse、responsePath="/result"），
//!    外层 Card/Column 布局）。
//! 3. 循环接收 `ClientEnvelope`：只有声明了 server action 的组件交互才会
//!    产生 action 消息（输入类被动变更不上线路，最新数据随 action 的信封
//!    metadata 到达）。
//! 4. 若收到的 action 带 `wantResponse`，回 `actionResponse`；客户端按本地
//!    登记的 responsePath 把响应值写回 `/result` 并刷新界面。

use a2ui_core::component::component::Component;
use a2ui_core::component::{ComponentId, DynamicValue};
use a2ui_core::message::server_to_client::{ActionResponse, ActionResponsePayload, CreateSurface};
use a2ui_core::message::{V1_0ClientMessage, V1_0ServerMessage};
use a2ui_core::{ClientEnvelope, ServerEnvelope};
use a2ui_transport::{WebSocketServer, WebSocketServerConnection};
use serde_json::json;

const LISTEN_ADDR: &str = "127.0.0.1:8765";
const SURFACE_ID: &str = "demo-surface";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 该 example 不引入 tracing-subscriber 依赖；tracing 事件在无订阅者时为 no-op，
    // 关键信息同时通过 println! 输出，方便前端手测时直接观察。
    let server = WebSocketServer::bind(LISTEN_ADDR).await?;
    let addr = server.local_addr()?;
    tracing::info!("serve_demo listening on ws://{}/", addr);
    println!("serve_demo listening on ws://{}/  (Ctrl-C to stop)", addr);

    loop {
        let conn = server.accept().await?;
        tracing::info!("client connected: {}", conn.peer_addr());
        // 每个连接顺序处理；如需并发可 tokio::spawn。
        if let Err(e) = handle_connection(conn).await {
            tracing::warn!("connection ended: {}", e);
        }
    }
}

/// 处理单个连接：推送 createSurface，然后循环接收 action。
async fn handle_connection(
    mut conn: WebSocketServerConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    let create = build_create_surface()?;
    conn.push(ServerEnvelope::V1_0(V1_0ServerMessage::CreateSurface(
        create,
    )))
    .await?;
    tracing::info!("pushed createSurface '{}'", SURFACE_ID);

    loop {
        let envelope = match conn.receive().await {
            Ok(e) => e,
            Err(e) => {
                tracing::info!("receive stopped: {}", e);
                break;
            }
        };

        match envelope {
            ClientEnvelope::V1_0 {
                message: V1_0ClientMessage::Action(action),
                metadata,
            } => {
                tracing::info!(
                    "received action name='{}' surface='{}' source={:?} context={:?} wantResponse={}",
                    action.name,
                    action.surface_id,
                    action.source_component_id,
                    action.context,
                    action.want_response
                );
                println!("[action] {} -> {:?}", action.name, action.context);

                // sendDataModel 开启时，最新数据模型经信封级 metadata 到达
                // （被动输入不单独上线路，随下一次 action 一并携带）。
                let submitted_name = metadata
                    .as_ref()
                    .and_then(|m| m.data_model.as_ref())
                    .and_then(|dm| dm.pointer("/form/name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if let Some(m) = &metadata {
                    println!(
                        "[metadata] surface='{}' name='{}'",
                        m.surface_id, submitted_name
                    );
                }

                // 若客户端期望响应，回 actionResponse；客户端本地按登记的
                // responsePath（/result）写回展示，服务端无需知晓该路径。
                if action.want_response {
                    if let Some(action_id) = action.action_id.clone() {
                        let resp = ActionResponse {
                            action_id,
                            response: ActionResponsePayload::Success(json!(format!(
                                "✅ 服务端已收到提交：{}",
                                submitted_name
                            ))),
                        };
                        conn.push(ServerEnvelope::V1_0(V1_0ServerMessage::ActionResponse(
                            resp,
                        )))
                        .await?;
                        tracing::info!("sent actionResponse for '{}'", action.name);
                    }
                }
            }
            ClientEnvelope::V1_0 {
                message: V1_0ClientMessage::FunctionResponse(fr),
                ..
            } => {
                tracing::info!("received functionResponse: call='{}'", fr.call);
            }
            ClientEnvelope::V1_0 {
                message: V1_0ClientMessage::Error(err),
                ..
            } => {
                tracing::warn!("received client error: {} - {}", err.code, err.message);
            }
            ClientEnvelope::V1_0 {
                message: V1_0ClientMessage::Capabilities(caps),
                ..
            } => {
                tracing::info!("received capabilities: {:?}", caps.features);
            }
        }
    }
    Ok(())
}

/// 构建一个简单表单的 `createSurface`：
///
/// ```text
/// Card(root_card)
///   └─ Column(form_col)
///        ├─ Text(title_text)         "请输入你的名字"
///        ├─ TextField(name_field)    value 绑定 /form/name
///        ├─ Button(submit_btn)       variant=primary，声明 action.event
///        │    └─ Text(submit_label)  "提交"        name="submit" wantResponse
///        └─ Text(result_text)        text 绑定 /result（actionResponse 本地写回）
/// ```
fn build_create_surface() -> Result<CreateSurface, Box<dyn std::error::Error>> {
    let title = Component::text(
        ComponentId::new("title_text")?,
        DynamicValue::literal("请输入你的名字"),
    );

    let name_field = Component::text_field(ComponentId::new("name_field")?)
        .with_label("姓名")
        .with_placeholder("在此输入…")
        .with_text_variant("shortText")
        .with_value(DynamicValue::path("/form/name"));

    let submit_label = Component::text(
        ComponentId::new("submit_label")?,
        DynamicValue::literal("提交"),
    );

    // Button 通过 serde 构造，以便附加声明式 server action（规范嵌套
    // `action.event.*` 形状）：点击发送名为 "submit" 的 action，
    // wantResponse 使客户端自动生成 actionId 并登记 pending；
    // responsePath 是客户端本地语义（不上线路），actionResponse 到达后
    // 由客户端写回本地数据模型 /result。
    let submit_btn: Component = serde_json::from_value(json!({
        "component": "Button",
        "id": "submit_btn",
        "child": "submit_label",
        "variant": "primary",
        "action": {
            "event": {
                "name": "submit",
                "wantResponse": true,
                "responsePath": "/result"
            }
        }
    }))?;

    let form_col = Component::column(
        ComponentId::new("form_col")?,
        vec![
            ComponentId::new("title_text")?,
            ComponentId::new("name_field")?,
            ComponentId::new("submit_btn")?,
            ComponentId::new("result_text")?,
        ],
    );

    // actionResponse 写回 /result 后由此组件展示（客户端本地闭环）
    let result_text = Component::text(
        ComponentId::new("result_text")?,
        DynamicValue::path("/result"),
    );

    let root_card = Component::card(
        ComponentId::new("root_card")?,
        ComponentId::new("form_col")?,
    );

    Ok(CreateSurface {
        surface_id: SURFACE_ID.to_string(),
        catalog_id: "basic".to_string(),
        surface_properties: None,
        send_data_model: true,
        components: Some(vec![
            root_card,
            form_col,
            title,
            name_field,
            submit_btn,
            submit_label,
            result_text,
        ]),
        data_model: Some(json!({
            "form": { "name": "" },
            "result": ""
        })),
    })
}
