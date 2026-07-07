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
///
/// 反序列化经 `try_from = "String"` 走 [`ComponentId::new`] 的完整校验，
/// 保证协议输入与手动构造两条路径的约束一致（newtype 序列化天然透明）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct ComponentId(String);

impl TryFrom<String> for ComponentId {
    type Error = crate::error::A2uiError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

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
///
/// untagged 枚举按声明顺序尝试匹配：结构化的 Path/FunctionCall 必须排在
/// Literal 之前——当 `T = Value`（默认）时 `Literal(Value)` 能贪婪匹配任意
/// JSON，若其在前则 `{"path": ...}` / `{"call": ...}` 永远解析不出绑定形式。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DynamicValue<T = Value> {
    /// 绑定到 Data Model 的路径
    Path { path: String },
    /// 调用注册函数
    FunctionCall { call: String, args: Value },
    /// 字面量值
    Literal(T),
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
    ///
    /// 逃生门：任意自定义 Catalog 的 props 都可经此读取。
    /// 新代码优先使用类型化访问器（`prop_*` 系列）与结构化视图
    /// （`children_decl` / `tabs_decl` / `action_decl` / `style_decl`）。
    pub fn properties(&self) -> &Value {
        &self.properties
    }

    // ---- 协议形态构造器 ----

    /// 从协议 JSON 值构造组件（`{"component": ..., "id": ..., <props>}`）。
    ///
    /// 下游 crate（渲染器测试、示例）经此构造任意 props 的组件，
    /// 无需直接依赖 `serde_json`。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use a2ui_core::component::component::Component;
    /// use a2ui_core::prelude::json;
    ///
    /// let comp = Component::from_value(json!({
    ///     "component": "Text", "id": "t1", "text": "hello"
    /// })).unwrap();
    /// assert_eq!(comp.component_type(), "Text");
    /// ```
    pub fn from_value(value: Value) -> crate::error::Result<Self> {
        Ok(serde_json::from_value(value)?)
    }

    /// 从协议 JSON 字符串构造组件（[`Component::from_value`] 的字符串版）。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use a2ui_core::component::component::Component;
    ///
    /// let comp = Component::from_json(
    ///     r#"{"component":"Button","id":"b1","child":"lbl"}"#
    /// ).unwrap();
    /// assert_eq!(comp.component_type(), "Button");
    /// ```
    pub fn from_json(json: &str) -> crate::error::Result<Self> {
        Ok(serde_json::from_str(json)?)
    }

    // ---- 类型化 props 访问器（裸标量） ----

    /// 读取字符串 prop（不含动态语义的键：`variant` / `title` 等）。
    ///
    /// 键缺失或值非字符串时返回 `None`（宽容语义，对齐 `.as_str()`）。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use a2ui_core::component::component::Component;
    /// use serde::Deserialize;
    /// use serde_json::json;
    ///
    /// let c = Component::deserialize(json!({
    ///     "component": "Button", "id": "b1", "variant": "primary"
    /// })).unwrap();
    /// assert_eq!(c.prop_str("variant"), Some("primary"));
    /// assert_eq!(c.prop_str("missing"), None);
    /// ```
    pub fn prop_str(&self, key: &str) -> Option<&str> {
        self.properties.get(key)?.as_str()
    }

    /// 读取布尔 prop。键缺失或值非布尔时返回 `None`。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use a2ui_core::component::component::Component;
    /// use serde::Deserialize;
    /// use serde_json::json;
    ///
    /// let c = Component::deserialize(json!({
    ///     "component": "CheckBox", "id": "cb", "checked": true
    /// })).unwrap();
    /// assert_eq!(c.prop_bool("checked"), Some(true));
    /// ```
    pub fn prop_bool(&self, key: &str) -> Option<bool> {
        self.properties.get(key)?.as_bool()
    }

