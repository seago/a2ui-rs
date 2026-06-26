use serde::{Deserialize, Serialize};

/// 能力描述：协议版本和支持的特性列表
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub version: String,
    pub features: Vec<String>,
}

/// 能力交换：客户端和服务端互相交换能力描述
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityExchange {
    pub client_capabilities: Capabilities,
    pub server_capabilities: Capabilities,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capabilities_serialization() {
        let caps = Capabilities {
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
        let result: std::result::Result<Capabilities, serde_json::Error> =
            serde_json::from_str(json).map_err(Into::into);
        assert!(result.is_err());
    }

    #[test]
    fn test_capability_exchange_serialization() {
        let exchange = CapabilityExchange {
            client_capabilities: Capabilities {
                version: "1.0".to_string(),
                features: vec!["tui".to_string()],
            },
            server_capabilities: Capabilities {
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
    fn test_capability_exchange_deny_unknown_fields() {
        let json = r#"{"clientCapabilities":{"version":"1.0","features":[]},"serverCapabilities":{"version":"1.0","features":[]},"extra":"x"}"#;
        let result: std::result::Result<CapabilityExchange, serde_json::Error> =
            serde_json::from_str(json).map_err(Into::into);
        assert!(result.is_err());
    }

    #[test]
    fn test_capabilities_empty_features() {
        let caps = Capabilities {
            version: "1.0".to_string(),
            features: vec![],
        };
        let json = serde_json::to_string(&caps).unwrap();
        let parsed: Capabilities = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.version, "1.0");
        assert!(parsed.features.is_empty());
    }
}
