use crate::error::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(tag = "version")]
pub enum ServerEnvelope {
    #[serde(rename = "v1.0")]
    V1_0(super::server_to_client::V1_0ServerMessage),
}

impl ServerEnvelope {
    pub fn from_json(json: &str) -> Result<Self> {
        Ok(serde_json::from_str(json)?)
    }

    pub fn to_value(&self) -> Result<Value> {
        Ok(serde_json::to_value(self)?)
    }
}

/// 客户端信封级 metadata（transport metadata 机制在本 WS/JSONL binding 下的载体）。
///
/// 规范：sendDataModel 开启时，客户端把该 surface 的完整数据模型随每条
/// 消息经 transport metadata 附带（"tagged by its surfaceId"）。
///
/// # 示例
///
/// ```rust
/// use a2ui_core::message::envelope::ClientMetadata;
///
/// let metadata = ClientMetadata {
///     surface_id: "s1".to_string(),
///     data_model: Some(serde_json::json!({"form": {"name": "alice"}})),
/// };
/// let value = serde_json::to_value(&metadata).unwrap();
/// assert_eq!(value["surfaceId"], "s1");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct ClientMetadata {
    /// 数据模型所属的 surface
    pub surface_id: String,
    /// 该 surface 的完整数据模型快照
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_model: Option<Value>,
}

/// 客户端 → 服务端信封。
///
/// wire 格式：`{"version":"v1.0", "<消息键>": {...}, "metadata": {...}?}`，
/// `metadata` 为可选的信封级字段、与消息键平级。
///
/// 注意：因 `metadata` 需与 flatten 的消息共存，信封层无法使用
/// `deny_unknown_fields`；消息键的合法性由内层 `V1_0ClientMessage`
/// （externally-tagged 枚举，未知键无匹配变体即报错）保证。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "version")]
pub enum ClientEnvelope {
    #[serde(rename = "v1.0")]
    V1_0 {
        #[serde(flatten)]
        message: super::client_to_server::V1_0ClientMessage,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        metadata: Option<ClientMetadata>,
    },
}

impl ClientEnvelope {
    /// 构造不带 metadata 的 v1.0 信封（最常见形态）
    pub fn v1_0(message: super::client_to_server::V1_0ClientMessage) -> Self {
        Self::V1_0 {
            message,
            metadata: None,
        }
    }

    /// 附加信封级 metadata
    pub fn with_metadata(self, metadata: ClientMetadata) -> Self {
        let Self::V1_0 { message, .. } = self;
        Self::V1_0 {
            message,
            metadata: Some(metadata),
        }
    }

    /// 信封内的消息
    pub fn message(&self) -> &super::client_to_server::V1_0ClientMessage {
        let Self::V1_0 { message, .. } = self;
        message
    }

    /// 信封级 metadata（如有）
    pub fn metadata(&self) -> Option<&ClientMetadata> {
        let Self::V1_0 { metadata, .. } = self;
        metadata.as_ref()
    }

    pub fn from_json(json: &str) -> Result<Self> {
        Ok(serde_json::from_str(json)?)
    }

    pub fn to_value(&self) -> Result<Value> {
        Ok(serde_json::to_value(self)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::client_to_server::V1_0ClientMessage;
    use serde_json::json;

    #[test]
    fn client_envelope_deserializes_metadata_sibling_of_action() {
        // 规范：sendDataModel 时数据模型经 transport metadata 附带，
        // 本 binding 定义为信封级字段（与 web-react 参考客户端一致）
        let json = r#"{
            "version": "v1.0",
            "action": {"name": "submit", "surfaceId": "s1", "sourceComponentId": "btn", "timestamp": "2026-07-07T00:00:00Z"},
            "metadata": {"surfaceId": "s1", "dataModel": {"form": {"name": "alice"}}}
        }"#;
        let envelope = ClientEnvelope::from_json(json).unwrap();
        let ClientEnvelope::V1_0 { message, metadata } = envelope;
        assert!(matches!(message, V1_0ClientMessage::Action(_)));
        let metadata = metadata.expect("metadata should be parsed");
        assert_eq!(metadata.surface_id, "s1");
        assert_eq!(
            metadata.data_model,
            Some(json!({"form": {"name": "alice"}}))
        );
    }

    #[test]
    fn client_envelope_serializes_metadata_at_envelope_level() {
        let envelope = ClientEnvelope::V1_0 {
            message: V1_0ClientMessage::Action(
                crate::message::client_to_server::ActionMessage::event("submit", "s1", "btn"),
            ),
            metadata: Some(ClientMetadata {
                surface_id: "s1".into(),
                data_model: Some(json!({"x": 1})),
            }),
        };
        let value = envelope.to_value().unwrap();
        assert_eq!(value["version"], "v1.0");
        assert_eq!(value["action"]["name"], "submit");
        // metadata 与 action 平级（信封层），不在 action.context 内
        assert_eq!(value["metadata"]["surfaceId"], "s1");
        assert_eq!(value["metadata"]["dataModel"]["x"], 1);
        assert!(value["action"]["context"].get("dataModel").is_none());
    }

    #[test]
    fn client_envelope_without_metadata_roundtrips_as_before() {
        // 无 metadata 时序列化不产生该字段；旧格式照常解析
        let json = r#"{"version":"v1.0","capabilities":{"version":"1.0","features":["tui"]}}"#;
        let envelope = ClientEnvelope::from_json(json).unwrap();
        let value = envelope.to_value().unwrap();
        assert!(value.get("metadata").is_none());
        assert_eq!(value["capabilities"]["version"], "1.0");
    }

    #[test]
    fn client_envelope_unknown_message_key_fails() {
        let json = r#"{"version":"v1.0","unknownMessage":{}}"#;
        assert!(ClientEnvelope::from_json(json).is_err());
    }
}
