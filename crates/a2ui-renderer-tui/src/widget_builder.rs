use a2ui_core::prelude::*;
use a2ui_renderer::component_forest::ComponentTreeNode;
use a2ui_renderer::{
    resolve_dynamic_string_prop, ComponentForest, ComponentStyle, CustomComponentRegistry,
    DataBinding,
};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// 渲染目标 widget（类型抹平，用于渲染管线）
#[derive(Debug, Clone)]
pub enum RenderableWidget {
    Paragraph {
        id: ComponentId,
        area: Rect,
        text: String,
        style: ComponentStyle,
    },
    Block {
        id: ComponentId,
        area: Rect,
        title: String,
    },
    Placeholder {
        id: ComponentId,
        area: Rect,
        reason: String,
    },
    TextField {
        id: ComponentId,
        area: Rect,
        value: String,
        placeholder: String,
    },
    CheckBox {
        id: ComponentId,
        area: Rect,
        label: String,
        checked: bool,
    },
    Slider {
        id: ComponentId,
        area: Rect,
        value: f64,
        min: f64,
        max: f64,
    },
    Button {
        id: ComponentId,
        area: Rect,
        label: String,
        variant: String,
    },
    Card {
        id: ComponentId,
        area: Rect,
    },
    Divider {
        id: ComponentId,
        area: Rect,
    },
    Icon {
        id: ComponentId,
        area: Rect,
        symbol: String,
        style: ComponentStyle,
    },
    Image {
        id: ComponentId,
        area: Rect,
        url: String,
    },
    Tabs {
        id: ComponentId,
        area: Rect,
        titles: Vec<String>,
        children_ids: Vec<ComponentId>,
    },
    ChoicePicker {
        id: ComponentId,
        area: Rect,
        options: Vec<String>,
        selected: Vec<String>,
    },
    Video {
        id: ComponentId,
        area: Rect,
        url: String,
    },
    AudioPlayer {
        id: ComponentId,
        area: Rect,
        url: String,
        description: String,
    },
    Modal {
        id: ComponentId,
        area: Rect,
        trigger_id: String,
        content_id: String,
    },
    DateTimeInput {
        id: ComponentId,
        area: Rect,
        label: String,
    },
}

impl RenderableWidget {
    pub fn id(&self) -> &ComponentId {
        match self {
            Self::Paragraph { id, .. }
            | Self::Block { id, .. }
            | Self::Placeholder { id, .. }
            | Self::TextField { id, .. }
            | Self::CheckBox { id, .. }
            | Self::Slider { id, .. }
            | Self::Button { id, .. }
            | Self::Card { id, .. }
            | Self::Divider { id, .. }
            | Self::Icon { id, .. }
            | Self::Image { id, .. }
            | Self::Tabs { id, .. }
            | Self::ChoicePicker { id, .. }
            | Self::Video { id, .. }
            | Self::AudioPlayer { id, .. }
            | Self::Modal { id, .. }
            | Self::DateTimeInput { id, .. } => id,
        }
    }

    pub fn area(&self) -> Rect {
        match self {
            Self::Paragraph { area, .. }
            | Self::Block { area, .. }
            | Self::Placeholder { area, .. }
            | Self::TextField { area, .. }
            | Self::CheckBox { area, .. }
            | Self::Slider { area, .. }
            | Self::Button { area, .. }
            | Self::Card { area, .. }
            | Self::Divider { area, .. }
            | Self::Icon { area, .. }
            | Self::Image { area, .. }
            | Self::Tabs { area, .. }
            | Self::ChoicePicker { area, .. }
            | Self::Video { area, .. }
            | Self::AudioPlayer { area, .. }
            | Self::Modal { area, .. }
            | Self::DateTimeInput { area, .. } => *area,
        }
    }
}

/// 将组件森林构建为渲染目标列表
pub struct WidgetBuilder<'a> {
    binding: &'a DataBinding,
    forest: &'a ComponentForest,
    registry: &'a CustomComponentRegistry,
}

impl<'a> WidgetBuilder<'a> {
    pub fn new(
        binding: &'a DataBinding,
        forest: &'a ComponentForest,
        registry: &'a CustomComponentRegistry,
    ) -> Self {
        Self {
            binding,
            forest,
            registry,
        }
    }

    /// 从指定 Surface 的根组件开始构建 widget 树
    pub fn build_tree(&self, surface_id: &str, area: Rect) -> Vec<RenderableWidget> {
        let tree = match self.forest.build_tree(surface_id) {
            Ok(t) => t,
            Err(_) => return vec![],
        };

        let mut widgets = Vec::new();
        self.flatten_node(&tree, area, &mut widgets);
        widgets
    }

