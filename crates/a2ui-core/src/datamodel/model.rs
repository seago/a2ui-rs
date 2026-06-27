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

    /// 获取可变引用（包含路径安全检查）
    pub fn get_mut(&mut self, pointer: &str) -> Result<Option<&mut Value>> {
        self.validate_pointer(pointer)?;
        Ok(self.value.pointer_mut(pointer))
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
        let Some(new_value) = value else {
            self._delete_at_pointer(pointer);
            return Ok(());
        };
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

    /// 判断路径段是否为数组索引（纯数字）
    fn is_array_index(segment: &str) -> Option<usize> {
        // 拒绝空字符串和含有前导零的多位数字（如 "01"）
        if segment.is_empty() {
            return None;
        }
        if segment.len() > 1 && segment.starts_with('0') {
            return None;
        }
        segment.parse::<usize>().ok()
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
                } else if let Value::Array(ref mut arr) = self.value {
                    if let Some(idx) = Self::is_array_index(&unescaped) {
                        if idx < arr.len() {
                            arr.remove(idx);
                        }
                    }
                }
            } else {
                // 导航到父对象
                let mut current = &mut self.value;
                for segment in &segments[..segments.len() - 1] {
                    // 使用 borrow checker 友好的方式
                    let child = match current {
                        Value::Object(ref mut map) => {
                            let key = segment.replace("~1", "/").replace("~0", "~");
                            map.get_mut(&key)
                        }
                        Value::Array(ref mut arr) => {
                            let key = segment.replace("~1", "/").replace("~0", "~");
                            if let Some(idx) = Self::is_array_index(&key) {
                                arr.get_mut(idx)
                            } else {
                                None
                            }
                        }
                        _ => None,
                    };
                    match child {
                        Some(val) => current = val,
                        None => return, // 父路径不存在
                    }
                }
                match current {
                    Value::Object(ref mut map) => {
                        map.remove(&unescaped);
                    }
                    Value::Array(ref mut arr) => {
                        if let Some(idx) = Self::is_array_index(&unescaped) {
                            if idx < arr.len() {
                                arr.remove(idx);
                            }
                        }
                    }
                    _ => {}
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
                match current {
                    Value::Object(ref mut map) => {
                        map.insert(unescaped, value.clone());
                    }
                    Value::Array(ref mut arr) => {
                        if let Some(idx) = Self::is_array_index(&unescaped) {
                            if idx < arr.len() {
                                arr[idx] = value.clone();
                            }
                        }
                    }
                    _ => return,
                }
            } else {
                // 中间段：确保路径存在
                match current {
                    Value::Object(ref mut map) => {
                        if !map.contains_key(&unescaped) {
                            // 检查下一段是否为数组索引来决定创建 Object 还是 Array
                            let next_is_array = segments
                                .get(i + 1)
                                .and_then(|s| Self::is_array_index(&s.replace("~1", "/").replace("~0", "~")))
                                .is_some();
                            let child = if next_is_array {
                                Value::Array(vec![])
                            } else {
                                Value::Object(Default::default())
                            };
                            map.insert(unescaped.clone(), child);
                        }
                        // 安全地获取子节点引用
                        if let Some(child) = map.get_mut(&unescaped) {
                            current = child;
                        } else {
                            return;
                        }
                    }
                    Value::Array(ref mut arr) => {
                        if let Some(idx) = Self::is_array_index(&unescaped) {
                            if let Some(child) = arr.get_mut(idx) {
                                current = child;
                            } else {
                                return;
                            }
                        } else {
                            return;
                        }
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

    // --- Array index support tests ---

    #[test]
    fn test_apply_pointer_through_array_index() {
        let mut dm = DataModel::new(json!({"arr": [{"name": "Alice"}, {"name": "Bob"}]}));
        dm.apply_pointer("/arr/0/name", Some(json!("Charlie")))
            .unwrap();
        assert_eq!(dm.get("/arr/0/name"), Some(&json!("Charlie")));
        // Bob 不受影响
        assert_eq!(dm.get("/arr/1/name"), Some(&json!("Bob")));
    }

    #[test]
    fn test_create_path_with_array_index() {
        let mut dm = DataModel::new(json!({"arr": [{"id": 1}, {"id": 2}]}));
        dm.apply_pointer("/arr/0/name", Some(json!("first")))
            .unwrap();
        assert_eq!(dm.get("/arr/0/name"), Some(&json!("first")));
        assert_eq!(dm.get("/arr/0/id"), Some(&json!(1)));
    }

    #[test]
    fn test_delete_at_array_index() {
        let mut dm = DataModel::new(json!({"arr": ["a", "b", "c"]}));
        dm.apply_pointer("/arr/1", None).unwrap();
        assert_eq!(dm.get("/arr/0"), Some(&json!("a")));
        assert_eq!(dm.get("/arr/1"), Some(&json!("c"))); // b 被删除，c 前移
    }
}
