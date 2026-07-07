//! 结构化 props 视图：规范中结构已知的声明（action / children / tabs / style）。
//!
//! 这些视图从 [`Component::properties`] 的对应子树解析而来，解析失败一律
//! 返回 `None` / 逐字段宽容（对齐四个平台渲染器的现状行为，不新增报错路径）。
//! 任意自定义键仍走 `prop_*` 访问器或 `properties()` 逃生门。
//!
//! 实现说明：为保持与现状**逐字段**的宽容语义（如 `wantResponse` 非布尔
//! 只按缺失处理、`fontSize` 非数值只丢该字段），视图由手写的宽容提取
//! 构造，而非整体 serde 反序列化——后者在任一字段类型不符时会拖垮整个
//! 视图，与现状不一致。

use crate::component::component::Component;
use crate::component::ComponentId;
use serde_json::{Map, Value};

/// action 声明（规范 §UserAction，`props.action` 子树的类型化视图）。
///
/// 经 [`Component::action_decl`] 获取。
///
/// # 示例
///
/// ```rust
/// use a2ui_core::component::component::Component;
/// use serde::Deserialize;
/// use serde_json::json;
///
/// let c = Component::deserialize(json!({
///     "component": "Button", "id": "b", "child": "lbl",
///     "action": {"event": {"name": "submit", "wantResponse": true}}
/// })).unwrap();
/// let decl = c.action_decl().unwrap();
/// assert_eq!(decl.event.name, "submit");
/// assert_eq!(decl.event.want_response, Some(true));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ActionDecl {
    /// 事件声明（`action.event.*`）
    pub event: EventDecl,
}

/// `action.event` 子对象的类型化视图。
///
/// # 示例
///
/// ```rust
/// use a2ui_core::component::component::Component;
/// use serde::Deserialize;
/// use serde_json::json;
///
/// let c = Component::deserialize(json!({
///     "component": "Button", "id": "b",
///     "action": {"event": {"name": "buy", "context": {"sku": {"path": "/sku"}}}}
/// })).unwrap();
/// let event = c.action_decl().unwrap().event;
/// assert_eq!(event.name, "buy");
/// assert!(event.context.unwrap().contains_key("sku"));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct EventDecl {
    /// 事件名（必填；缺失或非字符串时 `action_decl()` 整体为 `None`，
    /// 对齐现状 warn + 丢弃）
    pub name: String,
    /// context 声明：值是任意 JSON 的 DynamicValue，由渲染层逐个求值
    pub context: Option<Map<String, Value>>,
    /// 是否期待 actionResponse（非布尔值按缺失处理，对齐现状）
    pub want_response: Option<bool>,
    /// 实例 actionId（声明未提供时由客户端自动生成）
    pub action_id: Option<String>,
    /// 响应写回路径（本地语义，不上线路）
    pub response_path: Option<String>,
}

/// children 声明：数组形态或模板形态。
///
/// 注意：TUI 的 `{"children":{"children":[...]}}` 双重嵌套历史兼容形态
/// **不**进入本视图（解析为 `None`），由 TUI 私有代码自行兜底。
///
/// # 示例
///
/// ```rust
/// use a2ui_core::component::component::Component;
/// use a2ui_core::component::decl::ChildrenDecl;
/// use serde::Deserialize;
/// use serde_json::json;
///
/// let c = Component::deserialize(json!({
///     "component": "Column", "id": "col", "children": ["a", "b"]
/// })).unwrap();
/// assert!(matches!(c.children_decl(), Some(ChildrenDecl::Ids(ids)) if ids.len() == 2));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum ChildrenDecl {
    /// 数组形态：子组件 ID 列表（非字符串项与非法 ID 已静默过滤）
    Ids(Vec<ComponentId>),
    /// 模板形态：`{"template": <组件ID>, "path": <数据路径>}`（展开在核心层）
    Template {
        /// 模板组件 ID
        template: ComponentId,
        /// 绑定的数据模型路径
        path: String,
    },
}

