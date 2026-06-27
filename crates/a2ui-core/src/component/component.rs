use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---- TabItem ----

/// 标签项：标题 + 子组件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabItem {
    pub title: String,
    pub child: ComponentId,
}

// ---- ComponentId ----

/// 组件标识符，遵循 Unicode UAX #31 命名规则
/// 正则: ^[\p{XID_Start}_][\p{XID_Continue}]*$
/// @ 命名空间保留给系统
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ComponentId(String);

impl ComponentId {
    /// 创建新的 ComponentId，校验命名规则
    pub fn new<S: AsRef<str>>(s: S) -> crate::error::Result<Self> {
        let s = s.as_ref();
        if s.is_empty() {
            return Err(crate::error::A2uiError::InvalidComponentId(
                "empty ID".to_string(),
            ));
        }
        if s.starts_with('@') {
            return Err(crate::error::A2uiError::InvalidComponentId(format!(
                "'@' namespace is reserved for system: {}",
                s
            )));
        }
        // UAX #31: 首字符必须是 XID_Start 或 _
        let mut chars = s.chars();
        let first = chars.next().unwrap();
        if !is_xid_start(first) {
            return Err(crate::error::A2uiError::InvalidComponentId(format!(
                "ID must start with XID_Start or '_': {}",
                s
            )));
        }
        // 后续字符必须是 XID_Continue
        for c in chars {
            if !is_xid_continue(c) {
                return Err(crate::error::A2uiError::InvalidComponentId(format!(
                    "ID contains invalid character '{}': {}",
                    c, s
                )));
            }
        }
        Ok(Self(s.to_string()))
    }

    /// 获取内部字符串
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ComponentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for ComponentId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Unicode XID_Start 检查，使用 unicode-ident crate 实现严格 UAX #31 验证
fn is_xid_start(c: char) -> bool {
    c == '_' || unicode_ident::is_xid_start(c)
}

/// Unicode XID_Continue 检查，使用 unicode-ident crate 实现严格 UAX #31 验证
fn is_xid_continue(c: char) -> bool {
    c == '_' || unicode_ident::is_xid_continue(c)
}

// ---- DynamicValue ----

/// 动态值类型：支持字面量、路径绑定、函数调用三种形式
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DynamicValue<T = Value> {
    /// 字面量值
    Literal(T),
    /// 绑定到 Data Model 的路径
    #[serde(rename = "path")]
    Path { path: String },
    /// 调用注册函数
    #[serde(rename = "call")]
    FunctionCall { call: String, args: Value },
}

impl DynamicValue<String> {
    /// 创建字面量字符串
    pub fn literal(s: impl Into<String>) -> Self {
        DynamicValue::Literal(s.into())
    }

    /// 创建路径绑定
    pub fn path(path: impl Into<String>) -> Self {
        DynamicValue::Path { path: path.into() }
    }

    /// 创建函数调用
    pub fn call(func: impl Into<String>, args: Value) -> Self {
        DynamicValue::FunctionCall {
            call: func.into(),
            args,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            DynamicValue::Literal(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_path(&self) -> Option<&str> {
        match self {
            DynamicValue::Path { path } => Some(path.as_str()),
            _ => None,
        }
    }

    pub fn as_function_call(&self) -> Option<&str> {
        match self {
            DynamicValue::FunctionCall { call, .. } => Some(call.as_str()),
            _ => None,
        }
    }
}

impl DynamicValue<i64> {
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            DynamicValue::Literal(n) => Some(*n),
            _ => None,
        }
    }
}

impl DynamicValue<bool> {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            DynamicValue::Literal(b) => Some(*b),
            _ => None,
        }
    }
}

// ---- ComponentCommon ----

/// 组件通用属性（混入所有组件）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentCommon {
    /// 组件唯一标识符
    pub id: ComponentId,
    /// 无障碍属性
    pub accessibility: Option<AccessibilityAttributes>,
    /// 权重（类似 CSS flex-grow，仅在 Row/Column 直接子组件时有效）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<f64>,
}

/// 无障碍属性
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccessibilityAttributes {
    /// 无障碍标签
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// 详细描述
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

// ---- ComponentType ----

/// 组件类型枚举（用于反序列化时的类型分发）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "PascalCase")]
pub enum ComponentType {
    Text,
    Image,
    Icon,
    Video,
    AudioPlayer,
    Row,
    Column,
    List,
    Card,
    Tabs,
    Modal,
    Divider,
    Button,
    TextField,
    CheckBox,
    ChoicePicker,
    Slider,
    DateTimeInput,
}

impl ComponentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ComponentType::Text => "Text",
            ComponentType::Image => "Image",
            ComponentType::Icon => "Icon",
            ComponentType::Video => "Video",
            ComponentType::AudioPlayer => "AudioPlayer",
            ComponentType::Row => "Row",
            ComponentType::Column => "Column",
            ComponentType::List => "List",
            ComponentType::Card => "Card",
            ComponentType::Tabs => "Tabs",
            ComponentType::Modal => "Modal",
            ComponentType::Divider => "Divider",
            ComponentType::Button => "Button",
            ComponentType::TextField => "TextField",
            ComponentType::CheckBox => "CheckBox",
            ComponentType::ChoicePicker => "ChoicePicker",
            ComponentType::Slider => "Slider",
            ComponentType::DateTimeInput => "DateTimeInput",
        }
    }
}

// ---- Component ----