    /// 递归展平组件树为 RenderableWidget 列表（前序遍历：容器先于子元素）
    fn flatten_node(
        &self,
        node: &ComponentTreeNode,
        area: Rect,
        widgets: &mut Vec<RenderableWidget>,
    ) {
        let comp = &node.component;

        // 生成当前组件的 widget
        widgets.push(self.component_to_widget(comp, area));

        // 获取子组件 ID 列表（从组件属性解析）
        let child_ids = self.get_child_ids(comp);
        let children = &node.children;

        // 为缺失的组件创建占位符
        for missing_id in &child_ids {
            if !children
                .iter()
                .any(|c| c.component.id().as_str() == missing_id.as_str())
            {
                widgets.push(RenderableWidget::Placeholder {
                    id: missing_id.clone(),
                    area,
                    reason: format!("component not found: {}", missing_id),
                });
            }
        }

        if children.is_empty() {
            return;
        }

        // 计算子组件区域
        let child_areas = self.layout_children(comp, children, area);
        for (child_node, child_area) in children.iter().zip(child_areas) {
            self.flatten_node(child_node, child_area, widgets);
        }
    }

    /// 从组件属性中提取子组件 ID 列表
    fn get_child_ids(&self, component: &Component) -> Vec<ComponentId> {
        let props = component.properties();
        let mut ids = Vec::new();

        // 检查 children 属性 — 支持两种格式：
        // 1. 数组格式: {"children": ["id1", "id2"]}（Component::column() 生成）
        // 2. 对象格式: {"children": {"children": [...]}}（旧版兼容）
        if let Some(children_val) = props.get("children") {
            // 数组格式
            if let Some(children_arr) = children_val.as_array() {
                for id_val in children_arr {
                    if let Some(id_str) = id_val.as_str() {
                        if let Ok(id) = ComponentId::new(id_str) {
                            ids.push(id);
                        }
                    }
                }
            }
            // 对象格式（兼容）
            if let Some(children_obj) = children_val.as_object() {
                if let Some(ids_val) = children_obj.get("children") {
                    if let Some(ids_arr) = ids_val.as_array() {
                        for id_val in ids_arr {
                            if let Some(id_str) = id_val.as_str() {
                                if let Ok(id) = ComponentId::new(id_str) {
                                    ids.push(id);
                                }
                            }
                        }
                    }
                }
            }
        }

        // 检查单个 child 属性
        if let Some(child_str) = props.get("child").and_then(|v| v.as_str()) {
            if let Ok(id) = ComponentId::new(child_str) {
                ids.push(id);
            }
        }

        ids
    }

    /// 将单个组件转换为 RenderableWidget
    fn component_to_widget(&self, component: &Component, area: Rect) -> RenderableWidget {
        let ctype = component.component_type();

        match ctype {
            "Column" | "Row" | "List" => RenderableWidget::Block {
                id: component.id().clone(),
                area,
                title: format!("[{}]", ctype),
            },
            "Button" => {
                let props = component.properties();
                let child_id = props.get("child").and_then(|v| v.as_str()).unwrap_or("");
                // 尝试从引用的子 Text 组件获取 label
                let label = self.resolve_child_text(component, child_id);
                let variant = props
                    .get("variant")
                    .and_then(|v| v.as_str())
                    .unwrap_or("default")
                    .to_string();
                RenderableWidget::Button {
                    id: component.id().clone(),
                    area,
                    label,
                    variant,
                }
            }
            "Card" => RenderableWidget::Card {
                id: component.id().clone(),
                area,
            },
            "Divider" => RenderableWidget::Divider {
                id: component.id().clone(),
                area,
            },
            "Icon" => {
                let props = component.properties();
                let name = resolve_dynamic_string_prop(props, "name", Some(self.binding), "?");
                let symbol = icon_to_symbol(name);
                RenderableWidget::Icon {
                    id: component.id().clone(),
                    area,
                    symbol,
                    style: ComponentStyle::from_component_props(props),
                }
            }
            "Image" => {
                let props = component.properties();
                let url = resolve_dynamic_string_prop(props, "url", Some(self.binding), "");
                RenderableWidget::Image {
                    id: component.id().clone(),
                    area,
                    url,
                }
            }
            "Tabs" => {
                let props = component.properties();
                let (titles, children_ids) = parse_tabs(props);
                RenderableWidget::Tabs {
                    id: component.id().clone(),
                    area,
                    titles,
                    children_ids,
                }
            }
            "ChoicePicker" => {
                let props = component.properties();
                let options: Vec<String> = props
                    .get("options")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|o| o.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                let selected: Vec<String> = props
                    .get("value")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|s| s.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                RenderableWidget::ChoicePicker {
                    id: component.id().clone(),
                    area,
                    options,
                    selected,
                }
            }
            "Video" => {
                let url = resolve_dynamic_string_prop(
                    component.properties(),
                    "url",
                    Some(self.binding),
                    "",
                );
                RenderableWidget::Video {
                    id: component.id().clone(),
                    area,
                    url,
                }
            }
            "AudioPlayer" => {
                let props = component.properties();
                let url = resolve_dynamic_string_prop(props, "url", Some(self.binding), "");
                let desc =
                    resolve_dynamic_string_prop(props, "description", Some(self.binding), "");
                RenderableWidget::AudioPlayer {
                    id: component.id().clone(),
                    area,
                    url,
                    description: desc,
                }
            }
            "Modal" => {
                let props = component.properties();
                let trigger = props
                    .get("trigger")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let content = props
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                RenderableWidget::Modal {
                    id: component.id().clone(),
                    area,
                    trigger_id: trigger,
                    content_id: content,
                }
            }
            "DateTimeInput" => {
                let label = resolve_dynamic_string_prop(
                    component.properties(),
                    "label",
                    Some(self.binding),
                    "",
                );
                RenderableWidget::DateTimeInput {
                    id: component.id().clone(),
                    area,
                    label,
                }
            }
            "TextField" => {
                let props = component.properties();
                let value = resolve_dynamic_string_prop(props, "value", Some(self.binding), "");
                let placeholder =
                    resolve_dynamic_string_prop(props, "placeholder", Some(self.binding), "");
                RenderableWidget::TextField {
                    id: component.id().clone(),
                    area,
                    value,
                    placeholder,
                }
            }
            "CheckBox" => {
                let props = component.properties();
                let label = resolve_dynamic_string_prop(props, "label", Some(self.binding), "");
                let checked = props
                    .get("checked")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                RenderableWidget::CheckBox {
                    id: component.id().clone(),
                    area,
                    label,
                    checked,
                }
            }
            "Slider" => {
                let props = component.properties();
                let value = props.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let min = props.get("min").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let max = props.get("max").and_then(|v| v.as_f64()).unwrap_or(100.0);
                RenderableWidget::Slider {
                    id: component.id().clone(),
                    area,
                    value,
                    min,
                    max,
                }
            }
            _ => {
                // 先检查自定义组件注册表
                if self.registry.is_registered(ctype) {
                    RenderableWidget::Placeholder {
                        id: component.id().clone(),
                        area,
                        reason: format!("custom component: {}", ctype),
                    }
                } else {
                    // 尝试提取文本
                    let text = self.extract_text(component);
                    if text.starts_with('[') && text.ends_with(']') {
                        // 未知组件类型，渲染为占位符
                        RenderableWidget::Placeholder {
                            id: component.id().clone(),
                            area,
                            reason: format!("unknown component type: {}", ctype),
                        }
                    } else {
                        RenderableWidget::Paragraph {
                            id: component.id().clone(),
                            area,
                            text,
                            style: ComponentStyle::from_component_props(component.properties()),
                        }
                    }
                }
            }
        }
    }