/// Tabs 的单个标签页声明（`tabs[]` 数组元素的类型化视图）。
///
/// # 示例
///
/// ```rust
/// use a2ui_core::component::component::Component;
/// use serde::Deserialize;
/// use serde_json::json;
///
/// let c = Component::deserialize(json!({
///     "component": "Tabs", "id": "tabs",
///     "tabs": [{"title": "首页", "child": "home"}]
/// })).unwrap();
/// let tabs = c.tabs_decl().unwrap();
/// assert_eq!(tabs[0].title, "首页");
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TabDecl {
    /// 标签页标题
    pub title: String,
    /// 标签页内容组件 ID
    pub child: ComponentId,
}

/// spacing 声明：单数值或 `{x, y}` 对象。
///
/// # 示例
///
/// ```rust
/// use a2ui_core::component::component::Component;
/// use a2ui_core::component::decl::SpacingDecl;
/// use serde::Deserialize;
/// use serde_json::json;
///
/// let c = Component::deserialize(json!({
///     "component": "Column", "id": "col", "style": {"spacing": 8}
/// })).unwrap();
/// assert_eq!(c.style_decl().unwrap().spacing, Some(SpacingDecl::Uniform(8.0)));
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpacingDecl {
    /// 单数值形态（渲染层现状把它映射为纵向间距）
    Uniform(f64),
    /// `{x, y}` 对象形态（缺省分量按 0.0 处理，对齐现状）
    Xy {
        /// 横向间距
        x: f64,
        /// 纵向间距
        y: f64,
    },
}

/// style 对象的结构提取（数值/开关/颜色字符串**原样**给出；
/// 颜色解析、f32 换算等渲染语义留在 `a2ui-renderer::style`）。
///
/// 逐字段宽容：单字段类型不符只丢该字段，不影响其余字段。
///
/// # 示例
///
/// ```rust
/// use a2ui_core::component::component::Component;
/// use serde::Deserialize;
/// use serde_json::json;
///
/// let c = Component::deserialize(json!({
///     "component": "Text", "id": "t", "text": "hi",
///     "style": {"fontSize": 22, "color": "#1976d2"}
/// })).unwrap();
/// let style = c.style_decl().unwrap();
/// assert_eq!(style.font_size, Some(22.0));
/// assert_eq!(style.color.as_deref(), Some("#1976d2"));
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StyleDecl {
    /// 字号
    pub font_size: Option<f64>,
    /// 加粗
    pub strong: Option<bool>,
    /// 前景色（原始字符串，如 `"#1976d2"`；解析在渲染层）
    pub color: Option<String>,
    /// 填充色（原始字符串；解析在渲染层）
    pub fill: Option<String>,
    /// 内边距
    pub padding: Option<f64>,
    /// 子项间距
    pub spacing: Option<SpacingDecl>,
    /// 圆角半径
    pub radius: Option<f64>,
}

impl Component {
    /// 解析组件的 action 声明（`props.action.event`）。
    ///
    /// 只有声明了 server action 的组件交互才产生消息；`event.name`
    /// 缺失或非字符串时返回 `None`（对齐现状 warn + 丢弃），其余可选
    /// 字段逐字段宽容。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use a2ui_core::component::component::Component;
    /// use serde::Deserialize;
    /// use serde_json::json;
    ///
    /// let c = Component::deserialize(json!({
    ///     "component": "Button", "id": "b",
    ///     "action": {"event": {"name": "submit"}}
    /// })).unwrap();
    /// assert_eq!(c.action_decl().unwrap().event.name, "submit");
    /// assert!(Component::deserialize(json!({
    ///     "component": "Button", "id": "b2"
    /// })).unwrap().action_decl().is_none());
    /// ```
    pub fn action_decl(&self) -> Option<ActionDecl> {
        let event = self.properties().get("action")?.get("event")?;
        let name = event.get("name")?.as_str()?.to_string();
        Some(ActionDecl {
            event: EventDecl {
                name,
                context: event.get("context").and_then(|v| v.as_object()).cloned(),
                want_response: event.get("wantResponse").and_then(|v| v.as_bool()),
                action_id: event
                    .get("actionId")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                response_path: event
                    .get("responsePath")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            },
        })
    }