/// UI 组件（协议中的最小构建块）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Component {
    /// 组件类型名（如 "Text", "Button"）
    #[serde(rename = "component")]
    component_type: String,
    /// 通用属性
    #[serde(flatten)]
    common: crate::component::component::ComponentCommon,
    /// 组件特有属性（JSON 对象，由 Catalog schema 定义）
    #[serde(flatten)]
    properties: Value,
}

/// 将 DynamicValue<String> 按指定 key 转换为 JSON 属性对象
fn json_dynamic_string(key: &str, value: &DynamicValue<String>) -> serde_json::Value {
    match value {
        DynamicValue::Literal(s) => serde_json::json!({(key): s}),
        DynamicValue::Path { path } => serde_json::json!({(key): {"path": path}}),
        DynamicValue::FunctionCall { call, args } => {
            serde_json::json!({(key): {"call": call, "args": args}})
        }
    }
}

impl Component {
    /// 创建 Text 组件
    pub fn text(id: ComponentId, text: DynamicValue<String>) -> Self {
        Self {
            component_type: "Text".to_string(),
            common: crate::component::component::ComponentCommon {
                id,
                accessibility: None,
                weight: None,
            },
            properties: match text {
                DynamicValue::Literal(s) => serde_json::json!({"text": s}),
                DynamicValue::Path { path } => serde_json::json!({"text": {"path": path}}),
                DynamicValue::FunctionCall { call, args } => {
                    serde_json::json!({"text": {"call": call, "args": args}})
                }
            },
        }
    }

    /// 创建 Button 组件
    pub fn button(id: ComponentId, child: ComponentId) -> Self {
        Self {
            component_type: "Button".to_string(),
            common: crate::component::component::ComponentCommon {
                id,
                accessibility: None,
                weight: None,
            },
            properties: serde_json::json!({"child": child.as_str()}),
        }
    }

    /// 创建 Column 组件
    pub fn column(id: ComponentId, children: Vec<ComponentId>) -> Self {
        let ids: Vec<String> = children.iter().map(|c| c.as_str().to_string()).collect();
        Self {
            component_type: "Column".to_string(),
            common: crate::component::component::ComponentCommon {
                id,
                accessibility: None,
                weight: None,
            },
            properties: serde_json::json!({"children": ids}),
        }
    }

    /// 创建 Row 组件
    pub fn row(id: ComponentId, children: Vec<ComponentId>) -> Self {
        let ids: Vec<String> = children.iter().map(|c| c.as_str().to_string()).collect();
        Self {
            component_type: "Row".to_string(),
            common: crate::component::component::ComponentCommon {
                id,
                accessibility: None,
                weight: None,
            },
            properties: serde_json::json!({"children": ids}),
        }
    }

    /// 设置权重
    pub fn with_weight(mut self, weight: f64) -> Self {
        self.common.weight = Some(weight);
        self
    }

    /// 创建 List 组件（支持 Array 和 Object 两种 ChildList 模式）
    pub fn list(id: ComponentId, children: crate::component::child_list::ChildList) -> Self {
        let properties = match &children {
            crate::component::child_list::ChildList::Array { list } => {
                let ids: Vec<String> = list.iter().map(|c| c.as_str().to_string()).collect();
                serde_json::json!({"children": ids})
            }
            crate::component::child_list::ChildList::Object { template, path } => {
                serde_json::json!({"children": {"template": template.as_str(), "path": path}})
            }
        };
        Self {
            component_type: "List".to_string(),
            common: ComponentCommon {
                id,
                accessibility: None,
                weight: None,
            },
            properties,
        }
    }

    /// 创建 Card 组件
    pub fn card(id: ComponentId, child: ComponentId) -> Self {
        Self {
            component_type: "Card".to_string(),
            common: ComponentCommon {
                id,
                accessibility: None,
                weight: None,
            },
            properties: serde_json::json!({"child": child.as_str()}),
        }
    }

    /// 创建 Tabs 组件
    pub fn tabs(id: ComponentId, tabs: Vec<TabItem>) -> Self {
        let tabs_json: Vec<Value> = tabs
            .iter()
            .map(|t| serde_json::json!({"title": t.title, "child": t.child.as_str()}))
            .collect();
        Self {
            component_type: "Tabs".to_string(),
            common: ComponentCommon {
                id,
                accessibility: None,
                weight: None,
            },
            properties: serde_json::json!({"tabs": tabs_json}),
        }
    }

    /// 创建 Modal 组件
    pub fn modal(id: ComponentId, content: ComponentId, trigger: ComponentId) -> Self {
        Self {
            component_type: "Modal".to_string(),
            common: ComponentCommon {
                id,
                accessibility: None,
                weight: None,
            },
            properties: serde_json::json!({
                "content": content.as_str(),
                "trigger": trigger.as_str()
            }),
        }
    }

    /// 创建 Image 组件
    pub fn image(id: ComponentId, url: DynamicValue<String>) -> Self {
        Self {
            component_type: "Image".to_string(),
            common: ComponentCommon {
                id,
                accessibility: None,
                weight: None,
            },
            properties: json_dynamic_string("url", &url),
        }
    }

    /// 设置 Image 组件的 fit 属性
    pub fn with_fit(mut self, fit: String) -> Self {
        self.properties["fit"] = serde_json::json!(fit);
        self
    }

    /// 设置 Image 组件的 variant 属性
    pub fn with_variant(mut self, variant: String) -> Self {
        self.properties["variant"] = serde_json::json!(variant);
        self
    }

    /// 创建 Icon 组件
    pub fn icon(id: ComponentId, name: DynamicValue<String>) -> Self {
        Self {
            component_type: "Icon".to_string(),
            common: ComponentCommon {
                id,
                accessibility: None,
                weight: None,
            },
            properties: json_dynamic_string("name", &name),
        }
    }