    /// 读取数值 prop（整数与浮点均按 `f64` 给出）。键缺失或值非数值时返回 `None`。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use a2ui_core::component::component::Component;
    /// use serde::Deserialize;
    /// use serde_json::json;
    ///
    /// let c = Component::deserialize(json!({
    ///     "component": "Slider", "id": "s", "min": 0, "max": 100.5
    /// })).unwrap();
    /// assert_eq!(c.prop_f64("min"), Some(0.0));
    /// assert_eq!(c.prop_f64("max"), Some(100.5));
    /// ```
    pub fn prop_f64(&self, key: &str) -> Option<f64> {
        self.properties.get(key)?.as_f64()
    }

    /// 读取字符串数组 prop（如 ChoicePicker 的 `options` / `value`）。
    ///
    /// 键缺失或值非数组时返回 `None`；数组内的非字符串项被**静默过滤**
    /// （对齐现有渲染器行为，含 `{"path": ...}` 包装项）。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use a2ui_core::component::component::Component;
    /// use serde::Deserialize;
    /// use serde_json::json;
    ///
    /// let c = Component::deserialize(json!({
    ///     "component": "ChoicePicker", "id": "cp", "options": ["A", "B", 3]
    /// })).unwrap();
    /// assert_eq!(c.prop_str_list("options"), Some(vec!["A", "B"]));
    /// ```
    pub fn prop_str_list(&self, key: &str) -> Option<Vec<&str>> {
        let arr = self.properties.get(key)?.as_array()?;
        Some(arr.iter().filter_map(|v| v.as_str()).collect())
    }

    /// 读取组件 ID 引用 prop（`child` / `content` / `trigger` 等）。
    ///
    /// 键缺失、值非字符串或不满足 ComponentId 命名规则时返回 `None`。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use a2ui_core::component::component::Component;
    /// use a2ui_core::ComponentId;
    /// use serde::Deserialize;
    /// use serde_json::json;
    ///
    /// let c = Component::deserialize(json!({
    ///     "component": "Card", "id": "card", "child": "child_1"
    /// })).unwrap();
    /// assert_eq!(
    ///     c.prop_component_id("child"),
    ///     Some(ComponentId::new("child_1").unwrap())
    /// );
    /// ```
    pub fn prop_component_id(&self, key: &str) -> Option<ComponentId> {
        ComponentId::new(self.prop_str(key)?).ok()
    }

    // ---- 类型化 props 访问器（动态值） ----

    /// 读取动态字符串 prop（值可为字面量 / `{"path": ...}` / `{"call": ...}`）。
    ///
    /// 只做「JSON 形态 → 类型形态」的转换，**不做路径求值**——求值需要
    /// 数据绑定上下文，由 `a2ui-renderer` 负责。形态不符时返回 `None`。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use a2ui_core::component::component::{Component, DynamicValue};
    /// use serde::Deserialize;
    /// use serde_json::json;
    ///
    /// let c = Component::deserialize(json!({
    ///     "component": "Text", "id": "t", "text": {"path": "/user/name"}
    /// })).unwrap();
    /// assert_eq!(
    ///     c.prop_dynamic_str("text"),
    ///     Some(DynamicValue::Path { path: "/user/name".into() })
    /// );
    /// ```
    pub fn prop_dynamic_str(&self, key: &str) -> Option<DynamicValue<String>> {
        self.prop_dynamic(key)
    }

    /// 读取动态布尔 prop（如 CheckBox 的 `value`）。形态不符时返回 `None`。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use a2ui_core::component::component::{Component, DynamicValue};
    /// use serde::Deserialize;
    /// use serde_json::json;
    ///
    /// let c = Component::deserialize(json!({
    ///     "component": "CheckBox", "id": "cb", "value": true
    /// })).unwrap();
    /// assert_eq!(c.prop_dynamic_bool("value"), Some(DynamicValue::Literal(true)));
    /// ```
    pub fn prop_dynamic_bool(&self, key: &str) -> Option<DynamicValue<bool>> {
        self.prop_dynamic(key)
    }