    /// 解析组件的 children 声明（数组或 `{template, path}` 模板形态）。
    ///
    /// 数组内非字符串项与非法 ID 静默过滤（对齐现状）；其他形态
    /// （含 TUI 双重嵌套历史兼容形态）返回 `None`。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use a2ui_core::component::component::Component;
    /// use a2ui_core::component::decl::ChildrenDecl;
    /// use serde::Deserialize;
    /// use serde_json::json;
    ///
    /// let c = Component::deserialize(json!({
    ///     "component": "List", "id": "l",
    ///     "children": {"template": "item", "path": "/items"}
    /// })).unwrap();
    /// assert!(matches!(
    ///     c.children_decl(),
    ///     Some(ChildrenDecl::Template { path, .. }) if path == "/items"
    /// ));
    /// ```
    pub fn children_decl(&self) -> Option<ChildrenDecl> {
        match self.properties().get("children")? {
            Value::Array(arr) => Some(ChildrenDecl::Ids(
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .filter_map(|s| ComponentId::new(s).ok())
                    .collect(),
            )),
            Value::Object(obj) => {
                let template = obj.get("template")?.as_str()?;
                let path = obj.get("path")?.as_str()?;
                Some(ChildrenDecl::Template {
                    template: ComponentId::new(template).ok()?,
                    path: path.to_string(),
                })
            }
            _ => None,
        }
    }

    /// 解析 Tabs 组件的标签页声明数组。
    ///
    /// `tabs` 缺失或非数组时返回 `None`；`title` / `child` 缺失或非法的
    /// 标签项整项跳过（对齐现状的成对语义）。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use a2ui_core::component::component::Component;
    /// use serde::Deserialize;
    /// use serde_json::json;
    ///
    /// let c = Component::deserialize(json!({
    ///     "component": "Tabs", "id": "tabs",
    ///     "tabs": [{"title": "A", "child": "a"}, {"title": "B", "child": "b"}]
    /// })).unwrap();
    /// assert_eq!(c.tabs_decl().unwrap().len(), 2);
    /// ```
    pub fn tabs_decl(&self) -> Option<Vec<TabDecl>> {
        let arr = self.properties().get("tabs")?.as_array()?;
        Some(
            arr.iter()
                .filter_map(|tab| {
                    Some(TabDecl {
                        title: tab.get("title")?.as_str()?.to_string(),
                        child: ComponentId::new(tab.get("child")?.as_str()?).ok()?,
                    })
                })
                .collect(),
        )
    }

    /// 解析组件的 style 声明（`props.style` 对象的结构提取）。
    ///
    /// `style` 缺失或非对象时返回 `None`；字段级类型不符只丢该字段。
    /// 颜色字符串原样给出，解析留在渲染层。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use a2ui_core::component::component::Component;
    /// use serde::Deserialize;
    /// use serde_json::json;
    ///
    /// let c = Component::deserialize(json!({
    ///     "component": "Card", "id": "card", "child": "col",
    ///     "style": {"padding": 12, "radius": 10}
    /// })).unwrap();
    /// let style = c.style_decl().unwrap();
    /// assert_eq!(style.padding, Some(12.0));
    /// assert_eq!(style.radius, Some(10.0));
    /// ```
    pub fn style_decl(&self) -> Option<StyleDecl> {
        let style = self.properties().get("style")?.as_object()?;
        Some(StyleDecl {
            font_size: style.get("fontSize").and_then(|v| v.as_f64()),
            strong: style.get("strong").and_then(|v| v.as_bool()),
            color: style
                .get("color")
                .and_then(|v| v.as_str())
                .map(String::from),
            fill: style.get("fill").and_then(|v| v.as_str()).map(String::from),
            padding: style.get("padding").and_then(|v| v.as_f64()),
            spacing: style.get("spacing").and_then(parse_spacing),
            radius: style.get("radius").and_then(|v| v.as_f64()),
        })
    }
}

/// spacing 值解析：数值 → Uniform；对象 → Xy（缺省分量 0.0）；其余 None
fn parse_spacing(value: &Value) -> Option<SpacingDecl> {
    if let Some(n) = value.as_f64() {
        return Some(SpacingDecl::Uniform(n));
    }
    let obj = value.as_object()?;
    Some(SpacingDecl::Xy {
        x: obj.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0),
        y: obj.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0),
    })
}