    /// 创建 Video 组件
    pub fn video(id: ComponentId, url: DynamicValue<String>) -> Self {
        Self {
            component_type: "Video".to_string(),
            common: ComponentCommon {
                id,
                accessibility: None,
                weight: None,
            },
            properties: json_dynamic_string("url", &url),
        }
    }

    /// 设置 Video 组件的 posterUrl 属性
    pub fn with_poster_url(mut self, poster_url: String) -> Self {
        self.properties["posterUrl"] = serde_json::json!(poster_url);
        self
    }

    /// 创建 AudioPlayer 组件
    pub fn audio_player(id: ComponentId, url: DynamicValue<String>) -> Self {
        Self {
            component_type: "AudioPlayer".to_string(),
            common: ComponentCommon {
                id,
                accessibility: None,
                weight: None,
            },
            properties: json_dynamic_string("url", &url),
        }
    }

    /// 设置 AudioPlayer 组件的 description 属性
    pub fn with_description(mut self, description: String) -> Self {
        self.properties["description"] = serde_json::json!(description);
        self
    }

    /// 创建 TextField 组件
    pub fn text_field(id: ComponentId) -> Self {
        Self {
            component_type: "TextField".to_string(),
            common: ComponentCommon {
                id,
                accessibility: None,
                weight: None,
            },
            properties: Value::Object(Default::default()),
        }
    }

    /// 设置 TextField 的 label
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.properties["label"] = Value::String(label.into());
        self
    }

    /// 设置 TextField 的 placeholder
    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.properties["placeholder"] = Value::String(placeholder.into());
        self
    }

    /// 设置 TextField 的 value (DynamicString)
    pub fn with_value(mut self, value: DynamicValue<String>) -> Self {
        self.properties["value"] = match &value {
            DynamicValue::Literal(s) => Value::String(s.clone()),
            DynamicValue::Path { path } => serde_json::json!({"path": path}),
            DynamicValue::FunctionCall { call, args } => {
                serde_json::json!({"call": call, "args": args})
            }
        };
        self
    }

    /// 设置 TextField 的 variant (shortText/number/longText/obscured)
    pub fn with_text_variant(mut self, variant: impl Into<String>) -> Self {
        self.properties["variant"] = Value::String(variant.into());
        self
    }

    /// 创建 CheckBox 组件
    pub fn check_box(id: ComponentId) -> Self {
        Self {
            component_type: "CheckBox".to_string(),
            common: ComponentCommon {
                id,
                accessibility: None,
                weight: None,
            },
            properties: Value::Object(Default::default()),
        }
    }

    /// 设置 CheckBox 的 checked/value (bool 或 DynamicBool)
    pub fn with_checked(mut self, checked: bool) -> Self {
        self.properties["value"] = Value::Bool(checked);
        self
    }

    /// 设置 CheckBox 的动态 value (DynamicBool)
    pub fn with_value_bool(mut self, value: DynamicValue<bool>) -> Self {
        self.properties["value"] = match value {
            DynamicValue::Literal(b) => Value::Bool(b),
            DynamicValue::Path { path } => serde_json::json!({"path": path}),
            DynamicValue::FunctionCall { call, args } => {
                serde_json::json!({"call": call, "args": args})
            }
        };
        self
    }

    /// 创建 ChoicePicker 组件
    pub fn choice_picker(id: ComponentId) -> Self {
        Self {
            component_type: "ChoicePicker".to_string(),
            common: ComponentCommon {
                id,
                accessibility: None,
                weight: None,
            },
            properties: Value::Object(Default::default()),
        }
    }

    /// 设置 ChoicePicker 的 options
    pub fn with_options(mut self, options: Vec<String>) -> Self {
        let arr: Vec<Value> = options.into_iter().map(Value::String).collect();
        self.properties["options"] = Value::Array(arr);
        self
    }

    /// 设置 ChoicePicker 的 selected values
    pub fn with_selected(mut self, selected: Vec<String>) -> Self {
        let arr: Vec<Value> = selected.into_iter().map(Value::String).collect();
        self.properties["value"] = Value::Array(arr);
        self
    }

    /// 设置 ChoicePicker 的 displayStyle (如 "chip", "list", "dropdown")
    pub fn with_display_style(mut self, style: impl Into<String>) -> Self {
        self.properties["displayStyle"] = Value::String(style.into());
        self
    }

    /// 创建 Slider 组件
    pub fn slider(id: ComponentId) -> Self {
        Self {
            component_type: "Slider".to_string(),
            common: ComponentCommon {
                id,
                accessibility: None,
                weight: None,
            },
            properties: Value::Object(Default::default()),
        }
    }

    /// 设置 Slider 的 min
    pub fn with_min(mut self, min: f64) -> Self {
        self.properties["min"] = serde_json::json!(min);
        self
    }

    /// 设置 Slider 的 max
    pub fn with_max(mut self, max: f64) -> Self {
        self.properties["max"] = serde_json::json!(max);
        self
    }

    /// 设置 Slider 的 value (静态 f64)
    pub fn with_value_f64(mut self, value: f64) -> Self {
        self.properties["value"] = serde_json::json!(value);
        self
    }

    /// 设置 Slider 的 value (DynamicValue<f64>)
    pub fn with_num_value(mut self, value: DynamicValue<f64>) -> Self {
        self.properties["value"] = match value {
            DynamicValue::Literal(n) => serde_json::json!(n),
            DynamicValue::Path { path } => serde_json::json!({"path": path}),
            DynamicValue::FunctionCall { call, args } => {
                serde_json::json!({"call": call, "args": args})
            }
        };
        self
    }

    /// 设置 Slider 的 steps
    pub fn with_steps(mut self, steps: i64) -> Self {
        self.properties["steps"] = serde_json::json!(steps);
        self
    }

    /// 创建 DateTimeInput 组件
    pub fn date_time_input(id: ComponentId) -> Self {
        Self {
            component_type: "DateTimeInput".to_string(),
            common: ComponentCommon {
                id,
                accessibility: None,
                weight: None,
            },
            properties: Value::Object(Default::default()),
        }
    }

    /// 设置 DateTimeInput 的 enableDate
    pub fn with_enable_date(mut self, enable: bool) -> Self {
        self.properties["enableDate"] = Value::Bool(enable);
        self
    }

    /// 设置 DateTimeInput 的 enableTime
    pub fn with_enable_time(mut self, enable: bool) -> Self {
        self.properties["enableTime"] = Value::Bool(enable);
        self
    }

    /// 设置 DateTimeInput 的 min 日期
    pub fn with_min_date(mut self, min: impl Into<String>) -> Self {
        self.properties["min"] = Value::String(min.into());
        self
    }

    /// 设置 DateTimeInput 的 max 日期
    pub fn with_max_date(mut self, max: impl Into<String>) -> Self {
        self.properties["max"] = Value::String(max.into());
        self
    }

    /// 创建 Divider 组件
    ///
    /// Divider 组件无特有属性，仅用于视觉分隔。
    pub fn divider(id: ComponentId) -> Self {
        Self {
            component_type: "Divider".to_string(),
            common: ComponentCommon {
                id,
                accessibility: None,
                weight: None,
            },
            properties: serde_json::json!({}),
        }
    }

    /// 设置模板子组件（ChildList::Object 模式）
    /// 将 Column/Row 的 children 从固定列表切换为 Data Model 模板模式
    pub fn with_template_children(
        mut self,
        template_id: ComponentId,
        path: impl Into<String>,
    ) -> Self {
        self.properties = serde_json::json!({
            "children": {
                "template": template_id.as_str(),
                "path": path.into()
            }
        });
        self
    }

    /// 获取组件 ID
    pub fn id(&self) -> &ComponentId {
        &self.common.id
    }

    /// 获取组件类型名
    pub fn component_type(&self) -> &str {
        &self.component_type
    }

    /// 获取通用属性
    pub fn common(&self) -> &crate::component::component::ComponentCommon {
        &self.common
    }

    /// 获取组件特有属性
    pub fn properties(&self) -> &Value {
        &self.properties
    }
}

