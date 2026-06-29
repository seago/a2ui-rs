use crate::DataBinding;
use serde_json::Value;

/// Resolve a string-like component property from literal JSON or A2UI dynamic value objects.
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
pub fn resolve_dynamic_string_value(value: &Value, binding: Option<&DataBinding>) -> String {
    resolve_dynamic_string_value_with_missing_path(value, binding, |path| {
        format!("{{path:{}}}", path)
    })
}

/// Resolve a single JSON value as display text with caller-defined missing-path formatting.
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
}