#[cfg(test)]
mod tests {
    use crate::component::component::Component;
    use crate::component::decl::{ChildrenDecl, SpacingDecl};
    use crate::component::ComponentId;
    use serde_json::{json, Value};

    /// 经协议反序列化路径构造组件（与线路输入同形）
    fn component(mut props: Value) -> Component {
        let obj = props.as_object_mut().expect("props must be an object");
        obj.insert("component".into(), json!("Test"));
        obj.insert("id".into(), json!("c1"));
        serde_json::from_value(Value::Object(obj.clone())).expect("valid component")
    }

    // ---- action_decl ----

    #[test]
    fn action_decl_parses_full_spec_shape() {
        let c = component(json!({
            "action": {"event": {
                "name": "submit",
                "context": {"who": {"path": "/user/name"}, "fixed": 1},
                "wantResponse": true,
                "actionId": "a-42",
                "responsePath": "/result"
            }}
        }));
        let decl = c.action_decl().expect("action decl");
        assert_eq!(decl.event.name, "submit");
        let ctx = decl.event.context.expect("context map");
        assert_eq!(ctx.get("who"), Some(&json!({"path": "/user/name"})));
        assert_eq!(ctx.get("fixed"), Some(&json!(1)));
        assert_eq!(decl.event.want_response, Some(true));
        assert_eq!(decl.event.action_id.as_deref(), Some("a-42"));
        assert_eq!(decl.event.response_path.as_deref(), Some("/result"));
    }

    #[test]
    fn action_decl_requires_event_name() {
        // 现状：action.event 缺 name → warn + 丢弃 → 视图整体为 None
        assert!(
            component(json!({"action": {"event": {"wantResponse": true}}}))
                .action_decl()
                .is_none()
        );
        // name 非字符串同样丢弃
        assert!(component(json!({"action": {"event": {"name": 3}}}))
            .action_decl()
            .is_none());
        // 无 action / action 无 event
        assert!(component(json!({})).action_decl().is_none());
        assert!(component(json!({"action": {}})).action_decl().is_none());
    }

    #[test]
    fn action_decl_is_lenient_per_field_for_optional_fields() {
        // 现状：wantResponse 非布尔按缺失处理（unwrap_or(false)），消息仍发送
        let c = component(json!({
            "action": {"event": {
                "name": "submit",
                "wantResponse": "yes",
                "context": "not-an-object",
                "actionId": 5,
                "responsePath": false
            }}
        }));
        let decl = c.action_decl().expect("name 存在即成立");
        assert_eq!(decl.event.want_response, None);
        assert_eq!(decl.event.context, None);
        assert_eq!(decl.event.action_id, None);
        assert_eq!(decl.event.response_path, None);
    }

    // ---- children_decl ----

    #[test]
    fn children_decl_parses_id_array_filtering_invalid_entries() {
        // 对齐现状：非字符串项与非法 ID 静默过滤
        let c = component(json!({"children": ["a", "b", 3, "9bad"]}));
        match c.children_decl() {
            Some(ChildrenDecl::Ids(ids)) => {
                assert_eq!(
                    ids,
                    vec![
                        ComponentId::new("a").unwrap(),
                        ComponentId::new("b").unwrap()
                    ]
                );
            }
            other => panic!("expected Ids, got {:?}", other),
        }
    }

    #[test]
    fn children_decl_parses_template_form() {
        let c = component(json!({"children": {"template": "item_tmpl", "path": "/items"}}));
        match c.children_decl() {
            Some(ChildrenDecl::Template { template, path }) => {
                assert_eq!(template, ComponentId::new("item_tmpl").unwrap());
                assert_eq!(path, "/items");
            }
            other => panic!("expected Template, got {:?}", other),
        }
    }

