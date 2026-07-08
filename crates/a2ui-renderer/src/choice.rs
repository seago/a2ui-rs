//! ChoicePicker 选择语义：点击某选项后计算新选中集的纯函数。
//!
//! 规范 basic catalog 的 `variant` 决定行为：`mutuallyExclusive`（默认）
//! 单选整体替换；`multipleSelection` 多选切换成员。逻辑收编在公共层，
//! 平台渲染器只负责把点击事件映射到 [`toggle_choice`] 调用。

use crate::data_binding::DataBinding;
use crate::dynamic_value::{resolve_str_list, value_to_display_string};
use a2ui_core::component::component::Component;
use a2ui_core::component::{prop_keys, DynamicValue};

/// ChoicePicker `variant` 的规范取值：多选。
pub const VARIANT_MULTIPLE_SELECTION: &str = "multipleSelection";
/// ChoicePicker `variant` 的规范取值：单选（规范默认值）。
pub const VARIANT_MUTUALLY_EXCLUSIVE: &str = "mutuallyExclusive";

/// 求值后的 ChoicePicker 选项（label 已按数据绑定解析为展示文本）。
#[derive(Debug, Clone, PartialEq)]
pub struct ChoiceOption {
    /// 展示文本（路径未命中 / 函数调用给出与 `resolve_str` 一致的占位符）
    pub label: String,
    /// 选项的稳定值（写回选中集时使用）
    pub value: String,
}

/// 解析并求值组件的 `options` 声明，供平台直接渲染。
///
/// 形态宽容语义见 [`Component::options_decl`]；label 求值语义与
/// [`crate::dynamic_value::resolve_str`] 一致。`options` 缺失或非数组
/// 时给出空列表。
///
/// # 示例
///
/// ```rust
/// use a2ui_core::component::component::Component;
/// use a2ui_renderer::choice::choice_options;
/// use serde::Deserialize;
/// use serde_json::json;
///
/// let c = Component::deserialize(json!({
///     "component": "ChoicePicker", "id": "cp", "value": [],
///     "options": [{"label": "Email", "value": "email"}]
/// })).unwrap();
/// let options = choice_options(&c, None);
/// assert_eq!(options[0].label, "Email");
/// assert_eq!(options[0].value, "email");
/// ```
pub fn choice_options(component: &Component, binding: Option<&DataBinding>) -> Vec<ChoiceOption> {
    component
        .options_decl()
        .unwrap_or_default()
        .into_iter()
        .map(|opt| ChoiceOption {
            label: resolve_label(&opt.label, binding),
            value: opt.value,
        })
        .collect()
}

/// label（`DynamicValue<String>`）求值：语义对齐 `resolve_str`
fn resolve_label(label: &DynamicValue<String>, binding: Option<&DataBinding>) -> String {
    match label {
        DynamicValue::Literal(s) => s.clone(),
        DynamicValue::Path { path } => match binding.and_then(|b| b.get(path)) {
            Some(value) => value_to_display_string(value),
            None => format!("{{path:{}}}", path),
        },
        DynamicValue::FunctionCall { call, .. } => format!("{{call:{}}}", call),
    }
}

/// 解析并求值组件的当前选中集（规范 `value`: DynamicStringList）。
///
/// 字面量数组直取；`{"path": ...}` 经绑定解析；混入非字符串项的字面量
/// 数组逐项过滤（与数据模型侧的宽容一致）；仍解析不出一律空集。
///
/// # 示例
///
/// ```rust
/// use a2ui_core::component::component::Component;
/// use a2ui_renderer::choice::choice_selected;
/// use serde::Deserialize;
/// use serde_json::json;
///
/// let c = Component::deserialize(json!({
///     "component": "ChoicePicker", "id": "cp",
///     "options": [], "value": ["email"]
/// })).unwrap();
/// assert_eq!(choice_selected(&c, None), vec!["email".to_string()]);
/// ```
pub fn choice_selected(component: &Component, binding: Option<&DataBinding>) -> Vec<String> {
    if let Some(values) = component
        .prop_dynamic_str_list(prop_keys::VALUE)
        .and_then(|dv| resolve_str_list(&dv, binding))
    {
        return values;
    }
    // 类型化解析失败的兜底：字面量数组混入非字符串项时逐项过滤——
    // 与数据模型侧（resolve_str_list 对 path 命中数组）的宽容一致，
    // 声明侧与数据侧不应有不同的宽容度
    component
        .prop_str_list(prop_keys::VALUE)
        .map(|list| list.into_iter().map(String::from).collect())
        .unwrap_or_default()
}

