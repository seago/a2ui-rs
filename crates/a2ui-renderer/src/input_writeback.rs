//! 用户输入回写：把 TextInput/CheckToggle/SliderChange 事件的值写回
//! 组件声明的 DataModel 绑定路径，使双向绑定与 `sendDataModel` 快照
//! 反映最新输入。供各平台渲染器在 `handle_user_event` 中调用。

use crate::component_forest::ComponentForest;
use crate::data_binding::DataBinding;
use crate::error::RenderResult;
use crate::renderer::UserEvent;
use a2ui_core::ComponentId;
use serde_json::Value;
use std::collections::HashMap;

/// 在组件 props 的候选 key 中找到第一个 `{"path": "..."}` 绑定并写回值。
///
/// 返回 `Ok(Some((surface_id, path)))` 时表示写回成功，调用方应据此
/// 标脏对应 surface / 失效渲染缓存；组件不存在、无路径绑定或 surface
/// 的 binding 缺失时返回 `Ok(None)`（不视为错误——字面量属性无处可写）。
///
/// # 示例
///
/// ```rust
/// use a2ui_renderer::{ComponentForest, DataBinding};
/// use a2ui_renderer::input_writeback::write_back_input;
/// use a2ui_core::{ComponentId, DataModel};
/// use std::collections::HashMap;
///
/// let mut forest = ComponentForest::new();
/// let field: a2ui_core::prelude::Component = serde_json::from_value(serde_json::json!({
///     "component": "TextField", "id": "root", "value": {"path": "/form/name"}
/// })).unwrap();
/// forest.upsert("s1", field).unwrap();
///
/// let mut bindings = HashMap::new();
/// bindings.insert("s1".to_string(), DataBinding::new(DataModel::new(serde_json::json!({}))));
///
/// let written = write_back_input(
///     &forest,
///     &mut bindings,
///     &ComponentId::new("root").unwrap(),
///     &["value"],
///     serde_json::json!("alice"),
/// ).unwrap();
/// assert_eq!(written, Some(("s1".to_string(), "/form/name".to_string())));
/// assert_eq!(bindings["s1"].get("/form/name"), Some(&serde_json::json!("alice")));
/// ```
pub fn write_back_input(
    forest: &ComponentForest,
    bindings: &mut HashMap<String, DataBinding>,
    component_id: &ComponentId,
    candidate_keys: &[&str],
    value: Value,
) -> RenderResult<Option<(String, String)>> {
    let Some(surface_id) = forest.surface_of(component_id).map(String::from) else {
        return Ok(None);
    };
    let Some(component) = forest.get(&surface_id, component_id) else {
        return Ok(None);
    };
    let props = component.properties();

    let Some(path) = candidate_keys.iter().find_map(|key| {
        props
            .get(key)
            .and_then(|v| v.get("path"))
            .and_then(|p| p.as_str())
            .map(String::from)
    }) else {
        return Ok(None);
    };

    let Some(binding) = bindings.get_mut(&surface_id) else {
        return Ok(None);
    };
    binding.set(&path, value)?;
    Ok(Some((surface_id, path)))
}