    #[test]
    fn children_decl_rejects_other_shapes() {
        // TUI 的 {"children":{"children":[...]}} 双重嵌套历史兼容分支
        // 不进 core（留在 TUI 私有代码里）——此形态必须解析为 None
        assert!(component(json!({"children": {"children": ["a"]}}))
            .children_decl()
            .is_none());
        assert!(component(json!({"children": "a"}))
            .children_decl()
            .is_none());
        assert!(component(json!({})).children_decl().is_none());
        // template 缺 path / 非法 template ID → None
        assert!(component(json!({"children": {"template": "t"}}))
            .children_decl()
            .is_none());
        assert!(
            component(json!({"children": {"template": "9bad", "path": "/x"}}))
                .children_decl()
                .is_none()
        );
    }

    // ---- tabs_decl ----

    #[test]
    fn tabs_decl_parses_title_child_pairs() {
        let c = component(json!({"tabs": [
            {"title": "One", "child": "tab1"},
            {"title": "Two", "child": "tab2"}
        ]}));
        let tabs = c.tabs_decl().expect("tabs decl");
        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs[0].title, "One");
        assert_eq!(tabs[0].child, ComponentId::new("tab1").unwrap());
        assert_eq!(tabs[1].title, "Two");
    }

    #[test]
    fn tabs_decl_skips_malformed_tabs_and_requires_array() {
        // 对齐 egui 现状：title 或 child 缺失/非法的 tab 整项跳过
        let c = component(json!({"tabs": [
            {"title": "ok", "child": "t1"},
            {"title": "no child"},
            {"child": "t2"},
            {"title": "bad id", "child": "9bad"}
        ]}));
        let tabs = c.tabs_decl().expect("tabs decl");
        assert_eq!(tabs.len(), 1);
        assert_eq!(tabs[0].title, "ok");

        assert!(component(json!({})).tabs_decl().is_none());
        assert!(component(json!({"tabs": "x"})).tabs_decl().is_none());
    }

    // ---- style_decl ----

    #[test]
    fn style_decl_extracts_raw_fields() {
        let c = component(json!({"style": {
            "fontSize": 22,
            "strong": true,
            "color": "#1976d2",
            "fill": "#fafafa",
            "padding": 12,
            "spacing": {"x": 6, "y": 0},
            "radius": 10
        }}));
        let style = c.style_decl().expect("style decl");
        assert_eq!(style.font_size, Some(22.0));
        assert_eq!(style.strong, Some(true));
        assert_eq!(style.color.as_deref(), Some("#1976d2"));
        assert_eq!(style.fill.as_deref(), Some("#fafafa"));
        assert_eq!(style.padding, Some(12.0));
        assert_eq!(style.spacing, Some(SpacingDecl::Xy { x: 6.0, y: 0.0 }));
        assert_eq!(style.radius, Some(10.0));
    }

    #[test]
    fn style_decl_spacing_number_is_uniform_and_partial_xy_defaults_to_zero() {
        let c = component(json!({"style": {"spacing": 8}}));
        assert_eq!(
            c.style_decl().unwrap().spacing,
            Some(SpacingDecl::Uniform(8.0))
        );
        // 对齐现状：对象形态缺省分量按 0.0 处理
        let c = component(json!({"style": {"spacing": {"x": 6}}}));
        assert_eq!(
            c.style_decl().unwrap().spacing,
            Some(SpacingDecl::Xy { x: 6.0, y: 0.0 })
        );
        // 非数值/非对象 → None（现状 ignores_invalid_spacing）
        let c = component(json!({"style": {"spacing": "tight"}}));
        assert_eq!(c.style_decl().unwrap().spacing, None);
    }

    #[test]
    fn style_decl_is_lenient_per_field() {
        // 单字段类型不符只丢该字段，不拖垮整个视图（对齐现状逐字段 .as_*()）
        let c = component(json!({"style": {"fontSize": "big", "padding": 3, "color": 7}}));
        let style = c.style_decl().expect("style decl");
        assert_eq!(style.font_size, None);
        assert_eq!(style.padding, Some(3.0));
        assert_eq!(style.color, None);
    }

    #[test]
    fn style_decl_requires_style_object() {
        assert!(component(json!({})).style_decl().is_none());
        assert!(component(json!({"style": null})).style_decl().is_none());
        assert!(component(json!({"style": "bold"})).style_decl().is_none());
    }
}
