use crate::DataBinding;
use a2ui_core::component::DynamicValue;
use serde_json::Value;

/// 类型化动态字符串求值（新 API，配合 `Component::prop_dynamic_value`）。
///
/// 语义与 [`resolve_dynamic_string_value`] 逐条对齐：
/// - `Literal`：任意 JSON 字面量按显示文本给出（字符串原样，其余 `to_string`）
/// - `Path`：经 [`DataBinding`] 解析；未命中给 `{path:...}` 占位符
/// - `FunctionCall`：widget 层不求值，给 `{call:...}` 占位符
///
/// # 示例
///
/// ```rust
/// use a2ui_core::component::DynamicValue;
/// use a2ui_core::prelude::json;
/// use a2ui_renderer::dynamic_value::resolve_str;
///
/// assert_eq!(resolve_str(&DynamicValue::Literal(json!("hi")), None), "hi");
/// assert_eq!(resolve_str(&DynamicValue::Literal(json!(3)), None), "3");
/// ```
pub fn resolve_str(dv: &DynamicValue, binding: Option<&DataBinding>) -> String {
    resolve_str_with_missing_path(dv, binding, |path| format!("{{path:{}}}", path))
}

/// 同 [`resolve_str`]，但允许调用方自定义 path 未命中时的占位格式。
///
/// # 示例
///
/// ```rust
/// use a2ui_core::component::DynamicValue;
/// use a2ui_renderer::dynamic_value::resolve_str_with_missing_path;
///
/// let dv = DynamicValue::Path { path: "/missing".into() };
/// assert_eq!(
///     resolve_str_with_missing_path(&dv, None, |p| format!("{{{}…}}", p)),
///     "{/missing…}"
/// );
/// ```
pub fn resolve_str_with_missing_path(
    dv: &DynamicValue,
    binding: Option<&DataBinding>,
    missing_path: impl Fn(&str) -> String,
) -> String {
    match dv {
        DynamicValue::Literal(value) => value_to_display_string(value),
        DynamicValue::Path { path } => match binding.and_then(|binding| binding.get(path)) {
            Some(resolved) => value_to_display_string(resolved),
            None => missing_path(path),
        },
        DynamicValue::FunctionCall { call, .. } => format!("{{call:{}}}", call),
    }
}

/// 类型化动态布尔求值（新 API，收编 egui/iced/web 三份手写实现）。
///
/// `Literal` 直接给出；`Path` 经绑定解析且目标必须是布尔；
/// `FunctionCall` 在 widget 层不求值（现状无 dispatcher）→ `None`。
///
/// # 示例
///
/// ```rust
/// use a2ui_core::component::DynamicValue;
/// use a2ui_renderer::dynamic_value::resolve_bool;
///
/// assert_eq!(resolve_bool(&DynamicValue::Literal(true), None), Some(true));
/// assert_eq!(
///     resolve_bool(&DynamicValue::Path { path: "/x".into() }, None),
///     None
/// );
/// ```
pub fn resolve_bool(dv: &DynamicValue<bool>, binding: Option<&DataBinding>) -> Option<bool> {
    match dv {
        DynamicValue::Literal(b) => Some(*b),
        DynamicValue::Path { path } => binding
            .and_then(|binding| binding.get(path))
            .and_then(|value| value.as_bool()),
        DynamicValue::FunctionCall { .. } => None,
    }
}

/// 类型化动态数值求值（新 API，收编 egui/iced/web 三份手写实现）。
///
/// 语义同 [`resolve_bool`]，目标类型为 `f64`（整数字面量亦可）。
///
/// # 示例
///
/// ```rust
/// use a2ui_core::component::DynamicValue;
/// use a2ui_renderer::dynamic_value::resolve_f64;
///
/// assert_eq!(resolve_f64(&DynamicValue::Literal(5.0), None), Some(5.0));
/// ```
pub fn resolve_f64(dv: &DynamicValue<f64>, binding: Option<&DataBinding>) -> Option<f64> {
    match dv {
        DynamicValue::Literal(n) => Some(*n),
        DynamicValue::Path { path } => binding
            .and_then(|binding| binding.get(path))
            .and_then(|value| value.as_f64()),
        DynamicValue::FunctionCall { .. } => None,
    }
}

/// Resolve a string-like component property from literal JSON or A2UI dynamic value objects.
#[deprecated(
    since = "0.1.0",
    note = "改用 `Component::prop_dynamic_value` + `resolve_str`（类型化新 API）"
)]
#[allow(deprecated)] // 委托同批 deprecated 的旧实现
pub fn resolve_dynamic_string_prop(
    props: &Value,
    key: &str,
    binding: Option<&DataBinding>,
    fallback: &str,
) -> String {
    resolve_dynamic_string_prop_with_missing_path(props, key, binding, fallback, |path| {
        format!("{{path:{}}}", path)
    })
}

