use crate::component::DynamicValue;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

pub type ActionContext = HashMap<String, DynamicValue>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct EventAction {
    pub name: String,
    pub surface_id: String,
    pub source_component_id: Option<String>,
    pub context: ActionContext,
    pub want_response: bool,
    pub response_path: Option<String>,
    pub action_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct FunctionCallAction {
    pub call: String,
    pub args: ActionContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Action {
    Event(EventAction),
    FunctionCall(FunctionCallAction),
}

/// 客户端 action 消息（规范：用户与声明了 server action 的组件交互时发送）。
///
/// 规范必填字段：`name`、`surfaceId`、`sourceComponentId`、`timestamp`
/// （ISO 8601）、`context`；`actionId` 在 `wantResponse=true` 时必填。
/// `responsePath` 是客户端本地语义（响应写回路径），不上线路——本结构
/// 保留该字段仅为构造期传递，序列化时若为 `None` 不输出。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct ActionMessage {
    pub name: String,
    pub surface_id: String,
    pub source_component_id: String,
    /// 事件发生时刻，ISO 8601 UTC（`YYYY-MM-DDTHH:MM:SSZ`）
    pub timestamp: String,
    #[serde(default)]
    pub context: ActionContext,
    #[serde(default)]
    pub want_response: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_id: Option<String>,
}

impl ActionMessage {
    /// 构造事件消息，`timestamp` 自动取当前 UTC 时间。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use a2ui_core::message::client_to_server::ActionMessage;
    ///
    /// let msg = ActionMessage::event("submit", "s1", "submit_button");
    /// assert_eq!(msg.source_component_id, "submit_button");
    /// assert!(msg.timestamp.ends_with('Z'));
    /// ```
    pub fn event(
        name: impl Into<String>,
        surface_id: impl Into<String>,
        source_component_id: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            surface_id: surface_id.into(),
            source_component_id: source_component_id.into(),
            timestamp: now_iso8601(),
            context: HashMap::new(),
            want_response: false,
            response_path: None,
            action_id: None,
        }
    }

    /// 覆盖时间戳（测试确定性用）
    pub fn with_timestamp(mut self, timestamp: impl Into<String>) -> Self {
        self.timestamp = timestamp.into();
        self
    }

    pub fn with_response(
        mut self,
        response_path: impl Into<String>,
        action_id: impl Into<String>,
    ) -> Self {
        self.want_response = true;
        self.response_path = Some(response_path.into());
        self.action_id = Some(action_id.into());
        self
    }

    pub fn with_context(mut self, key: impl Into<String>, value: DynamicValue) -> Self {
        self.context.insert(key.into(), value);
        self
    }
}

/// 当前 UTC 时间的 ISO 8601 字符串（秒精度）
fn now_iso8601() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    epoch_secs_to_iso8601(secs)
}

