//! 输入组件声明状态的统一解析。
//!
//! 四个平台渲染器对「组件声明的输入状态」必须给出一致解读（行为差异
//! 统一专项 §3.2）；平台本地 UI 状态（如 iced 的受控组件缓存）在各
//! 平台侧叠加，缓存未命中时的兜底解析必须走这里。

use crate::dynamic_value::resolve_bool;
use crate::DataBinding;
use a2ui_core::component::component::Component;
use a2ui_core::component::prop_keys;

/// 解析 CheckBox 的勾选状态。
///
/// 规范键 `value`（DynamicBoolean）优先，历史兼容键 `checked`（非规范
/// 扩展）回退，两者均支持动态绑定；都解析不出时为 `false`。
///
/// # 示例
///
/// ```rust
/// use a2ui_core::component::component::Component;
/// use a2ui_renderer::checkbox_checked;
/// use serde::Deserialize;
/// use serde_json::json;
///
/// let c = Component::deserialize(json!({
///     "component": "CheckBox", "id": "cb", "label": "subscribe", "value": true
/// })).unwrap();
/// assert!(checkbox_checked(&c, None));
/// ```
pub fn checkbox_checked(component: &Component, binding: Option<&DataBinding>) -> bool {
    component
        .prop_dynamic_bool(prop_keys::VALUE)
        .and_then(|dv| resolve_bool(&dv, binding))
        .or_else(|| {
            component
                .prop_dynamic_bool(prop_keys::CHECKED)
                .and_then(|dv| resolve_bool(&dv, binding))
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use a2ui_core::DataModel;
    use serde::Deserialize;
    use serde_json::json;

    fn component(props: serde_json::Value) -> Component {
        let mut obj = props;
        obj["component"] = json!("CheckBox");
        obj["id"] = json!("cb");
        Component::deserialize(obj).unwrap()
    }

    fn binding(data: serde_json::Value) -> DataBinding {
        DataBinding::new(DataModel::new(data))
    }

    #[test]
    fn value_literal_and_binding_resolve() {
        assert!(checkbox_checked(&component(json!({"value": true})), None));
        assert!(!checkbox_checked(&component(json!({"value": false})), None));

        let b = binding(json!({"agree": true}));
        assert!(checkbox_checked(
            &component(json!({"value": {"path": "/agree"}})),
            Some(&b)
        ));
    }

    #[test]
    fn checked_fallback_supports_literal_and_binding() {
        // 历史兼容键：字面量（TUI 现状形态）与动态绑定都要支持
        assert!(checkbox_checked(&component(json!({"checked": true})), None));

        let b = binding(json!({"agree": true}));
        assert!(checkbox_checked(
            &component(json!({"checked": {"path": "/agree"}})),
            Some(&b)
        ));
    }

    #[test]
    fn value_takes_priority_over_checked() {
        let b = binding(json!({"v": false, "c": true}));
        // value 解析成功（false）即生效，不再看 checked
        assert!(!checkbox_checked(
            &component(json!({"value": {"path": "/v"}, "checked": {"path": "/c"}})),
            Some(&b)
        ));
    }

    #[test]
    fn unresolvable_value_falls_through_to_checked() {
        // value 绑定未命中 → 回退 checked（egui/web 现状的 or_else 语义）
        let b = binding(json!({"c": true}));
        assert!(checkbox_checked(
            &component(json!({"value": {"path": "/missing"}, "checked": {"path": "/c"}})),
            Some(&b)
        ));
    }

    #[test]
    fn defaults_to_false_when_nothing_resolves() {
        assert!(!checkbox_checked(&component(json!({})), None));
        assert!(!checkbox_checked(
            &component(json!({"value": {"path": "/missing"}})),
            None
        ));
        // 类型不符按缺失处理
        assert!(!checkbox_checked(
            &component(json!({"value": "yes", "checked": 1})),
            None
        ));
    }
}