/// 计算点击某选项后的新选中集。
///
/// - `mutuallyExclusive`（`variant` 缺失或取值非法时的规范默认）：整体
///   替换为 `[clicked]`。
/// - `multipleSelection`：`clicked` 已在集合中则移除、否则追加，保持
///   既有顺序。
///
/// # 示例
///
/// ```rust
/// use a2ui_renderer::choice::toggle_choice;
///
/// // 单选（默认）：整体替换
/// assert_eq!(toggle_choice(&["email".into()], "sms", None), vec!["sms"]);
/// // 多选：切换成员
/// assert_eq!(
///     toggle_choice(&["email".into()], "sms", Some("multipleSelection")),
///     vec!["email", "sms"]
/// );
/// ```
pub fn toggle_choice(current: &[String], clicked: &str, variant: Option<&str>) -> Vec<String> {
    if variant == Some(VARIANT_MULTIPLE_SELECTION) {
        if current.iter().any(|v| v == clicked) {
            current.iter().filter(|v| *v != clicked).cloned().collect()
        } else {
            let mut next = current.to_vec();
            next.push(clicked.to_string());
            next
        }
    } else {
        vec![clicked.to_string()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use a2ui_core::component::component::Component;
    use a2ui_core::DataModel;
    use serde::Deserialize;
    use serde_json::json;

    fn selected(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| s.to_string()).collect()
    }

    fn picker(props: serde_json::Value) -> Component {
        let mut obj = props;
        obj["component"] = json!("ChoicePicker");
        obj["id"] = json!("cp");
        Component::deserialize(obj).unwrap()
    }

    fn binding(data: serde_json::Value) -> crate::DataBinding {
        crate::DataBinding::new(DataModel::new(data))
    }

    #[test]
    fn choice_options_resolves_labels_for_both_forms() {
        let b = binding(json!({"labels": {"phone": "电话"}}));
        let c = picker(json!({"options": [
            {"label": "Email", "value": "email"},
            {"label": {"path": "/labels/phone"}, "value": "phone"},
            {"label": {"path": "/labels/missing"}, "value": "m"},
            {"label": {"call": "fmt", "args": {}}, "value": "f"},
            "plain"
        ]}));
        let options = choice_options(&c, Some(&b));
        assert_eq!(options.len(), 5);
        assert_eq!(
            (options[0].label.as_str(), options[0].value.as_str()),
            ("Email", "email")
        );
        // label 路径命中 → 数据模型值
        assert_eq!(options[1].label, "电话");
        // 路径未命中 / 函数调用 → 与 resolve_str 一致的占位符
        assert_eq!(options[2].label, "{path:/labels/missing}");
        assert_eq!(options[3].label, "{call:fmt}");
        // 裸字符串兼容形态
        assert_eq!(
            (options[4].label.as_str(), options[4].value.as_str()),
            ("plain", "plain")
        );
    }

    #[test]
    fn choice_options_empty_when_missing_or_malformed() {
        assert!(choice_options(&picker(json!({})), None).is_empty());
        assert!(choice_options(&picker(json!({"options": "x"})), None).is_empty());
    }

    #[test]
    fn choice_selected_filters_non_string_entries_in_literal_array() {
        // 声明侧与数据模型侧同等宽容：字面量数组混入非字符串项逐项过滤
        // （resolve_str_list 对 path 命中的数组即此语义，两侧必须一致）
        let c = picker(json!({"options": [], "value": ["a", 3, "b"]}));
        assert_eq!(choice_selected(&c, None), selected(&["a", "b"]));
    }

    #[test]
    fn choice_selected_resolves_literal_and_binding() {
        // 字面量数组
        let c = picker(json!({"options": [], "value": ["a"]}));
        assert_eq!(choice_selected(&c, None), selected(&["a"]));
        // 规范主路径：绑定到数据模型的字符串数组
        let b = binding(json!({"contact": {"preference": ["email", "sms"]}}));
        let c = picker(json!({"options": [], "value": {"path": "/contact/preference"}}));
        assert_eq!(choice_selected(&c, Some(&b)), selected(&["email", "sms"]));
        // 未命中 / 缺失 → 空集
        assert_eq!(choice_selected(&c, None), selected(&[]));
        assert_eq!(choice_selected(&picker(json!({})), None), selected(&[]));
    }

    #[test]
    fn mutually_exclusive_replaces_selection() {
        // 显式声明与规范默认（None）行为一致
        for variant in [Some(VARIANT_MUTUALLY_EXCLUSIVE), None] {
            assert_eq!(
                toggle_choice(&selected(&["email"]), "sms", variant),
                selected(&["sms"])
            );
            // 点击已选中项保持选中（单选不反选）
            assert_eq!(
                toggle_choice(&selected(&["email"]), "email", variant),
                selected(&["email"])
            );
            // 空集起步
            assert_eq!(toggle_choice(&[], "email", variant), selected(&["email"]));
        }
    }

    #[test]
    fn multiple_selection_toggles_membership_preserving_order() {
        let variant = Some(VARIANT_MULTIPLE_SELECTION);
        // 未选中 → 追加到尾部
        assert_eq!(
            toggle_choice(&selected(&["a", "b"]), "c", variant),
            selected(&["a", "b", "c"])
        );
        // 已选中 → 移除，其余保持顺序
        assert_eq!(
            toggle_choice(&selected(&["a", "b", "c"]), "b", variant),
            selected(&["a", "c"])
        );
        // 最后一项也可移除为空集
        assert_eq!(
            toggle_choice(&selected(&["a"]), "a", variant),
            selected(&[])
        );
    }

    #[test]
    fn unknown_variant_falls_back_to_mutually_exclusive() {
        // 规范：variant 枚举外取值按默认 mutuallyExclusive 处理
        assert_eq!(
            toggle_choice(&selected(&["a", "b"]), "c", Some("chips")),
            selected(&["c"])
        );
    }
}
