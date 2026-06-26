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
                    let args_map: serde_json::Map<String, serde_json::Value> =
                        args.into_iter().collect();
                    let args_value = serde_json::Value::Object(args_map);
                    if let Ok(value) = dispatcher.dispatch(func_name, args_value) {
                        return html_escape(&value_to_string(value));
                    }
                }
                // 函数不可用或执行失败 → 返回空
                return "".into();
            }

            // 否则解析为 JSON Pointer 路径
            match resolver.resolve(expr) {
                Some(value) => html_escape(&value_to_string(value)),
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

/// HTML 上下文转义：将特殊字符替换为 HTML entity
///
/// 转义规则：
/// - `&` → `&amp;`
/// - `<` → `&lt;`
/// - `>` → `&gt;`
/// - `"` → `&quot;`
///
/// 这是防止 XSS 的必要措施：formatString 的解析结果如果直接拼接到
/// HTML 中，未转义的特殊字符可能导致注入攻击。
pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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

    // --- P2-1: formatString HTML escaping ---

    #[test]
    fn test_resolve_escapes_html_in_path_value() {
        let dm = a2ui_core::DataModel::new(json!({"name": "<script>alert(1)</script>"}));
        let resolver = PathResolver::new(dm);
        let dispatcher = FunctionDispatcher::new();
        let result = FormatString::resolve("Hello, ${name}!", &resolver, &dispatcher);
        assert_eq!(result, "Hello, &lt;script&gt;alert(1)&lt;/script&gt;!");
    }

    #[test]
    fn test_resolve_escapes_ampersand() {
        let dm = a2ui_core::DataModel::new(json!({"brand": "A&T"}));
        let resolver = PathResolver::new(dm);
        let dispatcher = FunctionDispatcher::new();
        let result = FormatString::resolve("${brand}", &resolver, &dispatcher);
        assert_eq!(result, "A&amp;T");
    }

    #[test]
    fn test_resolve_escapes_quotes() {
        let dm = a2ui_core::DataModel::new(json!({"msg": "say \"hello\""}));
        let resolver = PathResolver::new(dm);
        let dispatcher = FunctionDispatcher::new();
        let result = FormatString::resolve("${msg}", &resolver, &dispatcher);
        assert_eq!(result, "say &quot;hello&quot;");
    }

    #[test]
    fn test_resolve_no_escape_on_safe_text() {
        let dm = a2ui_core::DataModel::new(json!({"name": "Alice"}));
        let resolver = PathResolver::new(dm);
        let dispatcher = FunctionDispatcher::new();
        let result = FormatString::resolve("Hello, ${name}!", &resolver, &dispatcher);
        assert_eq!(result, "Hello, Alice!");
    }
}
