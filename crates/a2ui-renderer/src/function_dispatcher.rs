use crate::error::RenderResult;
use serde_json::Value;
use std::collections::HashMap;

/// 函数执行边界
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallableFrom {
    /// 只能在客户端执行
    ClientOnly,
    /// 只能在服务端执行
    RemoteOnly,
    /// 两端均可执行
    ClientOrRemote,
}

/// 函数定义
#[derive(Debug, Clone)]
pub struct FunctionDef {
    pub name: String,
    pub callable_from: CallableFrom,
}

/// 函数调度器
#[derive(Debug, Clone, Default)]
pub struct FunctionDispatcher {
    /// 已注册的函数
    functions: HashMap<String, FunctionDef>,
}

impl FunctionDispatcher {
    /// 创建新的调度器
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册函数
    pub fn register(&mut self, name: impl Into<String>, callable_from: CallableFrom) {
        let name = name.into();
        self.functions.insert(
            name.clone(),
            FunctionDef {
                name: name.clone(),
                callable_from,
            },
        );
    }

    /// 执行函数调用
    pub fn dispatch(&self, name: &str, args: Value) -> RenderResult<Value> {
        let func = self
            .functions
            .get(name)
            .ok_or_else(|| crate::error::RendererError::FunctionNotAvailable(name.to_string()))?;
        // 简化实现：返回空值，实际由平台实现
        let _ = (func, args);
        Ok(Value::Null)
    }

    /// 检查函数是否可以从指定端调用
    pub fn can_call_from(&self, name: &str, from: CallableFrom) -> bool {
        self.functions.get(name).is_some_and(|f| {
            f.callable_from == from || f.callable_from == CallableFrom::ClientOrRemote
        })
    }

    /// 获取函数定义
    pub fn get(&self, name: &str) -> Option<&FunctionDef> {
        self.functions.get(name)
    }

    /// 获取所有已注册函数名
    pub fn registered_names(&self) -> Vec<&String> {
        self.functions.keys().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_register_and_dispatch() {
        let mut dispatcher = FunctionDispatcher::new();
        dispatcher.register("add", CallableFrom::ClientOrRemote);
        assert!(dispatcher.can_call_from("add", CallableFrom::ClientOnly));
        assert!(dispatcher.can_call_from("add", CallableFrom::RemoteOnly));
        assert!(dispatcher.can_call_from("add", CallableFrom::ClientOrRemote));
    }

    #[test]
    fn test_client_only_cannot_be_called_remote() {
        let mut dispatcher = FunctionDispatcher::new();
        dispatcher.register("validate", CallableFrom::ClientOnly);
        assert!(dispatcher.can_call_from("validate", CallableFrom::ClientOnly));
        assert!(!dispatcher.can_call_from("validate", CallableFrom::RemoteOnly));
    }

    #[test]
    fn test_remote_only_cannot_be_called_client() {
        let mut dispatcher = FunctionDispatcher::new();
        dispatcher.register("fetch", CallableFrom::RemoteOnly);
        assert!(!dispatcher.can_call_from("fetch", CallableFrom::ClientOnly));
        assert!(dispatcher.can_call_from("fetch", CallableFrom::RemoteOnly));
    }

    #[test]
    fn test_dispatch_unknown_function() {
        let dispatcher = FunctionDispatcher::new();
        let result: RenderResult<Value> = dispatcher.dispatch("unknown", json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn test_get_undefined() {
        let dispatcher = FunctionDispatcher::new();
        assert!(dispatcher.get("unknown").is_none());
    }

    #[test]
    fn test_registered_names() {
        let mut dispatcher = FunctionDispatcher::new();
        dispatcher.register("f1", CallableFrom::ClientOnly);
        dispatcher.register("f2", CallableFrom::RemoteOnly);
        let names = dispatcher.registered_names();
        assert_eq!(names.len(), 2);
    }
}
