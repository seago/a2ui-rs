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

    /// 获取可变引用
    pub fn get_mut(&mut self, pointer: &str) -> Option<&mut Value> {
        self.value.pointer_mut(pointer)
    }

    /// 应用 JSON Pointer 路径更新（upsert 语义）
    ///
    /// - `value: Some(v)` → 存在则更新，不存在则创建
    /// - `value: None` → 删除路径对应的 key
    /// - `pointer` 为 "/" 或空 → 替换整个 data model
    pub fn apply_pointer(&mut self, pointer: &str, value: Option<Value>) {
        if pointer.is_empty() || pointer == "/" {
            if let Some(v) = value {
                self.value = v;
            } else {
                self.value = Value::Object(Default::default());
            }
            return;
        }

        // 处理删除
        if value.is_none() {
            self._delete_at_pointer(pointer);
            return;
        }

        let new_value = value.unwrap();
        if let Some(target) = self.value.pointer_mut(pointer) {
            *target = new_value;
        } else {
            // 需要创建中间节点
            self._create_path(pointer, new_value);
        }
    }

    /// 删除 JSON Pointer 路径的值
    pub fn delete_pointer(&mut self, pointer: &str) {
        self.apply_pointer(pointer, None);
    }

    /// 获取内部 Value 的引用
    pub fn as_value(&self) -> &Value {
        &self.value
    }

    /// 获取内部 Value 的可变引用
    pub fn as_value_mut(&mut self) -> &mut Value {
        &mut self.value
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
        dm.apply_pointer("/name", Some(json!("Alice")));
        assert_eq!(dm.get("/name"), Some(&json!("Alice")));
    }

    #[test]
    fn test_apply_pointer_update() {
        let mut dm = DataModel::new(json!({"name": "Alice"}));
        dm.apply_pointer("/name", Some(json!("Bob")));
        assert_eq!(dm.get("/name"), Some(&json!("Bob")));
    }

    #[test]
    fn test_apply_pointer_delete() {
        let mut dm = DataModel::new(json!({"name": "Alice"}));
        dm.apply_pointer("/name", None);
        assert_eq!(dm.get("/name"), None);
    }

    #[test]
    fn test_apply_pointer_nested_create() {
        let mut dm = DataModel::new(json!({}));
        dm.apply_pointer("/user/name", Some(json!("Alice")));
        assert_eq!(dm.get("/user/name"), Some(&json!("Alice")));
    }

    #[test]
    fn test_apply_pointer_replace_root() {
        let mut dm = DataModel::new(json!({"old": true}));
        dm.apply_pointer("/", Some(json!({"new": true})));
        assert_eq!(dm.get("/new"), Some(&json!(true)));
        assert_eq!(dm.get("/old"), None);
    }

    #[test]
    fn test_apply_pointer_delete_root() {
        let mut dm = DataModel::new(json!({"a": 1}));
        dm.apply_pointer("/", None);
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
}
