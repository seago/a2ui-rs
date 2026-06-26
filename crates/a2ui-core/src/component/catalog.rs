use crate::error::{A2uiError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// A2UI Catalog：声明可用组件类型和函数定义
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    /// Catalog 唯一标识符（建议用域名前缀的 URI）
    catalog_id: String,
    /// Markdown 格式的设计原则说明
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<String>,
    /// 组件类型定义（组件名 → JSON Schema）
    #[serde(default)]
    components: HashMap<String, Value>,
    /// 函数定义（函数名 → JSON Schema）
    #[serde(default)]
    functions: HashMap<String, Value>,
    /// 内联 defs（仅允许 surfaceProperties / anyComponent / anyFunction）
    #[serde(rename = "$defs", default, skip_serializing_if = "HashMap::is_empty")]
    defs: HashMap<String, Value>,
}

impl Catalog {
    /// 创建最小 Catalog
    pub fn new(catalog_id: impl Into<String>) -> Self {
        Self {
            catalog_id: catalog_id.into(),
            instructions: None,
            components: HashMap::new(),
            functions: HashMap::new(),
            defs: HashMap::new(),
        }
    }

    /// 设置设计原则说明
    pub fn with_instructions(mut self, instructions: impl Into<String>) -> Self {
        self.instructions = Some(instructions.into());
        self
    }

    /// 添加组件定义
    pub fn add_component(&mut self, name: impl Into<String>, schema: Value) {
        self.components.insert(name.into(), schema);
    }

    /// 添加函数定义
    pub fn add_function(&mut self, name: impl Into<String>, schema: Value) {
        self.functions.insert(name.into(), schema);
    }

    /// 获取 Catalog ID
    pub fn catalog_id(&self) -> &str {
        &self.catalog_id
    }

    /// 获取所有组件定义
    pub fn components(&self) -> &HashMap<String, Value> {
        &self.components
    }

    /// 获取所有函数定义
    pub fn functions(&self) -> &HashMap<String, Value> {
        &self.functions
    }

    /// 检查组件是否存在
    pub fn has_component(&self, name: &str) -> bool {
        self.components.contains_key(name)
    }

    /// 获取组件 Schema
    pub fn get_component_schema(&self, name: &str) -> Option<&Value> {
        self.components.get(name)
    }

    /// 检查函数是否存在
    pub fn has_function(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }

    /// 获取函数 Schema
    pub fn get_function_schema(&self, name: &str) -> Option<&Value> {
        self.functions.get(name)
    }

    /// 获取函数的 callableFrom 值
    pub fn function_callable_from(&self, name: &str) -> Option<&str> {
        self.functions.get(name)?.get("callableFrom")?.as_str()
    }

    /// 校验 Catalog 结构合规性（v1.0 严格规则）
    pub fn validate(&self) -> Result<()> {
        // 检查 $defs 中只允许 surfaceProperties / anyComponent / anyFunction
        for key in self.defs.keys() {
            if !["surfaceProperties", "anyComponent", "anyFunction"].contains(&key.as_str()) {
                return Err(A2uiError::ValidationError {
                    message: format!("$defs contains disallowed key: {}", key),
                    component_id: "catalog".to_string(),
                    check_index: 0,
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_catalog_new() {
        let catalog = Catalog::new("https://example.com/basic".to_string());
        assert_eq!(catalog.catalog_id(), "https://example.com/basic");
        assert!(catalog.components().is_empty());
        assert!(catalog.functions().is_empty());
    }

    #[test]
    fn test_catalog_with_components() {
        let mut catalog = Catalog::new("basic".to_string());
        let text_schema = json!({
            "type": "object",
            "required": ["text"],
            "properties": {
                "text": { "type": "string" }
            }
        });
        catalog.add_component("Text".to_string(), text_schema.clone());
        assert!(catalog.has_component("Text"));
        assert!(catalog.get_component_schema("Text").is_some());
        assert!(catalog.get_component_schema("Button").is_none());
    }

    #[test]
    fn test_catalog_with_functions() {
        let mut catalog = Catalog::new("basic".to_string());
        let func_schema = json!({
            "type": "object",
            "returnType": "boolean",
            "callableFrom": "clientOnly",
            "properties": {
                "value": { "type": "string" }
            }
        });
        catalog.add_function("required".to_string(), func_schema);
        assert!(catalog.has_function("required"));
        assert!(!catalog.has_function("unknown"));
    }

    #[test]
    fn test_catalog_function_callable_from() {
        let mut catalog = Catalog::new("basic".to_string());
        let client_only = json!({
            "type": "object",
            "returnType": "boolean",
            "callableFrom": "clientOnly"
        });
        let remote_only = json!({
            "type": "object",
            "returnType": "string",
            "callableFrom": "remoteOnly"
        });
        catalog.add_function("validate".to_string(), client_only);
        catalog.add_function("fetch".to_string(), remote_only);

        assert_eq!(
            catalog.function_callable_from("validate"),
            Some("clientOnly")
        );
        assert_eq!(catalog.function_callable_from("fetch"), Some("remoteOnly"));
    }

    #[test]
    fn test_catalog_deserialize() {
        let json = r#"{
            "catalogId": "my-catalog",
            "instructions": "Test catalog",
            "components": {
                "Text": {"type": "object", "required": ["text"]}
            },
            "functions": {
                "required": {"type": "object", "returnType": "boolean", "callableFrom": "clientOnly"}
            }
        }"#;
        let catalog: Catalog = serde_json::from_str(json).unwrap();
        assert_eq!(catalog.catalog_id(), "my-catalog");
        assert!(catalog.has_component("Text"));
        assert!(catalog.has_function("required"));
    }

    #[test]
    fn test_catalog_validate_rejects_extra_defs() {
        let json = r#"{
            "catalogId": "test",
            "components": {},
            "functions": {},
            "$defs": {
                "surfaceProperties": {"type": "object"},
                "customSchema": {"type": "string"}
            }
        }"#;
        let catalog: Catalog = serde_json::from_str(json).unwrap();
        assert!(catalog.validate().is_err());
    }

    #[test]
    fn test_catalog_validate_accepts_valid_defs() {
        let json = r#"{
            "catalogId": "test",
            "components": {},
            "functions": {},
            "$defs": {
                "surfaceProperties": {"type": "object"},
                "anyComponent": {"oneOf": []},
                "anyFunction": {"oneOf": []}
            }
        }"#;
        let catalog: Catalog = serde_json::from_str(json).unwrap();
        assert!(catalog.validate().is_ok());
    }
}
