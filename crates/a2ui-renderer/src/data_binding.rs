use crate::error::RenderResult;
use a2ui_core::prelude::*;
use serde_json::Value;

/// Data Binding：封装 DataModel，提供路径读写和 DynamicValue 解析
#[derive(Debug, Clone)]
pub struct DataBinding {
    data_model: DataModel,
}

impl DataBinding {
    /// 从 DataModel 创建
    pub fn new(data_model: DataModel) -> Self {
        Self { data_model }
    }

    /// 获取 JSON Pointer 路径的值（安全版本，拒绝恶意路径）
    pub fn get(&self, path: &str) -> Option<&Value> {
        self.data_model.get_safe(path).ok().flatten()
    }

    /// 设置 JSON Pointer 路径的值
    pub fn set(&mut self, path: &str, value: Value) -> RenderResult<()> {
        self.data_model.apply_pointer(path, Some(value))?;
        Ok(())
    }

    /// 解析 DynamicValue 为具体值
    pub fn resolve_dynamic<T>(&self, dynamic: &DynamicValue<T>) -> RenderResult<Value>
    where
        T: Into<Value> + Clone,
    {
        match dynamic {
            DynamicValue::Literal(v) => Ok(v.clone().into()),
            DynamicValue::Path { path } => self.data_model.get_safe(path).ok().flatten().cloned().ok_or_else(|| {
                crate::error::RendererError::BindingError(format!("path not found: {}", path))
            }),
            DynamicValue::FunctionCall { call, .. } => Err(
                crate::error::RendererError::FunctionNotAvailable(call.clone()),
            ),
        }
    }

    /// 获取底层 DataModel
    pub fn as_value(&self) -> &Value {
        self.data_model.as_value()
    }

    /// 获取底层 DataModel 可变引用
    pub fn as_value_mut(&mut self) -> &mut Value {
        self.data_model.as_value_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_new_databinding() {
        let dm = DataModel::new(json!({"name": "Alice"}));
        let binding = DataBinding::new(dm);
        assert_eq!(binding.get("/name"), Some(&json!("Alice")));
    }

    #[test]
    fn test_set_value() {
        let mut binding = DataBinding::new(DataModel::empty());
        binding.set("/name", json!("Alice")).unwrap();
        assert_eq!(binding.get("/name"), Some(&json!("Alice")));
    }

    #[test]
    fn test_update_existing_value() {
        let mut binding = DataBinding::new(DataModel::new(json!({"name": "Alice"})));
        binding.set("/name", json!("Bob")).unwrap();
        assert_eq!(binding.get("/name"), Some(&json!("Bob")));
    }

    #[test]
    fn test_resolve_literal() {
        let binding = DataBinding::new(DataModel::empty());
        let dv: DynamicValue<String> = DynamicValue::Literal("hello".into());
        assert_eq!(binding.resolve_dynamic(&dv).unwrap(), "hello");
    }

    #[test]
    fn test_resolve_path() {
        let dm = DataModel::new(json!({"user": {"name": "Alice"}}));
        let binding = DataBinding::new(dm);
        let dv: DynamicValue<String> = DynamicValue::Path {
            path: "/user/name".into(),
        };
        assert_eq!(binding.resolve_dynamic(&dv).unwrap(), "Alice");
    }

    #[test]
    fn test_resolve_function_call_not_supported() {
        let binding = DataBinding::new(DataModel::empty());
        let dv: DynamicValue<String> = DynamicValue::FunctionCall {
            call: "unknown".into(),
            args: json!({}),
        };
        assert!(binding.resolve_dynamic(&dv).is_err());
    }

    #[test]
    fn test_as_value() {
        let dm = DataModel::new(json!({"x": 1}));
        let binding = DataBinding::new(dm);
        assert_eq!(binding.as_value(), &json!({"x": 1}));
    }
}