    /// 从组件属性中提取文本，并在可用时解析 DataBinding 路径。
    fn extract_text(&self, component: &Component) -> String {
        resolve_dynamic_string_prop(
            component.properties(),
            "text",
            Some(self.binding),
            &format!("[{}]", component.component_type()),
        )
    }

    /// 从 Button 引用的子 Text 组件中解析标签文本
    fn resolve_child_text(&self, component: &Component, child_id: &str) -> String {
        if child_id.is_empty() {
            return self.extract_text(component);
        }
        if let Ok(child_component_id) = ComponentId::new(child_id) {
            if let Some(surface_id) = self.forest.surface_of(component.id()) {
                if let Some(child) = self.forest.get(surface_id, &child_component_id) {
                    return self.extract_text(child);
                }
            }
        }
        self.extract_text(component)
    }

    /// 根据组件类型为子组件分配渲染区域
    fn layout_children(
        &self,
        parent: &Component,
        children: &[ComponentTreeNode],
        area: Rect,
    ) -> Vec<Rect> {
        let ctype = parent.component_type();
        let count = children.len();

        if count == 0 {
            return vec![];
        }

        match ctype {
            "Column" => {
                let height = area.height / count as u16;
                let mut areas = Vec::new();
                for i in 0..count {
                    areas.push(Rect::new(
                        area.x,
                        area.y + i as u16 * height,
                        area.width,
                        height,
                    ));
                }
                areas
            }
            "Row" => {
                let width = area.width / count as u16;
                let mut areas = Vec::new();
                for i in 0..count {
                    areas.push(Rect::new(
                        area.x + i as u16 * width,
                        area.y,
                        width,
                        area.height,
                    ));
                }
                areas
            }
            _ => {
                // 默认：均分区域
                let area_per_child = Rect::new(area.x, area.y, area.width, area.height);
                vec![area_per_child; count]
            }
        }
    }
}

pub(crate) fn component_style_to_tui(style: &ComponentStyle) -> Style {
    let mut tui_style = Style::default();
    if let Some(color) = style.color {
        tui_style = tui_style.fg(Color::Rgb(color.r, color.g, color.b));
    }
    if style.strong {
        tui_style = tui_style.add_modifier(Modifier::BOLD);
    }
    tui_style
}