/// 按事件类型选择候选绑定 key 并调用 [`write_back_input`]。
///
/// - `TextInput` → `["value"]`，写字符串
/// - `CheckToggle` → `["value", "checked"]`（各渲染器读取 key 不一，
///   按此顺序取第一个是路径绑定的属性），写布尔
/// - `SliderChange` → `["value"]`，写数值
/// - `Click`/`KeyPress` → 无值可写，返回 `Ok(None)`
///
/// # 示例
///
/// ```rust
/// use a2ui_renderer::{ComponentForest, DataBinding, UserEvent};
/// use a2ui_renderer::input_writeback::write_back_user_event;
/// use a2ui_core::{ComponentId, DataModel};
/// use std::collections::HashMap;
///
/// let mut forest = ComponentForest::new();
/// let checkbox: a2ui_core::prelude::Component = serde_json::from_value(serde_json::json!({
///     "component": "CheckBox", "id": "root", "checked": {"path": "/agree"}
/// })).unwrap();
/// forest.upsert("s1", checkbox).unwrap();
/// let mut bindings = HashMap::new();
/// bindings.insert("s1".to_string(), DataBinding::new(DataModel::new(serde_json::json!({}))));
///
/// let event = UserEvent::CheckToggle {
///     component_id: ComponentId::new("root").unwrap(),
///     checked: true,
/// };
/// let written = write_back_user_event(&forest, &mut bindings, &event).unwrap();
/// assert_eq!(written, Some(("s1".to_string(), "/agree".to_string())));
/// assert_eq!(bindings["s1"].get("/agree"), Some(&serde_json::json!(true)));
/// ```
pub fn write_back_user_event(
    forest: &ComponentForest,
    bindings: &mut HashMap<String, DataBinding>,
    event: &UserEvent,
) -> RenderResult<Option<(String, String)>> {
    match event {
        UserEvent::TextInput {
            component_id,
            value,
        } => write_back_input(
            forest,
            bindings,
            component_id,
            &["value"],
            Value::String(value.clone()),
        ),
        UserEvent::CheckToggle {
            component_id,
            checked,
        } => write_back_input(
            forest,
            bindings,
            component_id,
            &["value", "checked"],
            Value::Bool(*checked),
        ),
        UserEvent::SliderChange {
            component_id,
            value,
        } => {
            let Some(num) = serde_json::Number::from_f64(*value) else {
                // NaN/Infinity 无法表示为 JSON 数值，不写回
                return Ok(None);
            };
            write_back_input(
                forest,
                bindings,
                component_id,
                &["value"],
                Value::Number(num),
            )
        }
        UserEvent::Click { .. } | UserEvent::KeyPress { .. } => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use a2ui_core::DataModel;
    use serde_json::json;

    fn setup(component: serde_json::Value) -> (ComponentForest, HashMap<String, DataBinding>) {
        let mut forest = ComponentForest::new();
        let comp: a2ui_core::prelude::Component = serde_json::from_value(component).unwrap();
        forest.upsert("s1", comp).unwrap();
        let mut bindings = HashMap::new();
        bindings.insert(
            "s1".to_string(),
            DataBinding::new(DataModel::new(json!({}))),
        );
        (forest, bindings)
    }

    #[test]
    fn write_back_text_input_updates_binding() {
        let (forest, mut bindings) = setup(json!({
            "component":"TextField","id":"root","value":{"path":"/form/name"}
        }));
        let event = UserEvent::TextInput {
            component_id: ComponentId::new("root").unwrap(),
            value: "abc".into(),
        };
        let written = write_back_user_event(&forest, &mut bindings, &event).unwrap();
        assert_eq!(written, Some(("s1".to_string(), "/form/name".to_string())));
        assert_eq!(bindings["s1"].get("/form/name"), Some(&json!("abc")));
    }

    #[test]
    fn write_back_check_toggle_prefers_value_then_checked() {
        // 只有 checked 绑定
        let (forest, mut bindings) = setup(json!({
            "component":"CheckBox","id":"root","checked":{"path":"/agree"}
        }));
        let event = UserEvent::CheckToggle {
            component_id: ComponentId::new("root").unwrap(),
            checked: true,
        };
        let written = write_back_user_event(&forest, &mut bindings, &event).unwrap();
        assert_eq!(written, Some(("s1".to_string(), "/agree".to_string())));
        assert_eq!(bindings["s1"].get("/agree"), Some(&json!(true)));

        // value 与 checked 同时存在时优先 value
        let (forest, mut bindings) = setup(json!({
            "component":"CheckBox","id":"root",
            "value":{"path":"/v"},"checked":{"path":"/c"}
        }));
        let written = write_back_user_event(&forest, &mut bindings, &event).unwrap();
        assert_eq!(written, Some(("s1".to_string(), "/v".to_string())));
        assert_eq!(bindings["s1"].get("/v"), Some(&json!(true)));
        assert_eq!(bindings["s1"].get("/c"), None);
    }

    #[test]
    fn write_back_slider_change_writes_number() {
        let (forest, mut bindings) = setup(json!({
            "component":"Slider","id":"root","value":{"path":"/volume"},"min":0,"max":100
        }));
        let event = UserEvent::SliderChange {
            component_id: ComponentId::new("root").unwrap(),
            value: 42.5,
        };
        let written = write_back_user_event(&forest, &mut bindings, &event).unwrap();
        assert_eq!(written, Some(("s1".to_string(), "/volume".to_string())));
        assert_eq!(bindings["s1"].get("/volume"), Some(&json!(42.5)));
    }

    #[test]
    fn write_back_ignores_literal_props() {
        let (forest, mut bindings) = setup(json!({
            "component":"TextField","id":"root","value":"literal text"
        }));
        let event = UserEvent::TextInput {
            component_id: ComponentId::new("root").unwrap(),
            value: "abc".into(),
        };
        let written = write_back_user_event(&forest, &mut bindings, &event).unwrap();
        assert_eq!(written, None);
        assert_eq!(bindings["s1"].as_value(), &json!({}));
    }

    #[test]
    fn write_back_unknown_component_returns_none() {
        let (forest, mut bindings) = setup(json!({
            "component":"TextField","id":"root","value":{"path":"/x"}
        }));
        let event = UserEvent::TextInput {
            component_id: ComponentId::new("ghost").unwrap(),
            value: "abc".into(),
        };
        let written = write_back_user_event(&forest, &mut bindings, &event).unwrap();
        assert_eq!(written, None);
    }

    #[test]
    fn write_back_click_returns_none() {
        let (forest, mut bindings) = setup(json!({
            "component":"Button","id":"root","label":"go"
        }));
        let event = UserEvent::Click {
            component_id: ComponentId::new("root").unwrap(),
        };
        let written = write_back_user_event(&forest, &mut bindings, &event).unwrap();
        assert_eq!(written, None);
    }
}
