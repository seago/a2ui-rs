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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct ActionMessage {
    pub name: String,
    pub surface_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_component_id: Option<String>,
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
    pub fn event(name: impl Into<String>, surface_id: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            surface_id: surface_id.into(),
            source_component_id: None,
            context: HashMap::new(),
            want_response: false,
            response_path: None,
            action_id: None,
        }
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
        if let ServerEnvelope::V1_0(msg) = env {
            match msg {
                V1_0ServerMessage::UpdateDataModel(m) => assert_eq!(m.surface_id, "s1"),
                _ => panic!("wrong variant"),
            }
        }
    }

    #[test]
    fn test_client_envelope_action() {
        let json = r#"{"version":"v1.0","action":{"name":"click","surfaceId":"s1"}}"#;
        let env: ClientEnvelope = serde_json::from_str(json).unwrap();
        if let ClientEnvelope::V1_0 { message: msg, .. } = env {
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
        if let ClientEnvelope::V1_0 { message: msg, .. } = env {
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
        if let ClientEnvelope::V1_0 { message: msg, .. } = env {
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
