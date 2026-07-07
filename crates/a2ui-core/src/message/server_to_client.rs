use crate::component::component::Component;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResponseError {
    pub code: String,
    pub message: String,
}

/// actionResponse 的 payload：规范要求 `value`（成功）与 `error`（失败）
/// 恰有其一。外部标签表示法（`{"value": ...}` / `{"error": {...}}`）天然
/// 满足互斥——多键或未知键的 map 无法匹配任何变体。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionResponsePayload {
    /// 成功：`{"value": <any>}`
    #[serde(rename = "value")]
    Success(Value),
    /// 失败：`{"error": {"code": ..., "message": ...}}`
    #[serde(rename = "error")]
    Error(ResponseError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CallFunctionPayload {
    pub call: String,
    pub args: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct CreateSurface {
    pub surface_id: String,
    pub catalog_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surface_properties: Option<Value>,
    #[serde(default)]
    pub send_data_model: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub components: Option<Vec<Component>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_model: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct UpdateComponents {
    pub surface_id: String,
    pub components: Vec<Component>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDataModel {
    pub surface_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct DeleteSurface {
    pub surface_id: String,
}

/// actionResponse 消息。规范 wire 格式（actionId 在信封层、与
/// actionResponse 键平级）：
///
/// ```json
/// {"version":"v1.0","actionId":"a1","actionResponse":{"value":"done"}}
/// ```
///
/// 本结构直接序列化为这两个平级键，经 `V1_0ServerMessage` 的
/// untagged 变体并入信封。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct ActionResponse {
    pub action_id: String,
    #[serde(rename = "actionResponse")]
    pub response: ActionResponsePayload,
}

impl ActionResponse {
    /// 校验 ActionResponse 的基本结构有效性
    pub fn validate(&self) -> crate::error::Result<()> {
        if self.action_id.is_empty() {
            return Err(crate::error::A2uiError::ValidationError {
                message: "actionId must not be empty".to_string(),
                component_id: "actionResponse".to_string(),
                check_index: 0,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct CallFunction {
    pub function_call_id: String,
    pub want_response: bool,
    #[serde(flatten)]
    pub call: CallFunctionPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub enum V1_0ServerMessage {
    CreateSurface(CreateSurface),
    UpdateComponents(UpdateComponents),
    UpdateDataModel(UpdateDataModel),
    DeleteSurface(DeleteSurface),
    CallFunction(CallFunction),
    Capabilities(crate::message::capabilities::Capabilities),
    /// untagged（serde 要求置于枚举末尾）：actionResponse 消息在 wire 上
    /// 是两个平级键（actionId + actionResponse），由 ActionResponse 自身的
    /// Serialize/Deserialize 承载，不走单键外部标签
    #[serde(untagged)]
    ActionResponse(ActionResponse),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::{ComponentId, DynamicValue};
    use crate::prelude::*;
    use serde_json::json;

    #[test]
    fn test_create_surface_serialize() {
        let msg = CreateSurface {
            surface_id: "s1".into(),
            catalog_id: "basic".into(),
            surface_properties: None,
            send_data_model: false,
            components: None,
            data_model: None,
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["surfaceId"], "s1");
        assert_eq!(json["catalogId"], "basic");
        assert_eq!(json["sendDataModel"], false);
    }

    #[test]
    fn test_create_surface_with_components() {
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        let msg = CreateSurface {
            surface_id: "s1".into(),
            catalog_id: "basic".into(),
            surface_properties: None,
            send_data_model: false,
            components: Some(vec![comp]),
            data_model: None,
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["components"][0]["id"], "root");
    }

    #[test]
    fn test_update_components() {
        let comp = Component::text(
            ComponentId::new("title").unwrap(),
            DynamicValue::Literal("Title".to_string()),
        );
        let msg = UpdateComponents {
            surface_id: "s1".into(),
            components: vec![comp],
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["surfaceId"], "s1");
        assert_eq!(json["components"][0]["id"], "title");
    }

    #[test]
    fn test_update_data_model() {
        let msg = UpdateDataModel {
            surface_id: "s1".into(),
            path: None,
            value: Some(json!({"name": "Alice"})),
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["value"]["name"], "Alice");
    }

    #[test]
    fn test_update_data_model_delete() {
        let msg = UpdateDataModel {
            surface_id: "s1".into(),
            path: Some("/name".into()),
            value: None,
        };
        assert!(msg.value.is_none());
    }

    #[test]
    fn test_delete_surface() {
        let msg = DeleteSurface {
            surface_id: "s1".into(),
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["surfaceId"], "s1");
    }

    #[test]
    fn test_action_response_success() {
        let msg = ActionResponse {
            action_id: "act1".into(),
            response: ActionResponsePayload::Success(json!({"value": "done"})),
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["actionId"], "act1");
        assert_eq!(json["actionResponse"]["value"]["value"], "done");
    }

    #[test]
    fn test_action_response_error() {
        let msg = ActionResponse {
            action_id: "act1".into(),
            response: ActionResponsePayload::Error(ResponseError {
                code: "INVALID_INPUT".into(),
                message: "Invalid data".into(),
            }),
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["actionId"], "act1");
        assert_eq!(json["actionResponse"]["error"]["code"], "INVALID_INPUT");
    }

    #[test]
    fn test_call_function() {
        let msg = CallFunction {
            function_call_id: "fc1".into(),
            want_response: true,
            call: CallFunctionPayload {
                call: "formatString".into(),
                args: json!({"template": "Hi"}),
            },
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["functionCallId"], "fc1");
        assert_eq!(json["call"], "formatString");
        assert!(json["wantResponse"].as_bool().unwrap());
    }

    #[test]
    fn test_deserialize_create_surface() {
        let json = r#"{"version":"v1.0","createSurface":{"surfaceId":"s1","catalogId":"basic"}}"#;
        let envelope: ServerEnvelope = serde_json::from_str(json).unwrap();
        match envelope {
            ServerEnvelope::V1_0(V1_0ServerMessage::CreateSurface(msg)) => {
                assert_eq!(msg.surface_id, "s1");
                assert_eq!(msg.catalog_id, "basic");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_deserialize_update_components() {
        let json = r#"{"version":"v1.0","updateComponents":{"surfaceId":"s1","components":[]}}"#;
        let envelope: ServerEnvelope = serde_json::from_str(json).unwrap();
        match envelope {
            ServerEnvelope::V1_0(V1_0ServerMessage::UpdateComponents(msg)) => {
                assert_eq!(msg.surface_id, "s1");
                assert!(msg.components.is_empty());
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_deserialize_action_response() {
        // 规范 wire：actionId 在信封层（与 actionResponse 平级），payload 用 value 包装
        let json = r#"{"version":"v1.0","actionId":"a1","actionResponse":{"value":"ok"}}"#;
        let envelope: ServerEnvelope = serde_json::from_str(json).unwrap();
        match envelope {
            ServerEnvelope::V1_0(V1_0ServerMessage::ActionResponse(msg)) => {
                assert_eq!(msg.action_id, "a1");
                match msg.response {
                    ActionResponsePayload::Success(v) => assert_eq!(v, json!("ok")),
                    ActionResponsePayload::Error(_) => panic!("expected Success"),
                }
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_deserialize_action_response_error_variant() {
        let json = r#"{"version":"v1.0","actionId":"a1","actionResponse":{"error":{"code":"INVALID_INPUT","message":"bad data"}}}"#;
        let envelope: ServerEnvelope = serde_json::from_str(json).unwrap();
        match envelope {
            ServerEnvelope::V1_0(V1_0ServerMessage::ActionResponse(msg)) => {
                assert_eq!(msg.action_id, "a1");
                match msg.response {
                    ActionResponsePayload::Error(err) => {
                        assert_eq!(err.code, "INVALID_INPUT");
                        assert_eq!(err.message, "bad data");
                    }
                    ActionResponsePayload::Success(v) => {
                        panic!("error response must deserialize to Error variant, got Success({v})")
                    }
                }
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_serialize_action_response_spec_wire() {
        let envelope = ServerEnvelope::V1_0(V1_0ServerMessage::ActionResponse(ActionResponse {
            action_id: "a-123".into(),
            response: ActionResponsePayload::Success(json!({"total": 3})),
        }));
        let value = serde_json::to_value(&envelope).unwrap();
        assert_eq!(value["version"], "v1.0");
        assert_eq!(value["actionId"], "a-123", "actionId 必须在信封层");
        assert_eq!(value["actionResponse"]["value"]["total"], 3);
        // payload 内不得再出现 actionId
        assert!(value["actionResponse"].get("actionId").is_none());
    }

    #[test]
    fn test_action_response_scalar_success_serializes() {
        // 旧实现 flatten 标量 Value 会在序列化时报错；规范的 value 包装无此限制
        let envelope = ServerEnvelope::V1_0(V1_0ServerMessage::ActionResponse(ActionResponse {
            action_id: "a1".into(),
            response: ActionResponsePayload::Success(json!("done")),
        }));
        let value = serde_json::to_value(&envelope).unwrap();
        assert_eq!(value["actionResponse"]["value"], "done");
    }

    #[test]
    fn test_action_response_rejects_old_payload_level_action_id() {
        // 旧（与规范冲突的）wire 格式必须被拒绝
        let json = r#"{"version":"v1.0","actionResponse":{"actionId":"a1","value":"ok"}}"#;
        assert!(serde_json::from_str::<ServerEnvelope>(json).is_err());
    }

    #[test]
    fn test_action_response_rejects_both_value_and_error() {
        // 规范：Exactly one of value or error must be present
        let json = r#"{"version":"v1.0","actionId":"a1","actionResponse":{"value":1,"error":{"code":"E","message":"m"}}}"#;
        assert!(serde_json::from_str::<ServerEnvelope>(json).is_err());
    }

    #[test]
    fn test_deserialize_unknown_message_fails() {
        let json = r#"{"version":"v1.0","unknownMessage":{}}"#;
        let result: Result<ServerEnvelope> = serde_json::from_str(json).map_err(|e| e.into());
        assert!(result.is_err());
    }

    #[test]
    fn test_deny_unknown_fields_on_create_surface() {
        let json = r#"{"surfaceId":"s1","catalogId":"basic","extraField":"value"}"#;
        let result: Result<CreateSurface> = serde_json::from_str(json).map_err(Into::into);
        assert!(result.is_err());
    }

    #[test]
    fn test_deny_unknown_fields_on_call_function() {
        let json = r#"{"functionCallId":"fc1","wantResponse":true,"call":{"call":"test","args":{}},"extra":"x"}"#;
        let result: Result<CallFunction> = serde_json::from_str(json).map_err(Into::into);
        assert!(result.is_err());
    }

    #[test]
    fn test_server_envelope_capabilities() {
        let caps = crate::message::capabilities::Capabilities {
            version: "1.0".to_string(),
            features: vec!["basic".to_string()],
        };
        let envelope = ServerEnvelope::V1_0(V1_0ServerMessage::Capabilities(caps));
        let json = serde_json::to_string(&envelope).unwrap();
        let parsed: ServerEnvelope = serde_json::from_str(&json).unwrap();
        if let ServerEnvelope::V1_0(V1_0ServerMessage::Capabilities(c)) = parsed {
            assert_eq!(c.version, "1.0");
            assert_eq!(c.features, vec!["basic"]);
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn test_deny_unknown_fields_on_server_capabilities_message() {
        let json =
            r#"{"version":"v1.0","capabilities":{"version":"1.0","features":[],"extra":"x"}}"#;
        let result: Result<ServerEnvelope> = serde_json::from_str(json).map_err(Into::into);
        assert!(result.is_err());
    }

    #[test]
    fn test_deny_unknown_fields_on_delete_surface() {
        let json = r#"{"surfaceId":"s1","extra":"x"}"#;
        let result: Result<DeleteSurface> = serde_json::from_str(json).map_err(Into::into);
        assert!(result.is_err());
    }
}