/// 图标名 → Unicode 符号映射
fn icon_to_symbol(name: impl AsRef<str>) -> String {
    let name = name.as_ref();
    match name {
        "star" => "★".to_string(),
        "home" => "⌂".to_string(),
        "search" => "⌕".to_string(),
        "settings" => "⚙".to_string(),
        "person" => "👤".to_string(),
        "mail" => "✉".to_string(),
        "phone" => "📞".to_string(),
        "camera" => "📷".to_string(),
        "music" => "♪".to_string(),
        "video" => "▶".to_string(),
        "menu" => "☰".to_string(),
        "close" => "✕".to_string(),
        "check" => "✓".to_string(),
        "arrow_up" => "↑".to_string(),
        "arrow_down" => "↓".to_string(),
        "arrow_left" => "←".to_string(),
        "arrow_right" => "→".to_string(),
        "refresh" => "↻".to_string(),
        "delete" => "🗑".to_string(),
        "edit" => "✎".to_string(),
        "add" => "+".to_string(),
        "remove" => "−".to_string(),
        "info" => "ℹ".to_string(),
        "warning" => "⚠".to_string(),
        "error" => "✗".to_string(),
        "help" => "?".to_string(),
        "lock" => "🔒".to_string(),
        "unlock" => "🔓".to_string(),
        "heart" => "♥".to_string(),
        "bookmark" => "🔖".to_string(),
        "share" => "↗".to_string(),
        "download" => "↓".to_string(),
        "upload" => "↑".to_string(),
        _ => name.to_string(),
    }
}

/// 从 Tabs 组件的 properties 中解析标题和子组件列表
fn parse_tabs(props: &serde_json::Value) -> (Vec<String>, Vec<ComponentId>) {
    let mut titles = Vec::new();
    let mut children = Vec::new();
    if let Some(arr) = props.get("tabs").and_then(|v| v.as_array()) {
        for tab in arr {
            if let Some(title) = tab.get("title").and_then(|v| v.as_str()) {
                titles.push(title.to_string());
            }
            if let Some(child) = tab.get("child").and_then(|v| v.as_str()) {
                if let Ok(cid) = ComponentId::new(child) {
                    children.push(cid);
                }
            }
        }
    }
    (titles, children)
}

#[cfg(test)]
mod tests {
    use super::*;
    use a2ui_renderer::CustomComponentRegistry;
    use ratatui::style::{Color, Modifier};
    use serde_json::json;

