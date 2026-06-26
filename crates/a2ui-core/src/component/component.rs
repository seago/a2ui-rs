use crate::error::{A2uiError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---- ComponentId ----

/// 组件标识符，遵循 Unicode UAX #31 命名规则
/// 正则: ^[\p{XID_Start}_][\p{XID_Continue}]*$
/// @ 命名空间保留给系统
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ComponentId(String);

impl ComponentId {
    /// 创建新的 ComponentId，校验命名规则
    pub fn new<S: AsRef<str>>(s: S) -> Result<Self> {
        let s = s.as_ref();
        if s.is_empty() {
            return Err(A2uiError::InvalidComponentId("empty ID".to_string()));
        }
        if s.starts_with('@') {
            return Err(A2uiError::InvalidComponentId(
                format!("'@' namespace is reserved for system: {}", s)
            ));
        }
        // UAX #31: 首字符必须是 XID_Start 或 _
        let mut chars = s.chars();
        let first = chars.next().unwrap();
        if !is_xid_start(first) {
            return Err(A2uiError::InvalidComponentId(
                format!("ID must start with XID_Start or '_': {}", s)
            ));
        }
        // 后续字符必须是 XID_Continue
        for c in chars {
            if !is_xid_continue(c) {
                return Err(A2uiError::InvalidComponentId(
                    format!("ID contains invalid character '{}': {}", c, s)
                ));
            }
        }
        Ok(Self(s.to_string()))
    }

    /// 获取内部字符串
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ComponentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for ComponentId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Unicode XID_Start 检查（简化版，覆盖 ASCII + 常见 Unicode）
fn is_xid_start(c: char) -> bool {
    c == '_' || c.is_ascii_alphabetic() || (c as u32) > 0x7F
}

/// Unicode XID_Continue 检查
fn is_xid_continue(c: char) -> bool {
    is_xid_start(c) || c.is_ascii_alphanumeric() || (c as u32) > 0x7F
}

// ---- DynamicValue ----

/// 动态值类型：支持字面量、路径绑定、函数调用三种形式
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DynamicValue<T = Value> {
    /// 字面量值
    Literal(T),
    /// 绑定到 Data Model 的路径
    #[serde(rename = "path")]
    Path { path: String },
    /// 调用注册函数
    #[serde(rename = "call")]
    FunctionCall { call: String, args: Value },
}

impl DynamicValue<String> {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            DynamicValue::Literal(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_path(&self) -> Option<&str> {
        match self {
            DynamicValue::Path { path } => Some(path.as_str()),
            _ => None,
        }
    }

    pub fn as_function_call(&self) -> Option<&str> {
        match self {
            DynamicValue::FunctionCall { call, .. } => Some(call.as_str()),
            _ => None,
        }
    }
}

impl DynamicValue<i64> {
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            DynamicValue::Literal(n) => Some(*n),
            _ => None,
        }
    }
}

impl DynamicValue<bool> {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            DynamicValue::Literal(b) => Some(*b),
            _ => None,
        }
    }
}

// ---- ComponentCommon ----

/// 组件通用属性（混入所有组件）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentCommon {
    /// 组件唯一标识符
    pub id: ComponentId,
    /// 无障碍属性
    pub accessibility: Option<AccessibilityAttributes>,
    /// 权重（类似 CSS flex-grow，仅在 Row/Column 直接子组件时有效）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<f64>,
}

/// 无障碍属性
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccessibilityAttributes {
    /// 无障碍标签
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// 详细描述
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_component_id_valid() {
        let id = ComponentId::new("my_button").unwrap();
        assert_eq!(id.as_str(), "my_button");
    }

    #[test]
    fn test_component_id_invalid_starts_with_number() {
        assert!(ComponentId::new("123abc").is_err());
    }

    #[test]
    fn test_component_id_invalid_contains_space() {
        assert!(ComponentId::new("my button").is_err());
    }

    #[test]
    fn test_component_id_at_namespace_reserved() {
        assert!(ComponentId::new("@custom").is_err());
    }

    #[test]
    fn test_component_id_empty() {
        assert!(ComponentId::new("").is_err());
    }

    #[test]
    fn test_component_id_display() {
        let id = ComponentId::new("root").unwrap();
        assert_eq!(format!("{}", id), "root");
    }

    #[test]
    fn test_dynamic_value_literal_string() {
        let dv: DynamicValue<String> = DynamicValue::Literal("hello".into());
        assert_eq!(dv.as_str(), Some("hello"));
    }

    #[test]
    fn test_dynamic_value_literal_number() {
        let dv: DynamicValue<i64> = DynamicValue::Literal(42);
        assert_eq!(dv.as_i64(), Some(42));
    }

    #[test]
    fn test_dynamic_value_literal_bool() {
        let dv: DynamicValue<bool> = DynamicValue::Literal(true);
        assert_eq!(dv.as_bool(), Some(true));
    }

    #[test]
    fn test_dynamic_value_path() {
        let dv: DynamicValue<String> = DynamicValue::Path { path: "/user/name".into() };
        assert_eq!(dv.as_path(), Some("/user/name"));
    }

    #[test]
    fn test_dynamic_value_function_call() {
        let dv: DynamicValue<String> = DynamicValue::FunctionCall {
            call: "formatString".into(),
            args: json!({"template": "Hello {name}"}),
        };
        assert_eq!(dv.as_function_call(), Some("formatString"));
    }

    #[test]
    fn test_component_common_fields() {
        let common = ComponentCommon {
            id: ComponentId::new("root").unwrap(),
            accessibility: None,
            weight: None,
        };
        assert_eq!(common.id.as_str(), "root");
    }

    #[test]
    fn test_accessibility_attributes() {
        let acc = AccessibilityAttributes {
            label: Some("Submit".into()),
            description: Some("Submits form".into()),
        };
        let common = ComponentCommon {
            id: ComponentId::new("btn").unwrap(),
            accessibility: Some(acc),
            weight: Some(1.0),
        };
        assert!(common.accessibility.is_some());
        assert_eq!(common.weight, Some(1.0));
    }
}