/// Resolve a string-like component property with caller-defined missing-path formatting.
#[deprecated(
    since = "0.1.0",
    note = "改用 `Component::prop_dynamic_value` + `resolve_str_with_missing_path`（类型化新 API）"
)]
#[allow(deprecated)] // 委托同批 deprecated 的旧实现
pub fn resolve_dynamic_string_prop_with_missing_path(
    props: &Value,
    key: &str,
    binding: Option<&DataBinding>,
    fallback: &str,
    missing_path: impl Fn(&str) -> String,
) -> String {
    match props.get(key) {
        Some(value) => resolve_dynamic_string_value_with_missing_path(value, binding, missing_path),
        None => fallback.to_string(),
    }
}

/// Resolve a single JSON value as display text.
///
/// Supported dynamic value shapes:
/// - `{"path": "/..."}` resolves through `DataBinding` when available.
/// - `{"call": "name"}` is kept as a display placeholder.
#[deprecated(since = "0.1.0", note = "改用 `resolve_str`（类型化新 API）")]
#[allow(deprecated)] // 委托同批 deprecated 的旧实现
pub fn resolve_dynamic_string_value(value: &Value, binding: Option<&DataBinding>) -> String {
    resolve_dynamic_string_value_with_missing_path(value, binding, |path| {
        format!("{{path:{}}}", path)
    })
}

/// Resolve a single JSON value as display text with caller-defined missing-path formatting.
#[deprecated(
    since = "0.1.0",
    note = "改用 `resolve_str_with_missing_path`（类型化新 API）"
)]
pub fn resolve_dynamic_string_value_with_missing_path(
    value: &Value,
    binding: Option<&DataBinding>,
    missing_path: impl Fn(&str) -> String,
) -> String {
    if let Some(s) = value.as_str() {
        return s.to_string();
    }

    if let Some(obj) = value.as_object() {
        if let Some(path) = obj.get("path").and_then(|v| v.as_str()) {
            if let Some(resolved) = binding.and_then(|binding| binding.get(path)) {
                return value_to_display_string(resolved);
            }
            return missing_path(path);
        }
        if let Some(call) = obj.get("call").and_then(|v| v.as_str()) {
            return format!("{{call:{}}}", call);
        }
    }

    value_to_display_string(value)
}

pub fn value_to_display_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
#[allow(deprecated)] // 旧 &Value 入参 API 的等价性基线测试仍需调用它们
mod tests {
    use super::*;
    use a2ui_core::prelude::*;
    use serde_json::json;

    #[test]
    fn resolves_literal_strings() {
        assert_eq!(
            resolve_dynamic_string_prop(&json!({"text": "Hello"}), "text", None, "[Text]"),
            "Hello"
        );
    }

    #[test]
    fn resolves_non_string_literals_as_display_text() {
        assert_eq!(
            resolve_dynamic_string_prop(&json!({"text": 3}), "text", None, "[Text]"),
            "3"
        );
        assert_eq!(
            resolve_dynamic_string_prop(&json!({"text": true}), "text", None, "[Text]"),
            "true"
        );
    }

    #[test]
    fn resolves_paths_from_data_binding() {
        let binding = DataBinding::new(DataModel::new(json!({
            "user": {"name": "Alice", "count": 2}
        })));

        assert_eq!(
            resolve_dynamic_string_prop(
                &json!({"text": {"path": "/user/name"}}),
                "text",
                Some(&binding),
                "[Text]"
            ),
            "Alice"
        );
        assert_eq!(
            resolve_dynamic_string_prop(
                &json!({"text": {"path": "/user/count"}}),
                "text",
                Some(&binding),
                "[Text]"
            ),
            "2"
        );
    }

    #[test]
    fn keeps_missing_paths_as_placeholders() {
        let binding = DataBinding::new(DataModel::empty());

        assert_eq!(
            resolve_dynamic_string_prop(
                &json!({"text": {"path": "/missing"}}),
                "text",
                Some(&binding),
                "[Text]"
            ),
            "{path:/missing}"
        );
    }

    #[test]
    fn supports_custom_missing_path_placeholders() {
        let binding = DataBinding::new(DataModel::empty());

        assert_eq!(
            resolve_dynamic_string_prop_with_missing_path(
                &json!({"text": {"path": "/missing"}}),
                "text",
                Some(&binding),
                "[Text]",
                |path| format!("{{{}…}}", path)
            ),
            "{/missing…}"
        );
    }

