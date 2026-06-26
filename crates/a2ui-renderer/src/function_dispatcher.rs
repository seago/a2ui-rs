use crate::error::RenderResult;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

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

/// 函数处理器类型别名
pub type FunctionHandler = Arc<dyn Fn(Value) -> RenderResult<Value> + Send + Sync>;

/// 函数调度器
#[derive(Default)]
pub struct FunctionDispatcher {
    /// 已注册的函数元数据
    functions: HashMap<String, FunctionDef>,
    /// 函数处理器（闭包）
    handlers: HashMap<String, FunctionHandler>,
}

impl Clone for FunctionDispatcher {
    fn clone(&self) -> Self {
        Self {
            functions: self.functions.clone(),
            handlers: self.handlers.clone(),
        }
    }
}

impl std::fmt::Debug for FunctionDispatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FunctionDispatcher")
            .field("functions", &self.functions)
            .field("handlers_count", &self.handlers.len())
            .finish()
    }
}

impl FunctionDispatcher {
    /// 创建新的调度器
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册函数元数据（不提供执行逻辑）
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

    /// 注册函数并附带执行处理器
    pub fn register_with_handler(
        &mut self,
        name: impl Into<String>,
        callable_from: CallableFrom,
        handler: FunctionHandler,
    ) {
        let name = name.into();
        self.functions.insert(
            name.clone(),
            FunctionDef {
                name: name.clone(),
                callable_from,
            },
        );
        self.handlers.insert(name, handler);
    }

    /// 执行函数调用
    pub fn dispatch(&self, name: &str, args: Value) -> RenderResult<Value> {
        let func = self
            .functions
            .get(name)
            .ok_or_else(|| crate::error::RendererError::FunctionNotAvailable(name.to_string()))?;

        // 优先使用注册的 handler
        if let Some(handler) = self.handlers.get(name) {
            return handler(args);
        }

        // 简化实现：没有 handler 时返回空值
        let _ = func;
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

    #[test]
    fn test_dispatch_with_handler() {
        let mut dispatcher = FunctionDispatcher::new();
        dispatcher.register_with_handler(
            "upper",
            CallableFrom::ClientOrRemote,
            Arc::new(|args| {
                let s = args.get("value").and_then(|v| v.as_str()).unwrap_or("");
                Ok(json!(s.to_uppercase()))
            }),
        );
        let result = dispatcher
            .dispatch("upper", json!({"value": "hello"}))
            .unwrap();
        assert_eq!(result, json!("HELLO"));
    }

    #[test]
    fn test_dispatch_without_handler_returns_null() {
        let mut dispatcher = FunctionDispatcher::new();
        dispatcher.register("noop", CallableFrom::ClientOrRemote);
        let result = dispatcher.dispatch("noop", json!({})).unwrap();
        assert_eq!(result, Value::Null);
    }
}
