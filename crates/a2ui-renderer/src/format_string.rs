use crate::function_dispatcher::{CallableFrom, FunctionDispatcher};
use crate::path_resolver::PathResolver;

/// formatString 插值解析器
///
/// 支持两种插值语法：
/// - `${path}` — JSON Pointer 路径，从 DataModel 解析值
/// - `${funcName:key=value}` — 调用注册的函数
///
/// 字面量文本原样保留。
pub struct FormatString;

impl FormatString {
    /// 解析模板字符串，返回插值后的结果
    pub fn resolve(
        template: &str,
        resolver: &PathResolver,
        dispatcher: &FunctionDispatcher,
    ) -> String {
        let re = regex::Regex::new(r"\$\{([^}]+)\}").unwrap();

        re.replace_all(template, |caps: &regex::Captures| {
            let expr = caps.get(1).unwrap().as_str();

            // 尝试解析为函数调用：funcName:key=value,key2=value2
            if let Some(colon_pos) = expr.find(':') {
                let func_name = &expr[..colon_pos];
                let args_str = &expr[colon_pos + 1..];

                if dispatcher.can_call_from(func_name, CallableFrom::ClientOrRemote) {
                    let args = parse_function_args(args_str);
                    let args_map: serde_json::Map<String, serde_json::Value> = args.into_iter().collect();
                    let args_value = serde_json::Value::Object(args_map);
                    if let Ok(value) = dispatcher.dispatch(func_name, args_value) {
                        return value_to_string(value);
                    }
                }
                // 函数不可用或执行失败 → 返回空
                return "".into();
            }

            // 否则解析为 JSON Pointer 路径
            match resolver.resolve(expr) {
                Some(value) => value_to_string(value),
                None => "".into(),
            }
        })
        .into_owned()
    }
}

/// 将 serde_json::Value 转换为显示字符串（字符串类型去掉引号）
fn value_to_string(value: serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s,
        _ => value.to_string(),
    }
}

/// 解析函数参数字符串 "key=value,key2=value2" → HashMap
fn parse_function_args(s: &str) -> std::collections::HashMap<String, serde_json::Value> {
    let mut args = std::collections::HashMap::new();
    for pair in s.split(',') {
        if let Some(eq) = pair.find('=') {
            let key = pair[..eq].trim().to_string();
            let val = pair[eq + 1..].trim().to_string();
            args.insert(key, serde_json::Value::String(val));
        }
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Arc;

    #[test]
    fn test_resolve_literal() {
        let dm = a2ui_core::DataModel::new(json!({}));
        let resolver = PathResolver::new(dm);
        let result = FormatString::resolve("hello", &resolver, &FunctionDispatcher::new());
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_resolve_path_interpolation() {
        let dm = a2ui_core::DataModel::new(json!({"user": {"name": "Alice"}}));
        let resolver = PathResolver::new(dm);
        let dispatcher = FunctionDispatcher::new();
        let result = FormatString::resolve("Hello, ${user/name}!", &resolver, &dispatcher);
        assert_eq!(result, "Hello, Alice!");
    }

    #[test]
    fn test_resolve_function_call() {
        let dm = a2ui_core::DataModel::new(json!({}));
        let resolver = PathResolver::new(dm);
        let mut dispatcher = FunctionDispatcher::new();
        dispatcher.register_with_handler(
            "upper",
            CallableFrom::ClientOrRemote,
            Arc::new(|args| {
                let s = args.get("value").and_then(|v| v.as_str()).unwrap_or("");
                Ok(json!(s.to_uppercase()))
            }),
        );
        let result = FormatString::resolve("${upper:value=hello}", &resolver, &dispatcher);
        assert_eq!(result, "HELLO");
    }

    #[test]
    fn test_resolve_multiple_interpolations() {
        let dm = a2ui_core::DataModel::new(json!({"first": "Alice", "last": "Bob"}));
        let resolver = PathResolver::new(dm);
        let dispatcher = FunctionDispatcher::new();
        let result = FormatString::resolve("${first} ${last}", &resolver, &dispatcher);
        assert_eq!(result, "Alice Bob");
    }

    #[test]
    fn test_resolve_unknown_path_returns_empty() {
        let dm = a2ui_core::DataModel::new(json!({}));
        let resolver = PathResolver::new(dm);
        let dispatcher = FunctionDispatcher::new();
        let result = FormatString::resolve("Hello, ${missing/path}!", &resolver, &dispatcher);
        assert_eq!(result, "Hello, !");
    }

    #[test]
    fn test_resolve_unknown_function_returns_empty() {
        let dm = a2ui_core::DataModel::new(json!({}));
        let resolver = PathResolver::new(dm);
        let dispatcher = FunctionDispatcher::new();
        let result = FormatString::resolve("${unknownFunc:value=x}", &resolver, &dispatcher);
        assert_eq!(result, "");
    }
}