/// Unix 秒 → `YYYY-MM-DDTHH:MM:SSZ`。
/// 历法换算采用 Howard Hinnant 的 civil_from_days 算法，手写以避免
/// 为一个时间戳字段引入 chrono/time 依赖。
fn epoch_secs_to_iso8601(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (hh, mm, ss) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    if month <= 2 {
        year += 1;
    }
    format!("{year:04}-{month:02}-{day:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct FunctionResponse {
    pub function_call_id: String,
    pub call: String,
    pub value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct ClientError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub enum V1_0ClientMessage {
    Action(ActionMessage),
    FunctionResponse(FunctionResponse),
    Error(ClientError),
    Capabilities(crate::message::capabilities::Capabilities),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::V1_0ServerMessage;
    use crate::prelude::*;
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn test_action_message_carries_required_timestamp_and_source() {
        // 规范 action 消息：sourceComponentId 与 timestamp（ISO 8601）均必填
        let msg = ActionMessage::event("submit", "s1", "btn");
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["sourceComponentId"], "btn");
        let ts = json["timestamp"].as_str().expect("timestamp present");
        let re_ok = ts.len() == 20
            && ts.as_bytes()[4] == b'-'
            && ts.as_bytes()[7] == b'-'
            && ts.as_bytes()[10] == b'T'
            && ts.as_bytes()[13] == b':'
            && ts.as_bytes()[16] == b':'
            && ts.ends_with('Z');
        assert!(
            re_ok,
            "timestamp must be ISO 8601 UTC (YYYY-MM-DDTHH:MM:SSZ), got {ts}"
        );
    }

    #[test]
    fn test_action_message_rejects_missing_required_fields() {
        // 缺 timestamp
        let json = r#"{"name":"click","surfaceId":"s1","sourceComponentId":"btn"}"#;
        assert!(serde_json::from_str::<ActionMessage>(json).is_err());
        // 缺 sourceComponentId
        let json = r#"{"name":"click","surfaceId":"s1","timestamp":"2026-07-07T00:00:00Z"}"#;
        assert!(serde_json::from_str::<ActionMessage>(json).is_err());
    }

    #[test]
    fn test_action_message_roundtrip_with_all_spec_fields() {
        let json = r#"{
            "name": "submitForm",
            "surfaceId": "contact_form_1",
            "sourceComponentId": "submit_button",
            "timestamp": "2026-06-02T08:57:23Z",
            "context": {"isSubscribed": true},
            "wantResponse": true,
            "actionId": "form_submit_773"
        }"#;
        let msg: ActionMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.source_component_id, "submit_button");
        assert_eq!(msg.timestamp, "2026-06-02T08:57:23Z");
        assert!(msg.want_response);
    }

    #[test]
    fn test_epoch_to_iso8601_known_values() {
        assert_eq!(epoch_secs_to_iso8601(0), "1970-01-01T00:00:00Z");
        assert_eq!(epoch_secs_to_iso8601(1_000_000_000), "2001-09-09T01:46:40Z");
        // 闰年 2 月 29 日
        assert_eq!(epoch_secs_to_iso8601(951_782_400), "2000-02-29T00:00:00Z");
        // 世纪非闰年（2100）边界：3 月 1 日前一秒是 2 月 28 日
        assert_eq!(epoch_secs_to_iso8601(4_107_542_399), "2100-02-28T23:59:59Z");
    }

    #[test]
    fn test_action_event() {
        let action = Action::Event(EventAction {
            name: "submit".into(),
            surface_id: "s1".into(),
            source_component_id: Some("btn".into()),
            context: HashMap::new(),
            want_response: false,
            response_path: None,
            action_id: None,
        });
        let json = serde_json::to_value(&action).unwrap();
        assert_eq!(json["name"], "submit");
    }

    #[test]
    fn test_action_event_with_response() {
        let action = Action::Event(EventAction {
            name: "fetch".into(),
            surface_id: "s1".into(),
            source_component_id: None,
            context: HashMap::new(),
            want_response: true,
            response_path: Some("/result".into()),
            action_id: Some("act-1".into()),
        });
        let json = serde_json::to_value(&action).unwrap();
        assert!(json["wantResponse"].as_bool().unwrap());
        assert_eq!(json["responsePath"], "/result");
    }

    #[test]
    fn test_action_function_call() {
        let mut args = HashMap::new();
        args.insert(
            "value".into(),
            DynamicValue::Literal("test@example.com".into()),
        );
        let action = Action::FunctionCall(FunctionCallAction {
            call: "validate".into(),
            args,
        });
        let json = serde_json::to_value(&action).unwrap();
        assert_eq!(json["call"], "validate");
    }

    #[test]
    fn test_function_response() {
        let msg = FunctionResponse {
            function_call_id: "fc1".into(),
            call: "required".into(),
            value: json!(true),
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["value"], true);
    }

    #[test]
    fn test_client_error() {
        let msg = ClientError {
            code: "INVALID_FUNCTION_CALL".into(),
            message: "Function not registered".into(),
            function_call_id: Some("fc1".into()),
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["code"], "INVALID_FUNCTION_CALL");
    }

    #[test]
    fn test_server_envelope_deserialize_v1() {
        let json = r#"{"version":"v1.0","createSurface":{"surfaceId":"s1","catalogId":"basic"}}"#;
        let env: ServerEnvelope = serde_json::from_str(json).unwrap();
        assert!(matches!(env, ServerEnvelope::V1_0(_)));
    }

    #[test]
    fn test_server_envelope_deserialize_v1_0() {
        let json =
            r#"{"version":"v1.0","updateDataModel":{"surfaceId":"s1","path":"/","value":{"x":1}}}"#;
        let env: ServerEnvelope = serde_json::from_str(json).unwrap();
        let ServerEnvelope::V1_0(msg) = env;
        match msg {
            V1_0ServerMessage::UpdateDataModel(m) => assert_eq!(m.surface_id, "s1"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_client_envelope_action() {
        let json = r#"{"version":"v1.0","action":{"name":"click","surfaceId":"s1","sourceComponentId":"btn","timestamp":"2026-07-07T00:00:00Z"}}"#;
        let env: ClientEnvelope = serde_json::from_str(json).unwrap();
        let ClientEnvelope::V1_0 { message: msg, .. } = env;
        {
            match msg {
                V1_0ClientMessage::Action(a) => assert_eq!(a.name, "click"),
                _ => panic!("wrong variant"),
            }
        }
    }

    #[test]
    fn test_client_envelope_function_response() {
        let json = r#"{"version":"v1.0","functionResponse":{"functionCallId":"fc1","call":"req","value":true}}"#;
        let env: ClientEnvelope = serde_json::from_str(json).unwrap();
        let ClientEnvelope::V1_0 { message: msg, .. } = env;
        {
            match msg {
                V1_0ClientMessage::FunctionResponse(fr) => assert_eq!(fr.function_call_id, "fc1"),
                _ => panic!("wrong variant"),
            }
        }
    }

    #[test]
    fn test_client_envelope_error() {
        let json = r#"{"version":"v1.0","error":{"code":"E","message":"err"}}"#;
        let env: ClientEnvelope = serde_json::from_str(json).unwrap();
        let ClientEnvelope::V1_0 { message: msg, .. } = env;
        {
            match msg {
                V1_0ClientMessage::Error(e) => assert_eq!(e.code, "E"),
                _ => panic!("wrong variant"),
            }
        }
    }

    #[test]
    fn test_capabilities_serialization() {
        let caps = crate::message::capabilities::Capabilities {
            version: "1.0".to_string(),
            features: vec!["tui".to_string(), "gui".to_string()],
        };
        let json = serde_json::to_value(&caps).unwrap();
        assert_eq!(json["version"], "1.0");
        assert_eq!(json["features"][0], "tui");
        assert_eq!(json["features"][1], "gui");
    }

    #[test]
    fn test_capabilities_deny_unknown_fields() {
        let json = r#"{"version":"1.0","features":[],"extra":"x"}"#;
        let result: Result<crate::message::capabilities::Capabilities> =
            serde_json::from_str(json).map_err(Into::into);
        assert!(result.is_err());
    }

    #[test]
    fn test_capability_exchange_serialization() {
        let exchange = crate::message::capabilities::CapabilityExchange {
            client_capabilities: crate::message::capabilities::Capabilities {
                version: "1.0".to_string(),
                features: vec!["tui".to_string()],
            },
            server_capabilities: crate::message::capabilities::Capabilities {
                version: "1.0".to_string(),
                features: vec!["basic".to_string()],
            },
        };
        let json = serde_json::to_value(&exchange).unwrap();
        assert_eq!(json["clientCapabilities"]["version"], "1.0");
        assert_eq!(json["serverCapabilities"]["version"], "1.0");
        assert_eq!(json["serverCapabilities"]["features"][0], "basic");
    }

    #[test]
    fn test_client_envelope_capabilities() {
        let caps = crate::message::capabilities::Capabilities {
            version: "1.0".to_string(),
            features: vec!["tui".to_string()],
        };
        let msg = V1_0ClientMessage::Capabilities(caps);
        let envelope = ClientEnvelope::v1_0(msg);
        let json = serde_json::to_string(&envelope).unwrap();
        let parsed: ClientEnvelope = serde_json::from_str(&json).unwrap();
        if let ClientEnvelope::V1_0 {
            message: V1_0ClientMessage::Capabilities(c),
            ..
        } = parsed
        {
            assert_eq!(c.version, "1.0");
            assert_eq!(c.features, vec!["tui"]);
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn test_deny_unknown_fields_on_client_capabilities_message() {
        let json =
            r#"{"version":"v1.0","capabilities":{"version":"1.0","features":[],"extra":"x"}}"#;
        let result: Result<ClientEnvelope> = serde_json::from_str(json).map_err(Into::into);
        assert!(result.is_err());
    }

    #[test]
    fn test_client_envelope_capabilities_integration() {
        let caps = crate::message::capabilities::Capabilities {
            version: "1.0".to_string(),
            features: vec!["tui".to_string(), "jsonl".to_string()],
        };
        let envelope = ClientEnvelope::v1_0(V1_0ClientMessage::Capabilities(caps));
        let json = serde_json::to_string(&envelope).unwrap();
        assert!(json.contains("capabilities"));
        assert!(json.contains("tui"));
    }

    #[test]
    fn test_unknown_version_fails() {
        let json = r#"{"version":"v9.9","createSurface":{"surfaceId":"s1"}}"#;
        let result: Result<ServerEnvelope> = serde_json::from_str(json).map_err(Into::into);
        assert!(result.is_err());
    }

    #[test]
    fn test_deny_unknown_fields_on_action_message() {
        let json = r#"{"name":"click","surfaceId":"s1","unknownField":true}"#;
        let result: Result<ActionMessage> = serde_json::from_str(json).map_err(Into::into);
        assert!(result.is_err());
    }

    #[test]
    fn test_deny_unknown_fields_on_client_error() {
        let json = r#"{"code":"E","message":"err","extra":"x"}"#;
        let result: Result<ClientError> = serde_json::from_str(json).map_err(Into::into);
        assert!(result.is_err());
    }

    #[test]
    fn test_deny_unknown_fields_on_function_response() {
        let json = r#"{"functionCallId":"fc1","call":"req","value":true,"extra":"x"}"#;
        let result: Result<FunctionResponse> = serde_json::from_str(json).map_err(Into::into);
        assert!(result.is_err());
    }
}