    /// 读取动态数值 prop（如 Slider 的 `value`）。形态不符时返回 `None`。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use a2ui_core::component::component::{Component, DynamicValue};
    /// use serde::Deserialize;
    /// use serde_json::json;
    ///
    /// let c = Component::deserialize(json!({
    ///     "component": "Slider", "id": "s", "value": {"path": "/volume"}
    /// })).unwrap();
    /// assert_eq!(
    ///     c.prop_dynamic_f64("value"),
    ///     Some(DynamicValue::Path { path: "/volume".into() })
    /// );
    /// ```
    pub fn prop_dynamic_f64(&self, key: &str) -> Option<DynamicValue<f64>> {
        self.prop_dynamic(key)
    }

    /// 读取动态 prop 的通用形态（`T = Value`，字面量兜底任意 JSON）。
    ///
    /// 与 [`Component::prop_dynamic_str`] 不同，非字符串字面量（数字、
    /// 布尔等）也会以 `Literal` 给出——渲染层「任意字面量按显示文本渲染」
    /// 的现状语义依赖此形态。仅键缺失时返回 `None`。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use a2ui_core::component::component::{Component, DynamicValue};
    /// use serde::Deserialize;
    /// use serde_json::json;
    ///
    /// let c = Component::deserialize(json!({
    ///     "component": "Text", "id": "t", "text": 42
    /// })).unwrap();
    /// assert_eq!(
    ///     c.prop_dynamic_value("text"),
    ///     Some(DynamicValue::Literal(json!(42)))
    /// );
    /// ```
    pub fn prop_dynamic_value(&self, key: &str) -> Option<DynamicValue> {
        self.prop_dynamic(key)
    }

    /// `prop_dynamic_*` 的共同实现：untagged 顺序（Path → FunctionCall →
    /// Literal）由 [`DynamicValue`] 定义保障；解析失败一律返回 `None`
    /// （与手写 `.as_*()` 分支的宽容语义对齐，不新增报错路径）。
    fn prop_dynamic<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<DynamicValue<T>> {
        let value = self.properties.get(key)?;
        serde_json::from_value(value.clone()).ok()
    }
}

// ---- props 键名常量 ----

