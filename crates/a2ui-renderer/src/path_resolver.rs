use crate::error::RenderResult;
use a2ui_core::prelude::*;
use serde_json::Value;

/// 路径解析引擎
/// 支持绝对路径、相对路径（集合作用域）和 DynamicValue 解析
#[derive(Debug, Clone)]
pub struct PathResolver {
    data_model: DataModel,
    /// 集合作用域栈（用于解析相对路径）
    scope_stack: Vec<Scope>,
}

/// 作用域类型
#[derive(Debug, Clone)]
pub enum Scope {
    /// 根作用域
    Root,
    /// 集合作用域（数组迭代中的当前项）
    Collection { base_path: String, index: usize },
}

impl PathResolver {
    /// 从 DataModel 创建
    pub fn new(data_model: DataModel) -> Self {
        Self {
            data_model,
            scope_stack: vec![Scope::Root],
        }
    }

    /// 解析路径（支持绝对和相对路径）
    pub fn resolve(&self, path: &str) -> Option<Value> {
        if path.starts_with('/') {
            // 绝对路径
            self.data_model.get(path).cloned()
        } else if path == "@index" {
            // 系统保留变量
            self.current_index().map(|i| json!(i))
        } else {
            // 相对路径：在当前作用域下解析
            self.resolve_relative(path)
        }
    }

    /// 解析相对路径
    pub fn resolve_relative(&self, relative: &str) -> Option<Value> {
        let base = self.current_base_path();
        if base.is_empty() {
            self.data_model.get(&format!("/{}", relative)).cloned()
        } else {
            let idx = self.current_index().unwrap_or(0);
            let full = format!("{}/{}/{}", base, idx, relative);
            self.data_model.get(&full).cloned()
        }
    }

    /// 将相对路径转换为绝对路径
    pub fn resolve_to_absolute(&self, base: &str, relative: &str, index: usize) -> String {
        if relative.starts_with('/') {
            relative.to_string()
        } else {
            format!("{}/{}/{}", base, index, relative)
        }
    }

    /// 解析 DynamicValue
    pub fn resolve_dynamic<T>(&self, dynamic: &DynamicValue<T>) -> RenderResult<Value>
    where
        T: Clone + Into<Value>,
    {
        match dynamic {
            DynamicValue::Literal(v) => Ok(v.clone().into()),
            DynamicValue::Path { path } => self.resolve(path).ok_or_else(|| {
                crate::error::RendererError::SurfaceNotFound(format!("path not found: {}", path))
            }),
            DynamicValue::FunctionCall { call, .. } => Err(
                crate::error::RendererError::FunctionNotAvailable(call.clone()),
            ),
        }
    }

    /// 获取当前作用域的 JSON Pointer 基础路径
    fn current_base_path(&self) -> String {
        if let Some(scope) = self.scope_stack.iter().next_back() {
            match scope {
                Scope::Collection { base_path, .. } => return base_path.clone(),
                Scope::Root => {}
            }
        }
        String::new()
    }

    /// 获取当前索引
    fn current_index(&self) -> Option<usize> {
        if let Some(scope) = self.scope_stack.iter().next_back() {
            match scope {
                Scope::Collection { index, .. } => return Some(*index),
                Scope::Root => {}
            }
        }
        None
    }

    /// 进入集合作用域
    pub fn enter_collection(&mut self, base_path: impl Into<String>, index: usize) {
        self.scope_stack.push(Scope::Collection {
            base_path: base_path.into(),
            index,
        });
    }

    /// 退出集合作用域
    pub fn exit_collection(&mut self) {
        if self.scope_stack.len() > 1 {
            self.scope_stack.pop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_resolve_absolute_path() {
        let dm = DataModel::new(json!({"user": {"name": "Alice"}}));
        let resolver = PathResolver::new(dm);
        assert_eq!(resolver.resolve("/user/name"), Some(json!("Alice")));
    }

    #[test]
    fn test_resolve_nonexistent() {
        let dm = DataModel::new(json!({}));
        let resolver = PathResolver::new(dm);
        assert!(resolver.resolve("/missing").is_none());
    }

    #[test]
    fn test_resolve_relative_in_root() {
        let dm = DataModel::new(json!({"name": "Alice"}));
        let resolver = PathResolver::new(dm);
        assert_eq!(resolver.resolve_relative("name"), Some(json!("Alice")));
    }

    #[test]
    fn test_resolve_relative_in_collection() {
        let dm = DataModel::new(json!({"items": [{"name": "a"}, {"name": "b"}]}));
        let mut resolver = PathResolver::new(dm);
        resolver.enter_collection("/items", 1);
        assert_eq!(resolver.resolve_relative("name"), Some(json!("b")));
    }

    #[test]
    fn test_resolve_to_absolute() {
        let resolver = PathResolver::new(DataModel::empty());
        assert_eq!(
            resolver.resolve_to_absolute("/items", "name", 0),
            "/items/0/name"
        );
    }

    #[test]
    fn test_resolve_dynamic_literal() {
        let resolver = PathResolver::new(DataModel::empty());
        let dv: DynamicValue<String> = DynamicValue::Literal("hello".into());
        assert_eq!(resolver.resolve_dynamic(&dv).unwrap(), "hello");
    }

    #[test]
    fn test_resolve_dynamic_path() {
        let dm = DataModel::new(json!({"x": 42}));
        let resolver = PathResolver::new(dm);
        let dv: DynamicValue<i64> = DynamicValue::Path { path: "/x".into() };
        assert_eq!(resolver.resolve_dynamic(&dv).unwrap(), 42);
    }
}
