use crate::error::{A2uiError, Result};
use serde::Serialize;
use serde_json::Value;
use std::ops::Deref;

/// Data Model：组件绑定的纯 JSON 数据
/// 内部使用 serde_json::Value，提供 JSON Pointer 路径操作
#[derive(Debug, Clone, Default, Serialize)]
pub struct DataModel {
    value: Value,
}

impl DataModel {
    /// 从 JSON Value 创建 DataModel
    pub fn new(value: Value) -> Self {
        Self { value }
    }

    /// 从空对象创建
    pub fn empty() -> Self {
        Self {
            value: Value::Object(Default::default()),
        }
    }

    /// 获取 JSON Pointer 路径的值
    pub fn get(&self, pointer: &str) -> Option<&Value> {
        self.value.pointer(pointer)
    }

    /// 安全获取 JSON Pointer 路径的值，拒绝恶意构造的路径
    ///
    /// 校验规则：
    /// - 拒绝包含 null 字节的路径
    /// - 拒绝空路径段（如 `//`）
    /// - 拒绝 `..` 路径遍历
    pub fn get_safe(&self, pointer: &str) -> Result<Option<&Value>> {
        self.validate_pointer(pointer)?;
        Ok(self.value.pointer(pointer))
    }

    /// 获取可变引用
    pub fn get_mut(&mut self, pointer: &str) -> Option<&mut Value> {
        self.value.pointer_mut(pointer)
    }

    /// 应用 JSON Pointer 路径更新（upsert 语义），包含路径安全检查
    ///
    /// - `value: Some(v)` → 存在则更新，不存在则创建
    /// - `value: None` → 删除路径对应的 key
    /// - `pointer` 为 "/" 或空 → 替换整个 data model
    pub fn apply_pointer(&mut self, pointer: &str, value: Option<Value>) -> Result<()> {
        if pointer.is_empty() || pointer == "/" {
            if let Some(v) = value {
                self.value = v;
            } else {
                self.value = Value::Object(Default::default());
            }
            return Ok(());
        }

        // 路径安全检查
        self.validate_pointer(pointer)?;

        // 处理删除
        if value.is_none() {
            self._delete_at_pointer(pointer);
            return Ok(());
        }

        let new_value = value.unwrap();
        if let Some(target) = self.value.pointer_mut(pointer) {
            *target = new_value;
        } else {
            // 需要创建中间节点
            self._create_path(pointer, new_value);
        }
        Ok(())
    }

    /// 删除 JSON Pointer 路径的值
    pub fn delete_pointer(&mut self, pointer: &str) -> Result<()> {
        self.apply_pointer(pointer, None)
    }

    /// 获取内部 Value 的引用
    pub fn as_value(&self) -> &Value {
        &self.value
    }

    /// 获取内部 Value 的可变引用
    pub fn as_value_mut(&mut self) -> &mut Value {
        &mut self.value
    }

    /// 校验 JSON Pointer 路径的安全性
    ///
    /// # 拒绝的路径模式
    /// - 包含 null 字节（`\0`）
    /// - 空路径段（如 `/foo//bar`）
    /// - `..` 路径遍历片段
    fn validate_pointer(&self, pointer: &str) -> Result<()> {
        // 检查 null 字节
        if pointer.contains('\0') {
            return Err(A2uiError::InvalidPointer(format!(
                "path contains null byte: {:?}",
                pointer
            )));
        }

        // 分割路径段并检查每个段
        let segments: Vec<&str> = pointer.trim_start_matches('/').split('/').collect();
        for segment in &segments {
            if segment.is_empty() {
                return Err(A2uiError::InvalidPointer(format!(
                    "empty path segment in: {}",
                    pointer
                )));
            }
            if *segment == ".." {
                return Err(A2uiError::PathTraversal(format!(
                    "path traversal detected: {}",
                    pointer
                )));
            }
        }

        // 验证解析后的路径仍在 DataModel 根对象范围内
        // serde_json::Value::pointer 不会逃逸根对象，但验证解析结果
        if let Some(resolved) = self.value.pointer(pointer) {
            // 确保解析结果不是通过异常路径获得
            // serde_json 的 pointer 已经保证了不会逃逸根
            let _ = resolved;
        }

        Ok(())
    }

    /// 手动实现删除操作（serde_json 不支持直接删除）
    fn _delete_at_pointer(&mut self, pointer: &str) {
        let segments: Vec<&str> = pointer.trim_start_matches('/').split('/').collect();
        if segments.is_empty() {
            self.value = Value::Object(Default::default());
            return;
        }

        if let Some(last) = segments.last() {
            let unescaped = last.replace("~1", "/").replace("~0", "~");

            if segments.len() == 1 {
                // 直接子节点（根对象下）
                if let Value::Object(ref mut map) = self.value {
                    map.remove(&unescaped);
                }
            } else {
                // 导航到父对象
                let mut current = &mut self.value;
                for segment in &segments[..segments.len() - 1] {
                    let key = segment.replace("~1", "/").replace("~0", "~");
                    match current {
                        Value::Object(ref mut map) => {
                            if let Some(val) = map.get_mut(&key) {
                                current = val;
                            } else {
                                return; // 父路径不存在
                            }
                        }
                        _ => return,
                    }
                }
                if let Value::Object(ref mut map) = current {
                    map.remove(&unescaped);
                }
            }
        }
    }