    #[test]
    fn keeps_calls_as_placeholders() {
        assert_eq!(
            resolve_dynamic_string_prop(
                &json!({"text": {"call": "formatTitle"}}),
                "text",
                None,
                "[Text]"
            ),
            "{call:formatTitle}"
        );
    }

    #[test]
    fn returns_fallback_for_missing_property() {
        assert_eq!(
            resolve_dynamic_string_prop(&json!({}), "text", None, "[Text]"),
            "[Text]"
        );
    }

    // ---- 类型化新 API（DynamicValue<T> 入参）：与手写旧分支语义逐条对齐 ----

    #[test]
    fn resolve_str_matches_legacy_display_semantics() {
        let binding = DataBinding::new(DataModel::new(json!({
            "user": {"name": "Alice", "count": 2}
        })));

        // 字面量字符串
        assert_eq!(
            resolve_str(&DynamicValue::Literal(json!("Hello")), None),
            "Hello"
        );
        // 非字符串字面量按显示文本渲染（现状 value_to_display_string）
        assert_eq!(resolve_str(&DynamicValue::Literal(json!(3)), None), "3");
        assert_eq!(
            resolve_str(&DynamicValue::Literal(json!(true)), None),
            "true"
        );
        // path 命中 → 解析值；未命中 → 占位符
        assert_eq!(
            resolve_str(
                &DynamicValue::Path {
                    path: "/user/name".into()
                },
                Some(&binding)
            ),
            "Alice"
        );
        assert_eq!(
            resolve_str(
                &DynamicValue::Path {
                    path: "/missing".into()
                },
                Some(&binding)
            ),
            "{path:/missing}"
        );
        // call → 占位符
        assert_eq!(
            resolve_str(
                &DynamicValue::FunctionCall {
                    call: "formatTitle".into(),
                    args: json!(null)
                },
                None
            ),
            "{call:formatTitle}"
        );
    }

    #[test]
    fn resolve_str_with_missing_path_supports_custom_placeholder() {
        let binding = DataBinding::new(DataModel::empty());
        assert_eq!(
            resolve_str_with_missing_path(
                &DynamicValue::Path {
                    path: "/missing".into()
                },
                Some(&binding),
                |path| format!("{{{}…}}", path)
            ),
            "{/missing…}"
        );
    }

    #[test]
    fn resolve_bool_matches_legacy_triplet_semantics() {
        let binding = DataBinding::new(DataModel::new(json!({"agree": true, "n": 3})));

        assert_eq!(resolve_bool(&DynamicValue::Literal(true), None), Some(true));
        assert_eq!(
            resolve_bool(
                &DynamicValue::Path {
                    path: "/agree".into()
                },
                Some(&binding)
            ),
            Some(true)
        );
        // path 指向非布尔 → None（现状 .as_bool() 失败即 None）
        assert_eq!(
            resolve_bool(&DynamicValue::Path { path: "/n".into() }, Some(&binding)),
            None
        );
        // path 未命中 / 无绑定 → None
        assert_eq!(
            resolve_bool(
                &DynamicValue::Path {
                    path: "/missing".into()
                },
                Some(&binding)
            ),
            None
        );
        assert_eq!(
            resolve_bool(
                &DynamicValue::Path {
                    path: "/agree".into()
                },
                None
            ),
            None
        );
        // 函数调用形态在 widget 层不求值（现状无 dispatcher）→ None
        assert_eq!(
            resolve_bool(
                &DynamicValue::FunctionCall {
                    call: "f".into(),
                    args: json!(null)
                },
                Some(&binding)
            ),
            None
        );
    }

    #[test]
    fn resolve_f64_matches_legacy_triplet_semantics() {
        let binding = DataBinding::new(DataModel::new(json!({"volume": 0.5, "s": "x"})));

        assert_eq!(resolve_f64(&DynamicValue::Literal(5.0), None), Some(5.0));
        assert_eq!(
            resolve_f64(
                &DynamicValue::Path {
                    path: "/volume".into()
                },
                Some(&binding)
            ),
            Some(0.5)
        );
        assert_eq!(
            resolve_f64(&DynamicValue::Path { path: "/s".into() }, Some(&binding)),
            None
        );
        assert_eq!(
            resolve_f64(
                &DynamicValue::FunctionCall {
                    call: "f".into(),
                    args: json!(null)
                },
                Some(&binding)
            ),
            None
        );
    }
}