// ---- Tests ----

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Existing tests for foundational types

    #[test]
    fn test_component_id_valid() {
        let id = ComponentId::new("my_button").unwrap();
        assert_eq!(id.as_str(), "my_button");
    }

    #[test]
    fn test_component_id_invalid_starts_with_number() {
        assert!(ComponentId::new("123abc").is_err());
    }

    #[test]
    fn test_component_id_invalid_contains_space() {
        assert!(ComponentId::new("my button").is_err());
    }

    #[test]
    fn test_component_id_at_namespace_reserved() {
        assert!(ComponentId::new("@custom").is_err());
    }

    #[test]
    fn test_component_id_empty() {
        assert!(ComponentId::new("").is_err());
    }

    #[test]
    fn test_component_id_display() {
        let id = ComponentId::new("root").unwrap();
        assert_eq!(format!("{}", id), "root");
    }

    #[test]
    fn test_dynamic_value_literal_string() {
        let dv: DynamicValue<String> = DynamicValue::Literal("hello".into());
        assert_eq!(dv.as_str(), Some("hello"));
    }

    #[test]
    fn test_dynamic_value_literal_number() {
        let dv: DynamicValue<i64> = DynamicValue::Literal(42);
        assert_eq!(dv.as_i64(), Some(42));
    }

    #[test]
    fn test_dynamic_value_literal_bool() {
        let dv: DynamicValue<bool> = DynamicValue::Literal(true);
        assert_eq!(dv.as_bool(), Some(true));
    }

    #[test]
    fn test_dynamic_value_path() {
        let dv: DynamicValue<String> = DynamicValue::Path {
            path: "/user/name".into(),
        };
        assert_eq!(dv.as_path(), Some("/user/name"));
    }

    #[test]
    fn test_dynamic_value_function_call() {
        let dv: DynamicValue<String> = DynamicValue::FunctionCall {
            call: "formatString".into(),
            args: json!({"template": "Hello {name}"}),
        };
        assert_eq!(dv.as_function_call(), Some("formatString"));
    }

    #[test]
    fn test_component_common_fields() {
        let common = ComponentCommon {
            id: ComponentId::new("root").unwrap(),
            accessibility: None,
            weight: None,
        };
        assert_eq!(common.id.as_str(), "root");
    }

    #[test]
    fn test_accessibility_attributes() {
        let acc = AccessibilityAttributes {
            label: Some("Submit".into()),
            description: Some("Submits form".into()),
        };
        let common = ComponentCommon {
            id: ComponentId::new("btn").unwrap(),
            accessibility: Some(acc),
            weight: Some(1.0),
        };
        assert!(common.accessibility.is_some());
        assert_eq!(common.weight, Some(1.0));
    }

    // New tests for ComponentType and Component

    #[test]
    fn test_component_text() {
        let comp = Component::text(
            ComponentId::new("greeting").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        assert_eq!(comp.id().as_str(), "greeting");
        assert_eq!(comp.component_type(), "Text");
    }

    #[test]
    fn test_component_button() {
        let comp = Component::button(
            ComponentId::new("submit").unwrap(),
            ComponentId::new("submit_label").unwrap(),
        );
        assert_eq!(comp.component_type(), "Button");
    }

    #[test]
    fn test_component_column() {
        let comp = Component::column(
            ComponentId::new("col").unwrap(),
            vec![
                ComponentId::new("a").unwrap(),
                ComponentId::new("b").unwrap(),
            ],
        );
        assert_eq!(comp.component_type(), "Column");
    }

    #[test]
    fn test_component_row() {
        let comp = Component::row(ComponentId::new("row").unwrap(), vec![]);
        assert_eq!(comp.component_type(), "Row");
    }

    #[test]
    fn test_component_with_weight() {
        let comp = Component::text(
            ComponentId::new("t").unwrap(),
            DynamicValue::Literal("hi".to_string()),
        )
        .with_weight(2.0);
        assert_eq!(comp.common().weight, Some(2.0));
    }

    #[test]
    fn test_component_deserialize() {
        let json = r#"{"id":"root","component":"Text","text":"Hello"}"#;
        let comp: Component = serde_json::from_str(json).unwrap();
        assert_eq!(comp.id().as_str(), "root");
        assert_eq!(comp.component_type(), "Text");
    }

    #[test]
    fn test_component_type_from_str() {
        assert_eq!(ComponentType::Text.as_str(), "Text");
        assert_eq!(ComponentType::Button.as_str(), "Button");
        assert_eq!(ComponentType::Column.as_str(), "Column");
    }

    #[test]
    fn test_component_id_rejects_zero_width_space() {
        // U+200B 零宽空格不是 XID_Continue，应被拒绝
        assert!(ComponentId::new("my\u{200B}button").is_err());
    }

    #[test]
    fn test_component_id_rejects_math_symbol() {
        // U+2200 ∀ 数学符号不是 XID_Continue，应被拒绝
        assert!(ComponentId::new("my∀button").is_err());
    }

    #[test]
    fn test_component_id_allows_chinese() {
        // 中文字符是 XID_Continue，应被接受
        assert!(ComponentId::new("按钮").is_ok());
    }

    #[test]
    fn test_component_id_allows_unicode_letters() {
        // 拉丁扩展字符是有效的 XID 字符
        assert!(ComponentId::new("café").is_ok());
        assert!(ComponentId::new("naïve").is_ok());
    }

    // ---- List builder tests ----

    #[test]
    fn test_component_list_array() {
        let cl = crate::component::child_list::ChildList::array(vec![
            ComponentId::new("item1").unwrap(),
            ComponentId::new("item2").unwrap(),
        ]);
        let comp = Component::list(ComponentId::new("my_list").unwrap(), cl);
        assert_eq!(comp.component_type(), "List");
        let props = comp.properties();
        assert_eq!(props["children"][0], "item1");
        assert_eq!(props["children"][1], "item2");
    }

    #[test]
    fn test_component_list_object() {
        let cl = crate::component::child_list::ChildList::object(
            ComponentId::new("tmpl").unwrap(),
            "/items",
        );
        let comp = Component::list(ComponentId::new("my_list").unwrap(), cl);
        assert_eq!(comp.component_type(), "List");
        let props = comp.properties();
        assert_eq!(props["children"]["template"], "tmpl");
        assert_eq!(props["children"]["path"], "/items");
    }

    // ---- Card builder test ----

    #[test]
    fn test_component_card() {
        let comp = Component::card(
            ComponentId::new("my_card").unwrap(),
            ComponentId::new("card_content").unwrap(),
        );
        assert_eq!(comp.component_type(), "Card");
        let props = comp.properties();
        assert_eq!(props["child"], "card_content");
    }

    // ---- Tabs builder tests ----

    #[test]
    fn test_component_tabs() {
        let tabs = vec![
            TabItem {
                title: "Tab 1".to_string(),
                child: ComponentId::new("tab1_content").unwrap(),
            },
            TabItem {
                title: "Tab 2".to_string(),
                child: ComponentId::new("tab2_content").unwrap(),
            },
        ];
        let comp = Component::tabs(ComponentId::new("my_tabs").unwrap(), tabs);
        assert_eq!(comp.component_type(), "Tabs");
        let props = comp.properties();
        assert_eq!(props["tabs"][0]["title"], "Tab 1");
        assert_eq!(props["tabs"][0]["child"], "tab1_content");
        assert_eq!(props["tabs"][1]["title"], "Tab 2");
        assert_eq!(props["tabs"][1]["child"], "tab2_content");
    }

    #[test]
    fn test_tab_item_serialize() {
        let tab = TabItem {
            title: "My Tab".to_string(),
            child: ComponentId::new("tab_content").unwrap(),
        };
        let json = serde_json::to_value(&tab).unwrap();
        assert_eq!(json["title"], "My Tab");
        assert_eq!(json["child"], "tab_content");
    }

    #[test]
    fn test_tab_item_deserialize() {
        let json = r#"{"title":"My Tab","child":"tab_content"}"#;
        let tab: TabItem = serde_json::from_str(json).unwrap();
        assert_eq!(tab.title, "My Tab");
        assert_eq!(tab.child.as_str(), "tab_content");
    }

    // ---- Modal builder test ----

    #[test]
    fn test_component_modal() {
        let comp = Component::modal(
            ComponentId::new("my_modal").unwrap(),
            ComponentId::new("modal_body").unwrap(),
            ComponentId::new("open_btn").unwrap(),
        );
        assert_eq!(comp.component_type(), "Modal");
        let props = comp.properties();
        assert_eq!(props["content"], "modal_body");
        assert_eq!(props["trigger"], "open_btn");
    }

    // ---- Column / Row template children tests ----

    #[test]
    fn test_component_column_with_template() {
        let comp = Component::column(ComponentId::new("col").unwrap(), vec![])
            .with_template_children(ComponentId::new("item_template").unwrap(), "/items");
        assert_eq!(comp.component_type(), "Column");
        let props = comp.properties();
        assert_eq!(props["children"]["template"], "item_template");
        assert_eq!(props["children"]["path"], "/items");
    }

    #[test]
    fn test_component_row_with_template() {
        let comp = Component::row(ComponentId::new("row").unwrap(), vec![])
            .with_template_children(ComponentId::new("item_template").unwrap(), "/data");
        assert_eq!(comp.component_type(), "Row");
        let props = comp.properties();
        assert_eq!(props["children"]["template"], "item_template");
        assert_eq!(props["children"]["path"], "/data");
    }

    // ---- Image builder tests ----

    #[test]
    fn test_component_image_minimal() {
        let comp = Component::image(
            ComponentId::new("img").unwrap(),
            DynamicValue::Literal("https://example.com/photo.png".to_string()),
        );
        assert_eq!(comp.component_type(), "Image");
        assert_eq!(comp.properties()["url"], "https://example.com/photo.png");
    }

    #[test]
    fn test_component_image_with_builders() {
        let comp = Component::image(
            ComponentId::new("img").unwrap(),
            DynamicValue::Literal("https://example.com/photo.png".to_string()),
        )
        .with_fit("cover".to_string())
        .with_variant("rounded".to_string());
        assert_eq!(comp.component_type(), "Image");
        assert_eq!(comp.properties()["url"], "https://example.com/photo.png");
        assert_eq!(comp.properties()["fit"], "cover");
        assert_eq!(comp.properties()["variant"], "rounded");
    }

    #[test]
    fn test_component_image_dynamic_url() {
        let comp = Component::image(
            ComponentId::new("img").unwrap(),
            DynamicValue::Path {
                path: "/user/avatar".to_string(),
            },
        );
        assert_eq!(comp.component_type(), "Image");
        assert_eq!(comp.properties()["url"]["path"], "/user/avatar");
    }

    #[test]
    fn test_component_image_roundtrip() {
        let comp = Component::image(
            ComponentId::new("img").unwrap(),
            DynamicValue::Literal("https://example.com/photo.png".to_string()),
        )
        .with_fit("cover".to_string())
        .with_variant("rounded".to_string());
        let json = serde_json::to_string(&comp).unwrap();
        let deserialized: Component = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.component_type(), "Image");
        assert_eq!(
            deserialized.properties()["url"],
            "https://example.com/photo.png"
        );
        assert_eq!(deserialized.properties()["fit"], "cover");
    }

    // ---- Icon builder tests ----

    #[test]
    fn test_component_icon() {
        let comp = Component::icon(
            ComponentId::new("icon").unwrap(),
            DynamicValue::Literal("star".to_string()),
        );
        assert_eq!(comp.component_type(), "Icon");
        assert_eq!(comp.properties()["name"], "star");
    }

    #[test]
    fn test_component_icon_dynamic() {
        let comp = Component::icon(
            ComponentId::new("icon").unwrap(),
            DynamicValue::Path {
                path: "/theme/icon".to_string(),
            },
        );
        assert_eq!(comp.component_type(), "Icon");
        assert_eq!(comp.properties()["name"]["path"], "/theme/icon");
    }

    #[test]
    fn test_component_icon_roundtrip() {
        let comp = Component::icon(
            ComponentId::new("icon").unwrap(),
            DynamicValue::Literal("star".to_string()),
        );
        let json = serde_json::to_string(&comp).unwrap();
        let deserialized: Component = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.component_type(), "Icon");
        assert_eq!(deserialized.properties()["name"], "star");
    }

    // ---- Video builder tests ----

    #[test]
    fn test_component_video_minimal() {
        let comp = Component::video(
            ComponentId::new("vid").unwrap(),
            DynamicValue::Literal("https://example.com/video.mp4".to_string()),
        );
        assert_eq!(comp.component_type(), "Video");
        assert_eq!(comp.properties()["url"], "https://example.com/video.mp4");
        assert!(comp.properties().get("posterUrl").is_none());
    }

    #[test]
    fn test_component_video_with_builder() {
        let comp = Component::video(
            ComponentId::new("vid").unwrap(),
            DynamicValue::Literal("https://example.com/video.mp4".to_string()),
        )
        .with_poster_url("https://example.com/poster.jpg".to_string());
        assert_eq!(comp.component_type(), "Video");
        assert_eq!(comp.properties()["url"], "https://example.com/video.mp4");
        assert_eq!(
            comp.properties()["posterUrl"],
            "https://example.com/poster.jpg"
        );
    }

    #[test]
    fn test_component_video_dynamic_url() {
        let comp = Component::video(
            ComponentId::new("vid").unwrap(),
            DynamicValue::FunctionCall {
                call: "getUrl".to_string(),
                args: json!({"id": "intro"}),
            },
        );
        assert_eq!(comp.component_type(), "Video");
        assert_eq!(comp.properties()["url"]["call"], "getUrl");
        assert_eq!(comp.properties()["url"]["args"]["id"], "intro");
    }

    #[test]
    fn test_component_video_roundtrip() {
        let comp = Component::video(
            ComponentId::new("vid").unwrap(),
            DynamicValue::Literal("https://example.com/video.mp4".to_string()),
        )
        .with_poster_url("poster.jpg".to_string());
        let json = serde_json::to_string(&comp).unwrap();
        let deserialized: Component = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.component_type(), "Video");
        assert_eq!(
            deserialized.properties()["url"],
            "https://example.com/video.mp4"
        );
        assert_eq!(deserialized.properties()["posterUrl"], "poster.jpg");
    }

    // ---- AudioPlayer builder tests ----

    #[test]
    fn test_component_audio_player_minimal() {
        let comp = Component::audio_player(
            ComponentId::new("audio").unwrap(),
            DynamicValue::Literal("https://example.com/song.mp3".to_string()),
        );
        assert_eq!(comp.component_type(), "AudioPlayer");
        assert_eq!(comp.properties()["url"], "https://example.com/song.mp3");
        assert!(comp.properties().get("description").is_none());
    }

    #[test]
    fn test_component_audio_player_with_builder() {
        let comp = Component::audio_player(
            ComponentId::new("audio").unwrap(),
            DynamicValue::Literal("https://example.com/song.mp3".to_string()),
        )
        .with_description("A great song".to_string());
        assert_eq!(comp.component_type(), "AudioPlayer");
        assert_eq!(comp.properties()["url"], "https://example.com/song.mp3");
        assert_eq!(comp.properties()["description"], "A great song");
    }

    #[test]
    fn test_component_audio_player_dynamic_url() {
        let comp = Component::audio_player(
            ComponentId::new("audio").unwrap(),
            DynamicValue::Path {
                path: "/player/current".to_string(),
            },
        );
        assert_eq!(comp.component_type(), "AudioPlayer");
        assert_eq!(comp.properties()["url"]["path"], "/player/current");
    }

    #[test]
    fn test_component_audio_player_roundtrip() {
        let comp = Component::audio_player(
            ComponentId::new("audio").unwrap(),
            DynamicValue::Literal("https://example.com/song.mp3".to_string()),
        )
        .with_description("Background music".to_string());
        let json = serde_json::to_string(&comp).unwrap();
        let deserialized: Component = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.component_type(), "AudioPlayer");
        assert_eq!(
            deserialized.properties()["url"],
            "https://example.com/song.mp3"
        );
        assert_eq!(deserialized.properties()["description"], "Background music");
    }

    // ---- Divider builder tests ----

    #[test]
    fn test_component_divider() {
        let comp = Component::divider(ComponentId::new("div").unwrap());
        assert_eq!(comp.component_type(), "Divider");
        assert!(comp
            .properties()
            .as_object()
            .map_or(false, |m| m.is_empty()));
    }

    #[test]
    fn test_component_divider_roundtrip() {
        let comp = Component::divider(ComponentId::new("div").unwrap());
        let json = serde_json::to_string(&comp).unwrap();
        let deserialized: Component = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.component_type(), "Divider");
    }

    // ---- TextField builder tests ----

    #[test]
    fn test_component_text_field() {
        let comp = Component::text_field(ComponentId::new("tf").unwrap())
            .with_label("用户名")
            .with_placeholder("请输入")
            .with_value(DynamicValue::<String>::path("/form/username"))
            .with_text_variant("shortText");
        assert_eq!(comp.component_type(), "TextField");
        let p = comp.properties();
        assert_eq!(p["label"], "用户名");
        assert_eq!(p["placeholder"], "请输入");
        assert_eq!(p["value"]["path"], "/form/username");
        assert_eq!(p["variant"], "shortText");
    }

    #[test]
    fn test_component_text_field_obscured() {
        let comp = Component::text_field(ComponentId::new("pwd").unwrap())
            .with_label("密码")
            .with_text_variant("obscured");
        assert_eq!(comp.properties()["variant"], "obscured");
    }

    // ---- CheckBox builder tests ----

    #[test]
    fn test_component_check_box() {
        let comp = Component::check_box(ComponentId::new("cb").unwrap())
            .with_label("记住密码")
            .with_checked(true);
        assert_eq!(comp.component_type(), "CheckBox");
        let p = comp.properties();
        assert_eq!(p["label"], "记住密码");
        assert_eq!(p["value"], true);
    }

    #[test]
    fn test_component_check_box_dynamic_value() {
        let comp = Component::check_box(ComponentId::new("cb").unwrap())
            .with_label("同意条款")
            .with_value_bool(DynamicValue::<bool>::Path {
                path: "/form/agree".to_string(),
            });
        let p = comp.properties();
        assert_eq!(p["value"]["path"], "/form/agree");
    }

    // ---- ChoicePicker builder tests ----

    #[test]
    fn test_component_choice_picker() {
        let comp = Component::choice_picker(ComponentId::new("cp").unwrap())
            .with_options(vec!["A".into(), "B".into(), "C".into()])
            .with_selected(vec!["A".into()]);
        assert_eq!(comp.component_type(), "ChoicePicker");
        let p = comp.properties();
        assert_eq!(p["options"][0], "A");
        assert_eq!(p["options"][2], "C");
        assert_eq!(p["value"][0], "A");
    }

    // ---- Slider builder tests ----

    #[test]
    fn test_component_slider() {
        let comp = Component::slider(ComponentId::new("sl").unwrap())
            .with_min(0.0)
            .with_max(100.0)
            .with_value_f64(50.0)
            .with_label("音量");
        assert_eq!(comp.component_type(), "Slider");
        let p = comp.properties();
        assert_eq!(p["min"], 0.0);
        assert_eq!(p["max"], 100.0);
        assert_eq!(p["value"], 50.0);
        assert_eq!(p["label"], "音量");
    }

    // ---- DateTimeInput builder tests ----

    #[test]
    fn test_component_date_time_input() {
        let comp = Component::date_time_input(ComponentId::new("dt").unwrap())
            .with_label("选择日期")
            .with_enable_date(true)
            .with_enable_time(false);
        assert_eq!(comp.component_type(), "DateTimeInput");
        let p = comp.properties();
        assert_eq!(p["label"], "选择日期");
        assert_eq!(p["enableDate"], true);
        assert_eq!(p["enableTime"], false);
    }

    // ---- DynamicValue convenience constructors (Task A) ----

    #[test]
    fn test_dynamic_value_convenience_constructors() {
        // DynamicValue::literal()
        let lit: DynamicValue<String> = DynamicValue::literal("hello");
        assert_eq!(lit, DynamicValue::Literal("hello".to_string()));
        assert_eq!(lit.as_str(), Some("hello"));

        // DynamicValue::path()
        let p: DynamicValue<String> = DynamicValue::path("/user/name");
        assert_eq!(
            p,
            DynamicValue::Path {
                path: "/user/name".to_string()
            }
        );
        assert_eq!(p.as_path(), Some("/user/name"));

        // DynamicValue::call()
        let args = json!({"template": "Hello {name}"});
        let call: DynamicValue<String> = DynamicValue::call("fmt", args.clone());
        assert_eq!(
            call,
            DynamicValue::FunctionCall {
                call: "fmt".to_string(),
                args,
            }
        );
        assert_eq!(call.as_function_call(), Some("fmt"));
    }

    // ---- TextField serialization (Task B) ----

    #[test]
    fn test_component_text_field_builder() {
        let tf = Component::text_field(ComponentId::new("name").unwrap())
            .with_label("用户名")
            .with_placeholder("请输入")
            .with_value(DynamicValue::literal("张三"))
            .with_text_variant("shortText");
        assert_eq!(tf.component_type(), "TextField");
        let json_val = serde_json::to_value(&tf).unwrap();
        assert_eq!(json_val["label"], "用户名");
        assert_eq!(json_val["placeholder"], "请输入");
        assert_eq!(json_val["value"], "张三");
        assert_eq!(json_val["variant"], "shortText");
    }

    #[test]
    fn test_component_text_field_path_value() {
        let tf = Component::text_field(ComponentId::new("email").unwrap())
            .with_value(DynamicValue::path("/user/email"));
        let json_val = serde_json::to_value(&tf).unwrap();
        assert_eq!(json_val["value"]["path"], "/user/email");
    }

    // ---- CheckBox builder (Task B) ----

    #[test]
    fn test_component_check_box_builder() {
        let cb = Component::check_box(ComponentId::new("agree").unwrap())
            .with_checked(true)
            .with_label("同意条款");
        assert_eq!(cb.component_type(), "CheckBox");
        let json_val = serde_json::to_value(&cb).unwrap();
        assert_eq!(json_val["value"], true);
        assert_eq!(json_val["label"], "同意条款");
    }

    // ---- ChoicePicker display_style (Task B) ----
    // 以下测试会使⽤尚未实现的 with_display_style 方法 (RED)

    #[test]
    fn test_component_choice_picker_display_style() {
        let cp = Component::choice_picker(ComponentId::new("cp").unwrap())
            .with_options(vec!["A".into(), "B".into()])
            .with_selected(vec!["A".into()])
            .with_display_style("chip");
        assert_eq!(cp.component_type(), "ChoicePicker");
        let json_val = serde_json::to_value(&cp).unwrap();
        assert_eq!(json_val["options"], json!(["A", "B"]));
        assert_eq!(json_val["value"], json!(["A"]));
        assert_eq!(json_val["displayStyle"], "chip");
    }

    // ---- Slider steps (Task B) ----
    // 以下测试会使用尚未实现的 with_steps / with_num_value 方法 (RED)

    #[test]
    fn test_component_slider_with_steps() {
        let sl = Component::slider(ComponentId::new("vol").unwrap())
            .with_min(0.0)
            .with_max(100.0)
            .with_num_value(DynamicValue::Literal(50.0))
            .with_steps(10)
            .with_label("音量");
        assert_eq!(sl.component_type(), "Slider");
        let json_val = serde_json::to_value(&sl).unwrap();
        assert_eq!(json_val["min"], 0.0);
        assert_eq!(json_val["max"], 100.0);
        assert_eq!(json_val["value"], 50.0);
        assert_eq!(json_val["steps"], 10);
        assert_eq!(json_val["label"], "音量");
    }

    // ---- DateTimeInput min/max (Task B) ----
    // 以下测试会使用尚未实现的 with_min_date / with_max_date 方法 (RED)

    #[test]
    fn test_component_date_time_input_with_min_max() {
        let dt = Component::date_time_input(ComponentId::new("appt").unwrap())
            .with_label("预约时间")
            .with_enable_date(true)
            .with_enable_time(true)
            .with_min_date("2024-01-01")
            .with_max_date("2024-12-31");
        assert_eq!(dt.component_type(), "DateTimeInput");
        let json_val = serde_json::to_value(&dt).unwrap();
        assert_eq!(json_val["label"], "预约时间");
        assert_eq!(json_val["enableDate"], true);
        assert_eq!(json_val["enableTime"], true);
        assert_eq!(json_val["min"], "2024-01-01");
        assert_eq!(json_val["max"], "2024-12-31");
    }
}