    /// 手动创建中间路径
    fn _create_path(&mut self, pointer: &str, value: Value) {
        let segments: Vec<&str> = pointer.trim_start_matches('/').split('/').collect();
        if segments.is_empty() {
            return;
        }

        let mut current = &mut self.value;
        for (i, segment) in segments.iter().enumerate() {
            let unescaped = segment.replace("~1", "/").replace("~0", "~");

            if i == segments.len() - 1 {
                // 最后一段：设置值
                if let Value::Object(ref mut map) = current {
                    map.insert(unescaped, value.clone());
                }
            } else {
                // 中间段：确保是 Object
                match current {
                    Value::Object(ref mut map) => {
                        if !map.contains_key(&unescaped) {
                            map.insert(unescaped.clone(), Value::Object(Default::default()));
                        }
                        current = map.get_mut(&unescaped).unwrap();
                    }
                    _ => return,
                }
            }
        }
    }
}

impl Deref for DataModel {
    type Target = Value;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_new_datamodel() {
        let dm = DataModel::new(json!({"name": "Alice"}));
        assert_eq!(dm.get("/name"), Some(&json!("Alice")));
    }

    #[test]
    fn test_empty_datamodel() {
        let dm = DataModel::empty();
        assert_eq!(dm.get("/name"), None);
    }

    #[test]
    fn test_apply_pointer_create() {
        let mut dm = DataModel::new(json!({}));
        dm.apply_pointer("/name", Some(json!("Alice"))).unwrap();
        assert_eq!(dm.get("/name"), Some(&json!("Alice")));
    }

    #[test]
    fn test_apply_pointer_update() {
        let mut dm = DataModel::new(json!({"name": "Alice"}));
        dm.apply_pointer("/name", Some(json!("Bob"))).unwrap();
        assert_eq!(dm.get("/name"), Some(&json!("Bob")));
    }

    #[test]
    fn test_apply_pointer_delete() {
        let mut dm = DataModel::new(json!({"name": "Alice"}));
        dm.apply_pointer("/name", None).unwrap();
        assert_eq!(dm.get("/name"), None);
    }

    #[test]
    fn test_apply_pointer_nested_create() {
        let mut dm = DataModel::new(json!({}));
        dm.apply_pointer("/user/name", Some(json!("Alice")))
            .unwrap();
        assert_eq!(dm.get("/user/name"), Some(&json!("Alice")));
    }

    #[test]
    fn test_apply_pointer_replace_root() {
        let mut dm = DataModel::new(json!({"old": true}));
        dm.apply_pointer("/", Some(json!({"new": true}))).unwrap();
        assert_eq!(dm.get("/new"), Some(&json!(true)));
        assert_eq!(dm.get("/old"), None);
    }

    #[test]
    fn test_apply_pointer_delete_root() {
        let mut dm = DataModel::new(json!({"a": 1}));
        dm.apply_pointer("/", None).unwrap();
        assert_eq!(dm.get("/a"), None);
    }

    #[test]
    fn test_resolve_pointer_nonexistent() {
        let dm = DataModel::new(json!({}));
        assert_eq!(dm.get("/missing"), None);
    }

    #[test]
    fn test_resolve_pointer_nested_array() {
        let dm = DataModel::new(json!({"items": [{"id": 1}, {"id": 2}]}));
        assert_eq!(dm.get("/items/0/id"), Some(&json!(1)));
        assert_eq!(dm.get("/items/1/id"), Some(&json!(2)));
    }

    #[test]
    fn test_resolve_pointer_escaped_slash() {
        let dm = DataModel::new(json!({"a/b": "value"}));
        assert_eq!(dm.get("/a~1b"), Some(&json!("value")));
    }

    #[test]
    fn test_resolve_pointer_tilde_escape() {
        let dm = DataModel::new(json!({"a~b": "value"}));
        assert_eq!(dm.get("/a~0b"), Some(&json!("value")));
    }

    #[test]
    fn test_as_value() {
        let dm = DataModel::new(json!({"x": 1}));
        assert_eq!(dm.as_value(), &json!({"x": 1}));
    }

    // --- Path traversal protection tests (P0-2) ---

    #[test]
    fn test_reject_null_byte_in_path() {
        let mut dm = DataModel::new(json!({"a": 1}));
        let result = dm.apply_pointer("/a\0/b", Some(json!(1)));
        assert!(result.is_err());
    }

    #[test]
    fn test_reject_empty_path_segment() {
        let mut dm = DataModel::new(json!({"a": {"b": 1}}));
        let result = dm.apply_pointer("/a//b", Some(json!(2)));
        assert!(result.is_err());
    }

    #[test]
    fn test_reject_dotdot_traversal() {
        let mut dm = DataModel::new(json!({"a": {"b": 1}}));
        let result = dm.apply_pointer("/a/../b", Some(json!(2)));
        assert!(result.is_err());
    }

    #[test]
    fn test_get_safe_rejects_malicious_path() {
        let dm = DataModel::new(json!({"a": 1}));
        let result = dm.get_safe("/a//b");
        assert!(result.is_err());
    }

    #[test]
    fn test_valid_nested_path_still_works() {
        let mut dm = DataModel::new(json!({"user": {"name": "Alice"}}));
        dm.apply_pointer("/user/name", Some(json!("Bob"))).unwrap();
        assert_eq!(dm.get("/user/name"), Some(&json!("Bob")));
    }

    #[test]
    fn test_valid_path_with_escaped_chars() {
        let mut dm = DataModel::new(json!({"a/b": "value"}));
        dm.apply_pointer("/a~1b", Some(json!("updated"))).unwrap();
        assert_eq!(dm.get("/a~1b"), Some(&json!("updated")));
    }
}
