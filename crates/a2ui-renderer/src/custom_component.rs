use serde_json::Value;
use std::collections::HashMap;

/// 自定义组件定义
///
/// 描述一个超出 Basic Catalog 的自定义组件类型，包含类型名和可选的 JSON Schema。
#[derive(Debug, Clone)]
pub struct CustomComponentDef {
    /// 组件类型名（如 "MyChart"）
    pub name: String,
    /// 可选 JSON Schema 描述
    pub schema: Option<Value>,
}

impl CustomComponentDef {
    /// 创建新的自定义组件定义
    ///
    /// # 示例
    ///
    /// ```rust
    /// use a2ui_renderer::CustomComponentDef;
    ///
    /// let def = CustomComponentDef::new("MyChart");
    /// assert_eq!(def.name, "MyChart");
    /// assert!(def.schema.is_none());
    /// ```
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            schema: None,
        }
    }

    /// 设置组件定义的 JSON Schema
    ///
    /// # 示例
    ///
    /// ```rust
    /// use a2ui_renderer::CustomComponentDef;
    /// use serde_json::json;
    ///
    /// let def = CustomComponentDef::new("MyChart")
    ///     .with_schema(json!({"type": "object"}));
    /// assert!(def.schema.is_some());
    /// ```
    pub fn with_schema(mut self, schema: Value) -> Self {
        self.schema = Some(schema);
        self
    }
}

/// 自定义组件注册表
///
/// 管理 Basic Catalog 以外的自定义组件类型。
/// 渲染器遇到未知组件类型时查询此表，若已注册则识别为"自定义组件"而非"未知组件"。
///
/// # 示例
///
/// ```rust
/// use a2ui_renderer::{CustomComponentRegistry, CustomComponentDef};
///
/// let mut registry = CustomComponentRegistry::new();
/// registry.register(CustomComponentDef::new("MyChart")).unwrap();
/// assert!(registry.is_registered("MyChart"));
/// assert!(!registry.is_registered("Unknown"));
/// ```
#[derive(Debug, Clone, Default)]
pub struct CustomComponentRegistry {
    components: HashMap<String, CustomComponentDef>,
}

impl CustomComponentRegistry {
    /// 创建新的空注册表
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册自定义组件类型
    ///
    /// 如果同名组件已注册，返回错误信息。
    ///
    /// # 参数
    /// * `def` - 自定义组件定义
    ///
    /// # 错误
    ///
    /// 如果组件名已被注册，返回包含已存在组件名的错误字符串。
    pub fn register(&mut self, def: CustomComponentDef) -> Result<(), String> {
        let name = def.name.clone();
        if self.components.contains_key(&name) {
            return Err(format!("custom component already registered: {}", name));
        }
        self.components.insert(name, def);
        Ok(())
    }

    /// 检查组件类型是否已注册
    pub fn is_registered(&self, name: &str) -> bool {
        self.components.contains_key(name)
    }

    /// 获取组件定义
    pub fn get(&self, name: &str) -> Option<&CustomComponentDef> {
        self.components.get(name)
    }

    /// 返回所有已注册的组件名列表
    pub fn registered_names(&self) -> Vec<&String> {
        self.components.keys().collect()
    }

    /// 返回已注册的组件数量
    pub fn len(&self) -> usize {
        self.components.len()
    }

    /// 注册表是否为空
    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_registry_is_empty() {
        let reg = CustomComponentRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
    }

    #[test]
    fn test_register_and_check() {
        let mut reg = CustomComponentRegistry::new();
        assert!(reg.is_empty());

        reg.register(CustomComponentDef::new("MyChart")).unwrap();
        assert!(reg.is_registered("MyChart"));
        assert!(!reg.is_registered("Unknown"));
        assert_eq!(reg.len(), 1);
        assert!(!reg.is_empty());
    }

    #[test]
    fn test_duplicate_register_fails() {
        let mut reg = CustomComponentRegistry::new();
        reg.register(CustomComponentDef::new("MyChart")).unwrap();
        let result = reg.register(CustomComponentDef::new("MyChart"));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already registered"));
    }

    #[test]
    fn test_with_schema() {
        let def = CustomComponentDef::new("MyChart").with_schema(serde_json::json!({
            "type": "object",
            "properties": {
                "data": {"type": "array"}
            }
        }));
        let mut reg = CustomComponentRegistry::new();
        reg.register(def).unwrap();
        let retrieved = reg.get("MyChart").unwrap();
        assert!(retrieved.schema.is_some());
    }

    #[test]
    fn test_registered_names() {
        let mut reg = CustomComponentRegistry::new();
        reg.register(CustomComponentDef::new("Chart")).unwrap();
        reg.register(CustomComponentDef::new("Map")).unwrap();
        let names = reg.registered_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&&"Chart".to_string()));
        assert!(names.contains(&&"Map".to_string()));
    }

    #[test]
    fn test_get_returns_none_for_unregistered() {
        let reg = CustomComponentRegistry::new();
        assert!(reg.get("NonExistent").is_none());
    }

    #[test]
    fn test_clone() {
        let mut reg = CustomComponentRegistry::new();
        reg.register(CustomComponentDef::new("Chart")).unwrap();
        let cloned = reg.clone();
        assert!(cloned.is_registered("Chart"));
    }

    #[test]
    fn test_default_is_empty() {
        let reg: CustomComponentRegistry = Default::default();
        assert!(reg.is_empty());
    }
}