    #[test]
    fn test_build_widget_tree_from_components() {
        let mut forest = ComponentForest::new();
        let binding = DataBinding::new(DataModel::new(json!({"title": "Hello"})));
        let reg = CustomComponentRegistry::new();

        // Column 作为根组件（ID 必须为 "root" 才能被 forest 识别）
        let root = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("title").unwrap()],
        );
        let title = Component::text(
            ComponentId::new("title").unwrap(),
            DynamicValue::Path {
                path: "/title".into(),
            },
        );

        forest.upsert("s1", root).unwrap();
        forest.upsert("s1", title).unwrap();

        let builder = WidgetBuilder::new(&binding, &forest, &reg);
        let widgets = builder.build_tree("s1", Rect::new(0, 0, 80, 24));

        // 至少有一个 widget（root Column + title Text）
        assert!(widgets.len() >= 2);
        // root 应为 Block（Column 类型）
        assert_eq!(widgets[0].id().as_str(), "root");
        // title 应能找到
        let title_widget = widgets.iter().find(|w| w.id().as_str() == "title");
        assert!(matches!(
            title_widget,
            Some(RenderableWidget::Paragraph { text, .. }) if text == "Hello"
        ));
    }

    #[test]
    fn test_button_label_resolves_child_text_from_data_binding() {
        let mut forest = ComponentForest::new();
        let binding = DataBinding::new(DataModel::new(json!({"button": {"label": "Submit"}})));
        let reg = CustomComponentRegistry::new();

        let root = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("btn").unwrap()],
        );
        let btn = Component::button(
            ComponentId::new("btn").unwrap(),
            ComponentId::new("label").unwrap(),
        );
        let label = Component::text(
            ComponentId::new("label").unwrap(),
            DynamicValue::Path {
                path: "/button/label".into(),
            },
        );

        forest.upsert("s1", root).unwrap();
        forest.upsert("s1", btn).unwrap();
        forest.upsert("s1", label).unwrap();

        let builder = WidgetBuilder::new(&binding, &forest, &reg);
        let widgets = builder.build_tree("s1", Rect::new(0, 0, 80, 24));
        let button_widget = widgets.iter().find(|w| w.id().as_str() == "btn");

        assert!(matches!(
            button_widget,
            Some(RenderableWidget::Button { label, .. }) if label == "Submit"
        ));
    }

    #[test]
    fn test_string_props_resolve_from_data_binding() {
        let mut forest = ComponentForest::new();
        let binding = DataBinding::new(DataModel::new(json!({
            "icon": "star",
            "image": "https://example.com/a.png",
            "video": "https://example.com/a.mp4",
            "audio": {
                "url": "https://example.com/a.mp3",
                "description": "Track"
            },
            "form": {
                "value": "Alice",
                "placeholder": "Name",
                "label": "Accept"
            },
            "date": "Pick date"
        })));
        let reg = CustomComponentRegistry::new();

        let root: Component = serde_json::from_value(json!({
            "id": "root",
            "component": "Column",
            "children": ["icon", "image", "video", "audio", "field", "check", "date"]
        }))
        .unwrap();
        let icon: Component = serde_json::from_value(json!({
            "id": "icon",
            "component": "Icon",
            "name": {"path": "/icon"}
        }))
        .unwrap();
        let image: Component = serde_json::from_value(json!({
            "id": "image",
            "component": "Image",
            "url": {"path": "/image"}
        }))
        .unwrap();
        let video: Component = serde_json::from_value(json!({
            "id": "video",
            "component": "Video",
            "url": {"path": "/video"}
        }))
        .unwrap();
        let audio: Component = serde_json::from_value(json!({
            "id": "audio",
            "component": "AudioPlayer",
            "url": {"path": "/audio/url"},
            "description": {"path": "/audio/description"}
        }))
        .unwrap();
        let field: Component = serde_json::from_value(json!({
            "id": "field",
            "component": "TextField",
            "value": {"path": "/form/value"},
            "placeholder": {"path": "/form/placeholder"}
        }))
        .unwrap();
        let check: Component = serde_json::from_value(json!({
            "id": "check",
            "component": "CheckBox",
            "checked": true,
            "label": {"path": "/form/label"}
        }))
        .unwrap();
        let date: Component = serde_json::from_value(json!({
            "id": "date",
            "component": "DateTimeInput",
            "label": {"path": "/date"}
        }))
        .unwrap();

        for component in [root, icon, image, video, audio, field, check, date] {
            forest.upsert("s1", component).unwrap();
        }

        let builder = WidgetBuilder::new(&binding, &forest, &reg);
        let widgets = builder.build_tree("s1", Rect::new(0, 0, 80, 24));

        assert!(matches!(
            widgets.iter().find(|w| w.id().as_str() == "icon"),
            Some(RenderableWidget::Icon { symbol, .. }) if symbol == "★"
        ));
        assert!(matches!(
            widgets.iter().find(|w| w.id().as_str() == "image"),
            Some(RenderableWidget::Image { url, .. }) if url == "https://example.com/a.png"
        ));
        assert!(matches!(
            widgets.iter().find(|w| w.id().as_str() == "video"),
            Some(RenderableWidget::Video { url, .. }) if url == "https://example.com/a.mp4"
        ));
        assert!(matches!(
            widgets.iter().find(|w| w.id().as_str() == "audio"),
            Some(RenderableWidget::AudioPlayer { url, description, .. })
                if url == "https://example.com/a.mp3" && description == "Track"
        ));
        assert!(matches!(
            widgets.iter().find(|w| w.id().as_str() == "field"),
            Some(RenderableWidget::TextField { value, placeholder, .. })
                if value == "Alice" && placeholder == "Name"
        ));
        assert!(matches!(
            widgets.iter().find(|w| w.id().as_str() == "check"),
            Some(RenderableWidget::CheckBox { label, checked, .. })
                if label == "Accept" && *checked
        ));
        assert!(matches!(
            widgets.iter().find(|w| w.id().as_str() == "date"),
            Some(RenderableWidget::DateTimeInput { label, .. }) if label == "Pick date"
        ));
    }

    #[test]
    fn test_missing_component_renders_placeholder() {
        let mut forest = ComponentForest::new();
        let binding = DataBinding::new(DataModel::empty());

        let root = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("missing").unwrap()],
        );
        forest.upsert("s1", root).unwrap();

        let reg = CustomComponentRegistry::new();
        let builder = WidgetBuilder::new(&binding, &forest, &reg);
        let widgets = builder.build_tree("s1", Rect::new(0, 0, 80, 24));

        // missing 组件应渲染为占位符
        let placeholder = widgets.iter().find(|w| w.id().as_str() == "missing");
        assert!(placeholder.is_some());
        if let Some(RenderableWidget::Placeholder { reason, .. }) = placeholder {
            assert!(reason.contains("not found"));
        }
    }

    #[test]
    fn test_widget_area_assignment() {
        let mut forest = ComponentForest::new();
        let binding = DataBinding::new(DataModel::empty());

        let root = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("child").unwrap()],
        );
        forest.upsert("s1", root).unwrap();

        let reg = CustomComponentRegistry::new();
        let builder = WidgetBuilder::new(&binding, &forest, &reg);
        let widgets = builder.build_tree("s1", Rect::new(10, 20, 80, 24));

        // root widget 应获得完整区域
        let root_widget = widgets.iter().find(|w| w.id().as_str() == "root");
        assert!(root_widget.is_some());
        assert_eq!(root_widget.unwrap().area(), Rect::new(10, 20, 80, 24));
    }

    // --- P3-3: TextField/CheckBox/Slider widget mapping tests ---

    #[test]
    fn test_textfield_component_maps_to_textfield_widget() {
        let mut forest = ComponentForest::new();
        let binding = DataBinding::new(DataModel::empty());

        let root = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("name_input").unwrap()],
        );
        let tf: Component = serde_json::from_str(
            r#"{"id":"name_input","component":"TextField","value":"Alice","placeholder":"Enter name"}"#
        ).unwrap();
        forest.upsert("s1", root).unwrap();
        forest.upsert("s1", tf).unwrap();

        let reg = CustomComponentRegistry::new();
        let builder = WidgetBuilder::new(&binding, &forest, &reg);
        let widgets = builder.build_tree("s1", Rect::new(0, 0, 80, 24));

        let tf_widget = widgets.iter().find(|w| w.id().as_str() == "name_input");
        assert!(tf_widget.is_some(), "TextField widget should exist in tree");
        assert!(
            matches!(tf_widget.unwrap(), RenderableWidget::TextField { .. }),
            "TextField component should produce RenderableWidget::TextField"
        );
    }

    #[test]
    fn test_checkbox_component_maps_to_checkbox_widget() {
        let mut forest = ComponentForest::new();
        let binding = DataBinding::new(DataModel::empty());

        let root = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("agree").unwrap()],
        );
        let cb: Component = serde_json::from_str(
            r#"{"id":"agree","component":"CheckBox","checked":true,"label":"I agree"}"#,
        )
        .unwrap();
        forest.upsert("s1", root).unwrap();
        forest.upsert("s1", cb).unwrap();

        let reg = CustomComponentRegistry::new();
        let builder = WidgetBuilder::new(&binding, &forest, &reg);
        let widgets = builder.build_tree("s1", Rect::new(0, 0, 80, 24));

        let cb_widget = widgets.iter().find(|w| w.id().as_str() == "agree");
        assert!(cb_widget.is_some(), "CheckBox widget should exist in tree");
        assert!(
            matches!(cb_widget.unwrap(), RenderableWidget::CheckBox { .. }),
            "CheckBox component should produce RenderableWidget::CheckBox"
        );
    }

    #[test]
    fn test_slider_component_maps_to_slider_widget() {
        let mut forest = ComponentForest::new();
        let binding = DataBinding::new(DataModel::empty());

        let root = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("volume").unwrap()],
        );
        let sl: Component = serde_json::from_str(
            r#"{"id":"volume","component":"Slider","value":50,"min":0,"max":100}"#,
        )
        .unwrap();
        forest.upsert("s1", root).unwrap();
        forest.upsert("s1", sl).unwrap();

        let reg = CustomComponentRegistry::new();
        let builder = WidgetBuilder::new(&binding, &forest, &reg);
        let widgets = builder.build_tree("s1", Rect::new(0, 0, 80, 24));

        let sl_widget = widgets.iter().find(|w| w.id().as_str() == "volume");
        assert!(sl_widget.is_some(), "Slider widget should exist in tree");
        assert!(
            matches!(sl_widget.unwrap(), RenderableWidget::Slider { .. }),
            "Slider component should produce RenderableWidget::Slider"
        );
    }

    // ---- 新增组件映射测试 ----

    #[test]
    fn test_button_maps_to_button_widget() {
        let mut forest = ComponentForest::new();
        let binding = DataBinding::new(DataModel::empty());
        let root = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("btn").unwrap()],
        );
        let btn: Component = serde_json::from_str(
            r#"{"id":"btn","component":"Button","child":"lbl","variant":"primary"}"#,
        )
        .unwrap();
        forest.upsert("s1", root).unwrap();
        forest.upsert("s1", btn).unwrap();
        let reg = CustomComponentRegistry::new();
        let builder = WidgetBuilder::new(&binding, &forest, &reg);
        let widgets = builder.build_tree("s1", Rect::new(0, 0, 80, 24));
        let w = widgets.iter().find(|w| w.id().as_str() == "btn");
        assert!(w.is_some(), "Button widget should exist");
        assert!(matches!(w.unwrap(), RenderableWidget::Button { .. }));
    }

    #[test]
    fn test_divider_maps_to_divider_widget() {
        let mut forest = ComponentForest::new();
        let binding = DataBinding::new(DataModel::empty());
        let root = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("div").unwrap()],
        );
        let div: Component = serde_json::from_str(r#"{"id":"div","component":"Divider"}"#).unwrap();
        forest.upsert("s1", root).unwrap();
        forest.upsert("s1", div).unwrap();
        let reg = CustomComponentRegistry::new();
        let builder = WidgetBuilder::new(&binding, &forest, &reg);
        let widgets = builder.build_tree("s1", Rect::new(0, 0, 80, 24));
        let w = widgets.iter().find(|w| w.id().as_str() == "div");
        assert!(w.is_some(), "Divider widget should exist");
        assert!(matches!(w.unwrap(), RenderableWidget::Divider { .. }));
    }

    #[test]
    fn test_icon_maps_to_icon_widget() {
        let mut forest = ComponentForest::new();
        let binding = DataBinding::new(DataModel::empty());
        let root = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("ic").unwrap()],
        );
        let icon: Component =
            serde_json::from_str(r#"{"id":"ic","component":"Icon","name":"star"}"#).unwrap();
        forest.upsert("s1", root).unwrap();
        forest.upsert("s1", icon).unwrap();
        let reg = CustomComponentRegistry::new();
        let builder = WidgetBuilder::new(&binding, &forest, &reg);
        let widgets = builder.build_tree("s1", Rect::new(0, 0, 80, 24));
        let w = widgets.iter().find(|w| w.id().as_str() == "ic");
        assert!(w.is_some(), "Icon widget should exist");
        assert!(matches!(w.unwrap(), RenderableWidget::Icon { .. }));
    }

    #[test]
    fn test_styled_text_maps_to_paragraph_with_degraded_tui_style() {
        let mut forest = ComponentForest::new();
        let binding = DataBinding::new(DataModel::empty());
        let root: Component = serde_json::from_value(json!({
            "id": "root",
            "component": "Text",
            "text": "Styled",
            "style": {
                "fontSize": 18,
                "strong": true,
                "color": "#112233",
                "fill": "#445566",
                "padding": 9,
                "radius": 5
            }
        }))
        .unwrap();
        forest.upsert("s1", root).unwrap();

        let reg = CustomComponentRegistry::new();
        let builder = WidgetBuilder::new(&binding, &forest, &reg);
        let widgets = builder.build_tree("s1", Rect::new(0, 0, 80, 24));
        let w = widgets.iter().find(|w| w.id().as_str() == "root");

        match w {
            Some(RenderableWidget::Paragraph { style, .. }) => {
                assert!(style.strong);
                assert_eq!(
                    style.color.map(|c| (c.r, c.g, c.b, c.a)),
                    Some((17, 34, 51, 255))
                );
                assert_eq!(
                    style.fill.map(|c| (c.r, c.g, c.b, c.a)),
                    Some((68, 85, 102, 255))
                );
                assert_eq!(style.padding, Some(9.0));
                assert_eq!(style.radius, Some(5.0));

                let tui_style = component_style_to_tui(style);
                assert_eq!(tui_style.fg, Some(Color::Rgb(17, 34, 51)));
                assert_eq!(tui_style.bg, None);
                assert!(tui_style.add_modifier.contains(Modifier::BOLD));
            }
            other => panic!("styled Text should map to Paragraph, got {other:?}"),
        }
    }

    #[test]
    fn test_styled_icon_maps_to_icon_with_degraded_tui_style() {
        let mut forest = ComponentForest::new();
        let binding = DataBinding::new(DataModel::empty());
        let root: Component = serde_json::from_value(json!({
            "id": "root",
            "component": "Icon",
            "name": "star",
            "style": {
                "fontSize": 18,
                "strong": true,
                "color": "#112233",
                "fill": "#445566",
                "padding": 9,
                "radius": 5
            }
        }))
        .unwrap();
        forest.upsert("s1", root).unwrap();

        let reg = CustomComponentRegistry::new();
        let builder = WidgetBuilder::new(&binding, &forest, &reg);
        let widgets = builder.build_tree("s1", Rect::new(0, 0, 80, 24));
        let w = widgets.iter().find(|w| w.id().as_str() == "root");

        match w {
            Some(RenderableWidget::Icon { style, .. }) => {
                assert!(style.strong);
                assert_eq!(
                    style.color.map(|c| (c.r, c.g, c.b, c.a)),
                    Some((17, 34, 51, 255))
                );

                let tui_style = component_style_to_tui(style);
                assert_eq!(tui_style.fg, Some(Color::Rgb(17, 34, 51)));
                assert_eq!(tui_style.bg, None);
                assert!(tui_style.add_modifier.contains(Modifier::BOLD));
            }
            other => panic!("styled Icon should map to Icon, got {other:?}"),
        }
    }

    #[test]
    fn test_image_maps_to_image_widget() {
        let mut forest = ComponentForest::new();
        let binding = DataBinding::new(DataModel::empty());
        let root = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("img").unwrap()],
        );
        let img: Component = serde_json::from_str(
            r#"{"id":"img","component":"Image","url":"https://example.com/pic.png"}"#,
        )
        .unwrap();
        forest.upsert("s1", root).unwrap();
        forest.upsert("s1", img).unwrap();
        let reg = CustomComponentRegistry::new();
        let builder = WidgetBuilder::new(&binding, &forest, &reg);
        let widgets = builder.build_tree("s1", Rect::new(0, 0, 80, 24));
        let w = widgets.iter().find(|w| w.id().as_str() == "img");
        assert!(w.is_some(), "Image widget should exist");
        assert!(matches!(w.unwrap(), RenderableWidget::Image { .. }));
    }

    #[test]
    fn test_card_maps_to_card_widget() {
        let mut forest = ComponentForest::new();
        let binding = DataBinding::new(DataModel::empty());
        let root = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("card").unwrap()],
        );
        let card: Component =
            serde_json::from_str(r#"{"id":"card","component":"Card","child":"inner"}"#).unwrap();
        forest.upsert("s1", root).unwrap();
        forest.upsert("s1", card).unwrap();
        let reg = CustomComponentRegistry::new();
        let builder = WidgetBuilder::new(&binding, &forest, &reg);
        let widgets = builder.build_tree("s1", Rect::new(0, 0, 80, 24));
        let w = widgets.iter().find(|w| w.id().as_str() == "card");
        assert!(w.is_some(), "Card widget should exist");
        assert!(matches!(w.unwrap(), RenderableWidget::Card { .. }));
    }

    #[test]
    fn test_tabs_maps_to_tabs_widget() {
        let mut forest = ComponentForest::new();
        let binding = DataBinding::new(DataModel::empty());
        let root = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("tabs").unwrap()],
        );
        let tabs: Component = serde_json::from_str(
            r#"{"id":"tabs","component":"Tabs","tabs":[{"title":"A","child":"a"},{"title":"B","child":"b"}]}"#,
        ).unwrap();
        forest.upsert("s1", root).unwrap();
        forest.upsert("s1", tabs).unwrap();
        let reg = CustomComponentRegistry::new();
        let builder = WidgetBuilder::new(&binding, &forest, &reg);
        let widgets = builder.build_tree("s1", Rect::new(0, 0, 80, 24));
        let w = widgets.iter().find(|w| w.id().as_str() == "tabs");
        assert!(w.is_some(), "Tabs widget should exist");
        assert!(matches!(w.unwrap(), RenderableWidget::Tabs { .. }));
    }

    #[test]
    fn test_choice_picker_maps_to_choice_picker_widget() {
        let mut forest = ComponentForest::new();
        let binding = DataBinding::new(DataModel::empty());
        let root = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("cp").unwrap()],
        );
        let cp: Component = serde_json::from_str(
            r#"{"id":"cp","component":"ChoicePicker","options":["A","B","C"],"value":["A"]}"#,
        )
        .unwrap();
        forest.upsert("s1", root).unwrap();
        forest.upsert("s1", cp).unwrap();
        let reg = CustomComponentRegistry::new();
        let builder = WidgetBuilder::new(&binding, &forest, &reg);
        let widgets = builder.build_tree("s1", Rect::new(0, 0, 80, 24));
        let w = widgets.iter().find(|w| w.id().as_str() == "cp");
        assert!(w.is_some(), "ChoicePicker widget should exist");
        assert!(matches!(w.unwrap(), RenderableWidget::ChoicePicker { .. }));
    }

    #[test]
    fn test_video_maps_to_video_widget() {
        let mut forest = ComponentForest::new();
        let binding = DataBinding::new(DataModel::empty());
        let root = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("vid").unwrap()],
        );
        let vid: Component =
            serde_json::from_str(r#"{"id":"vid","component":"Video","url":"http://x.mp4"}"#)
                .unwrap();
        forest.upsert("s1", root).unwrap();
        forest.upsert("s1", vid).unwrap();
        let reg = CustomComponentRegistry::new();
        let builder = WidgetBuilder::new(&binding, &forest, &reg);
        let widgets = builder.build_tree("s1", Rect::new(0, 0, 80, 24));
        assert!(widgets
            .iter()
            .any(|w| matches!(w, RenderableWidget::Video { .. })));
    }

    #[test]
    fn test_audio_player_maps_to_audio_player_widget() {
        let mut forest = ComponentForest::new();
        let binding = DataBinding::new(DataModel::empty());
        let root = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("aud").unwrap()],
        );
        let aud: Component = serde_json::from_str(
            r#"{"id":"aud","component":"AudioPlayer","url":"http://x.mp3","description":"Song"}"#,
        )
        .unwrap();
        forest.upsert("s1", root).unwrap();
        forest.upsert("s1", aud).unwrap();
        let reg = CustomComponentRegistry::new();
        let builder = WidgetBuilder::new(&binding, &forest, &reg);
        let widgets = builder.build_tree("s1", Rect::new(0, 0, 80, 24));
        assert!(widgets
            .iter()
            .any(|w| matches!(w, RenderableWidget::AudioPlayer { .. })));
    }

    #[test]
    fn test_modal_maps_to_modal_widget() {
        let mut forest = ComponentForest::new();
        let binding = DataBinding::new(DataModel::empty());
        let root = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("modal").unwrap()],
        );
        let modal: Component = serde_json::from_str(
            r#"{"id":"modal","component":"Modal","content":"body","trigger":"btn"}"#,
        )
        .unwrap();
        forest.upsert("s1", root).unwrap();
        forest.upsert("s1", modal).unwrap();
        let reg = CustomComponentRegistry::new();
        let builder = WidgetBuilder::new(&binding, &forest, &reg);
        let widgets = builder.build_tree("s1", Rect::new(0, 0, 80, 24));
        assert!(widgets
            .iter()
            .any(|w| matches!(w, RenderableWidget::Modal { .. })));
    }

    #[test]
    fn test_date_time_input_maps_to_date_time_input_widget() {
        let mut forest = ComponentForest::new();
        let binding = DataBinding::new(DataModel::empty());
        let root = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("dt").unwrap()],
        );
        let dt: Component =
            serde_json::from_str(r#"{"id":"dt","component":"DateTimeInput","label":"Pick date"}"#)
                .unwrap();
        forest.upsert("s1", root).unwrap();
        forest.upsert("s1", dt).unwrap();
        let reg = CustomComponentRegistry::new();
        let builder = WidgetBuilder::new(&binding, &forest, &reg);
        let widgets = builder.build_tree("s1", Rect::new(0, 0, 80, 24));
        assert!(widgets
            .iter()
            .any(|w| matches!(w, RenderableWidget::DateTimeInput { .. })));
    }
}