/// 组件 props 的键名常量（收录规范 Basic Catalog 中被渲染器消费的键全集）。
///
/// 渲染器应引用这些常量而非裸字符串，规避方案 A（开放键访问器）的拼写风险。
///
/// # 示例
///
/// ```rust
/// use a2ui_core::component::component::{prop_keys, Component};
/// use serde::Deserialize;
/// use serde_json::json;
///
/// let c = Component::deserialize(json!({
///     "component": "Text", "id": "t", "text": "hello"
/// })).unwrap();
/// assert_eq!(c.prop_str(prop_keys::TEXT), Some("hello"));
/// ```
pub mod prop_keys {
    /// Text / Button 的展示文本（动态）
    pub const TEXT: &str = "text";
    /// CheckBox / DateTimeInput / ChoicePicker / Slider / TextField 的标签（动态）
    pub const LABEL: &str = "label";
    /// TextField（string）/ CheckBox（bool）/ Slider（number）/ ChoicePicker（array）的值
    pub const VALUE: &str = "value";
    /// CheckBox 的历史键（非规范键；TUI 唯一来源，egui/web 兜底）
    pub const CHECKED: &str = "checked";
    /// Slider / DateTimeInput 的下界
    pub const MIN: &str = "min";
    /// Slider / DateTimeInput 的上界
    pub const MAX: &str = "max";
    /// TextField 的占位文本（动态）
    pub const PLACEHOLDER: &str = "placeholder";
    /// Button / TextField / Text 的变体
    pub const VARIANT: &str = "variant";
    /// Image / Video / AudioPlayer 的资源地址（动态）
    pub const URL: &str = "url";
    /// Icon 的图标名（动态）
    pub const NAME: &str = "name";
    /// AudioPlayer 的描述文本（动态）
    pub const DESCRIPTION: &str = "description";
    /// Image 的宽度（number 或 "fill"/"shrink"）
    pub const WIDTH: &str = "width";
    /// Image 的高度（number 或 "fill"/"shrink"）
    pub const HEIGHT: &str = "height";
    /// Button / Card 的子组件 ID 引用
    pub const CHILD: &str = "child";
    /// Row / Column / List 的子组件列表（数组或 {template, path} 模板）
    pub const CHILDREN: &str = "children";
    /// Modal 的内容组件 ID 引用
    pub const CONTENT: &str = "content";
    /// Modal 的触发组件 ID 引用
    pub const TRIGGER: &str = "trigger";
    /// Modal 的标题
    pub const TITLE: &str = "title";
    /// Tabs 的标签页声明数组
    pub const TABS: &str = "tabs";
    /// ChoicePicker 的候选项数组
    pub const OPTIONS: &str = "options";
    /// 可交互组件的 action 声明（`action.event.*`）
    pub const ACTION: &str = "action";
    /// 通用样式对象
    pub const STYLE: &str = "style";
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
    fn test_component_id_deserialize_valid() {
        let id: ComponentId = serde_json::from_str(r#""my_button""#).unwrap();
        assert_eq!(id.as_str(), "my_button");
    }

    #[test]
    fn test_component_id_deserialize_rejects_invalid() {
        // 反序列化必须走与 ComponentId::new 相同的校验，不能被 serde 旁路
        assert!(serde_json::from_str::<ComponentId>(r#""@system""#).is_err());
        assert!(serde_json::from_str::<ComponentId>(r#""123 bad id""#).is_err());
        assert!(serde_json::from_str::<ComponentId>(r#""""#).is_err());
        assert!(serde_json::from_str::<ComponentId>(r#""has space""#).is_err());
    }

    #[test]
    fn test_component_deserialize_rejects_invalid_id() {
        // 协议消息入口：Component 携带非法 id 时整体反序列化失败
        let json = r#"{"id":"@system","component":"Text","text":"hi"}"#;
        assert!(serde_json::from_str::<Component>(json).is_err());
    }

    #[test]
    fn test_component_id_serialize_stays_transparent() {
        let id = ComponentId::new("root").unwrap();
        assert_eq!(serde_json::to_string(&id).unwrap(), r#""root""#);
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
    fn test_dynamic_value_default_deserialize_path() {
        // T = Value（默认类型参数）时，{"path": ...} 必须解析为 Path 而非 Literal
        let dv: DynamicValue = serde_json::from_str(r#"{"path":"/user/name"}"#).unwrap();
        assert_eq!(
            dv,
            DynamicValue::Path {
                path: "/user/name".into()
            }
        );
    }

    #[test]
    fn test_dynamic_value_default_deserialize_function_call() {
        let dv: DynamicValue =
            serde_json::from_str(r#"{"call":"formatString","args":{"t":"hi"}}"#).unwrap();
        assert_eq!(
            dv,
            DynamicValue::FunctionCall {
                call: "formatString".into(),
                args: json!({"t": "hi"}),
            }
        );
    }

    #[test]
    fn test_dynamic_value_default_deserialize_literal() {
        // 普通对象（不含 path/call 结构）仍应解析为 Literal
        let dv: DynamicValue = serde_json::from_str(r#"{"name":"Alice","age":30}"#).unwrap();
        assert_eq!(dv, DynamicValue::Literal(json!({"name":"Alice","age":30})));
        let dv: DynamicValue = serde_json::from_str(r#""plain string""#).unwrap();
        assert_eq!(dv, DynamicValue::Literal(json!("plain string")));
        let dv: DynamicValue = serde_json::from_str("42").unwrap();
        assert_eq!(dv, DynamicValue::Literal(json!(42)));
    }

    #[test]
    fn test_dynamic_value_string_deserialize_roundtrip() {
        // T = String 的既有行为不受变体顺序调整影响
        let dv: DynamicValue<String> = serde_json::from_str(r#"{"path":"/a"}"#).unwrap();
        assert_eq!(dv.as_path(), Some("/a"));
        let dv: DynamicValue<String> = serde_json::from_str(r#""hello""#).unwrap();
        assert_eq!(dv.as_str(), Some("hello"));
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

// ---- 类型化 props 访问器测试 ----

#[cfg(test)]
mod prop_accessor_tests {
    use super::*;
    use serde_json::json;

    /// 经协议反序列化路径构造组件（与线路输入同形）
    fn component(mut props: Value) -> Component {
        let obj = props.as_object_mut().expect("props must be an object");
        obj.insert("component".into(), json!("Test"));
        obj.insert("id".into(), json!("c1"));
        serde_json::from_value(Value::Object(obj.clone())).expect("valid component")
    }

    // ---- 裸标量 ----

    #[test]
    fn prop_str_reads_string_and_rejects_other_types() {
        let c = component(json!({"variant": "primary", "num": 3}));
        assert_eq!(c.prop_str("variant"), Some("primary"));
        assert_eq!(c.prop_str("num"), None); // 类型不符 → None（对齐 .as_str() 宽容语义）
        assert_eq!(c.prop_str("missing"), None);
    }

    #[test]
    fn prop_bool_reads_bool_and_rejects_other_types() {
        let c = component(json!({"checked": true, "s": "true"}));
        assert_eq!(c.prop_bool("checked"), Some(true));
        assert_eq!(c.prop_bool("s"), None);
        assert_eq!(c.prop_bool("missing"), None);
    }

    #[test]
    fn prop_f64_reads_int_and_float_and_rejects_other_types() {
        let c = component(json!({"min": 5, "max": 0.5, "s": "3"}));
        assert_eq!(c.prop_f64("min"), Some(5.0));
        assert_eq!(c.prop_f64("max"), Some(0.5));
        assert_eq!(c.prop_f64("s"), None);
        assert_eq!(c.prop_f64("missing"), None);
    }

    #[test]
    fn prop_str_list_filters_non_string_entries_silently() {
        // 对齐现状：options 解析对非字符串项静默过滤（含 {"path":...} 包装）
        let c = component(json!({
            "options": ["A", "B"],
            "mixed": ["A", 3, {"path": "/x"}],
            "notArray": "A"
        }));
        assert_eq!(c.prop_str_list("options"), Some(vec!["A", "B"]));
        assert_eq!(c.prop_str_list("mixed"), Some(vec!["A"]));
        assert_eq!(c.prop_str_list("notArray"), None);
        assert_eq!(c.prop_str_list("missing"), None);
    }

    #[test]
    fn prop_component_id_validates_id_rules() {
        let c = component(json!({"child": "child_1", "bad": "9bad", "num": 3}));
        assert_eq!(
            c.prop_component_id("child"),
            Some(ComponentId::new("child_1").unwrap())
        );
        assert_eq!(c.prop_component_id("bad"), None); // 非法 ID → None
        assert_eq!(c.prop_component_id("num"), None);
        assert_eq!(c.prop_component_id("missing"), None);
    }

    // ---- 动态值四象限：字面量 / path / call / 类型不符 ----

    #[test]
    fn prop_dynamic_str_four_quadrants() {
        let c = component(json!({
            "lit": "hello",
            "bound": {"path": "/user/name"},
            "called": {"call": "fmt", "args": {"x": 1}},
            "mismatch": 3,
            "badPath": {"path": 3}
        }));
        assert_eq!(
            c.prop_dynamic_str("lit"),
            Some(DynamicValue::Literal("hello".to_string()))
        );
        assert_eq!(
            c.prop_dynamic_str("bound"),
            Some(DynamicValue::Path {
                path: "/user/name".to_string()
            })
        );
        assert_eq!(
            c.prop_dynamic_str("called"),
            Some(DynamicValue::FunctionCall {
                call: "fmt".to_string(),
                args: json!({"x": 1})
            })
        );
        // 类型不符 → None（现状 .as_str() 失败即 None，不新增报错路径）
        assert_eq!(c.prop_dynamic_str("mismatch"), None);
        assert_eq!(c.prop_dynamic_str("badPath"), None);
        assert_eq!(c.prop_dynamic_str("missing"), None);
    }

    #[test]
    fn prop_dynamic_bool_four_quadrants() {
        let c = component(json!({
            "lit": true,
            "bound": {"path": "/form/agree"},
            "mismatch": "true",
            "callNoArgs": {"call": "f"}
        }));
        assert_eq!(
            c.prop_dynamic_bool("lit"),
            Some(DynamicValue::Literal(true))
        );
        assert_eq!(
            c.prop_dynamic_bool("bound"),
            Some(DynamicValue::Path {
                path: "/form/agree".to_string()
            })
        );
        assert_eq!(c.prop_dynamic_bool("mismatch"), None);
        // {"call":..} 缺 args：现状手写分支同样解析不出 → None
        assert_eq!(c.prop_dynamic_bool("callNoArgs"), None);
    }

    #[test]
    fn prop_dynamic_f64_four_quadrants() {
        let c = component(json!({
            "int": 5,
            "float": 0.5,
            "bound": {"path": "/volume"},
            "mismatch": true
        }));
        assert_eq!(c.prop_dynamic_f64("int"), Some(DynamicValue::Literal(5.0)));
        assert_eq!(
            c.prop_dynamic_f64("float"),
            Some(DynamicValue::Literal(0.5))
        );
        assert_eq!(
            c.prop_dynamic_f64("bound"),
            Some(DynamicValue::Path {
                path: "/volume".to_string()
            })
        );
        assert_eq!(c.prop_dynamic_f64("mismatch"), None);
    }

    #[test]
    fn prop_dynamic_value_wraps_any_literal_shape() {
        // T = Value 的动态访问器：字面量兜底任意 JSON（含非字符串），
        // 支撑渲染层「数字/布尔字面量按显示文本渲染」的现状语义
        let c = component(json!({
            "num": 3,
            "s": "x",
            "bound": {"path": "/p"},
            "called": {"call": "f", "args": []}
        }));
        assert_eq!(
            c.prop_dynamic_value("num"),
            Some(DynamicValue::Literal(json!(3)))
        );
        assert_eq!(
            c.prop_dynamic_value("s"),
            Some(DynamicValue::Literal(json!("x")))
        );
        assert_eq!(
            c.prop_dynamic_value("bound"),
            Some(DynamicValue::Path {
                path: "/p".to_string()
            })
        );
        assert_eq!(
            c.prop_dynamic_value("called"),
            Some(DynamicValue::FunctionCall {
                call: "f".to_string(),
                args: json!([])
            })
        );
        assert_eq!(c.prop_dynamic_value("missing"), None);
    }

    #[test]
    fn prop_keys_constants_match_wire_names() {
        assert_eq!(prop_keys::TEXT, "text");
        assert_eq!(prop_keys::CHILDREN, "children");
        assert_eq!(prop_keys::ACTION, "action");
        assert_eq!(prop_keys::CHECKED, "checked");
    }

    // ---- Component::from_value / from_json 构造器 ----

    #[test]
    fn component_from_value_parses_protocol_shape() {
        let comp = Component::from_value(json!({
            "component": "Button", "id": "b1", "child": "lbl", "variant": "primary"
        }))
        .expect("valid component");
        assert_eq!(comp.component_type(), "Button");
        assert_eq!(comp.id().as_str(), "b1");
        assert_eq!(comp.prop_str("variant"), Some("primary"));
        // 非法 ID → Err（走 ComponentId 校验）
        assert!(Component::from_value(json!({"component": "Text", "id": "9bad"})).is_err());
        // 缺必填字段 → Err
        assert!(Component::from_value(json!({"id": "x"})).is_err());
    }

    #[test]
    fn component_from_json_parses_protocol_shape() {
        let comp = Component::from_json(r#"{"component":"Text","id":"t1","text":"hi"}"#)
            .expect("valid component");
        assert_eq!(comp.component_type(), "Text");
        assert_eq!(comp.prop_str("text"), Some("hi"));
        assert!(Component::from_json("not json").is_err());
    }
}
