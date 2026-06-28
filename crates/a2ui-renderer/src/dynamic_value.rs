use crate::DataBinding;
use serde_json::Value;

/// Resolve a string-like component property from literal JSON or A2UI dynamic value objects.
pub fn resolve_dynamic_string_prop(
    props: &Value,
    key: &str,
    binding: Option<&DataBinding>,
    fallback: &str,
) -> String {
    props
        .get(key)
        .map(|value| resolve_dynamic_string_value(value, binding))
        .unwrap_or_else(|| fallback.to_string())
}

/// Resolve a single JSON value as display text.
///
/// Supported dynamic value shapes:
/// - `{"path": "/..."}` resolves through `DataBinding` when available.
/// - `{"call": "name"}` is kept as a display placeholder.
pub fn resolve_dynamic_string_value(value: &Value, binding: Option<&DataBinding>) -> String {
    if let Some(s) = value.as_str() {
        return s.to_string();
    }

    if let Some(obj) = value.as_object() {
        if let Some(path) = obj.get("path").and_then(|v| v.as_str()) {
            if let Some(resolved) = binding.and_then(|binding| binding.get(path)) {
                return value_to_display_string(resolved);
            }
            return format!("{{path:{}}}", path);
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
