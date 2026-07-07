use a2ui_core::component::{prop_keys, ChildrenDecl};
use a2ui_core::prelude::*;
use a2ui_renderer::component_forest::ComponentTreeNode;
use a2ui_renderer::{
    resolve_str, ComponentForest, ComponentStyle, CustomComponentRegistry, DataBinding,
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

        match comp.component_type() {
            // TUI 未实现 Modal 弹层交互，占位文本已含 trigger/content 信息，
            // 不平铺其子组件（否则会与占位文本重叠渲染）
            "Modal" => return,
            // Tabs 仅渲染激活（第 0 个）tab 的子组件，按 id 从 children 中选取
            "Tabs" => {
                let (_, tab_children) = parse_tabs(comp);
                if let Some(active_id) = tab_children.first() {
                    if let Some(child_node) =
                        children.iter().find(|c| c.component.id() == active_id)
                    {
                        // 去掉标题行后的剩余区域
                        let content_area = Rect::new(
                            area.x,
                            area.y.saturating_add(1),
                            area.width,
                            area.height.saturating_sub(1),
                        );
                        self.flatten_node(child_node, content_area, widgets);
                    }
                }
                return;
            }
            _ => {}
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
        let mut ids = Vec::new();

        // 检查 children 属性 — 支持两种格式：
        // 1. 数组格式: {"children": ["id1", "id2"]}（Component::column() 生成，
        //    经 core 的 children_decl() 视图解析）
        // 2. 对象格式: {"children": {"children": [...]}}（旧版兼容——历史
        //    包袱不进 core，此处私有兜底；模板形态在核心层已展开为数组）
        match component.children_decl() {
            Some(ChildrenDecl::Ids(list)) => ids.extend(list),
            _ => {
                if let Some(ids_arr) = component
                    .properties()
                    .get(prop_keys::CHILDREN)
                    .and_then(|v| v.as_object())
                    .and_then(|obj| obj.get("children"))
                    .and_then(|v| v.as_array())
                {
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

        // 检查单个 child 属性
        if let Some(id) = component.prop_component_id(prop_keys::CHILD) {
            ids.push(id);
        }

        // Modal 的 content/trigger 引用（缺失时同样产生占位符）
        for key in [prop_keys::CONTENT, prop_keys::TRIGGER] {
            if let Some(id) = component.prop_component_id(key) {
                ids.push(id);
            }
        }

        // Tabs 的 tabs[].child 引用
        let (_, tab_children) = parse_tabs(component);
        ids.extend(tab_children);

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
                let child_id = component.prop_str(prop_keys::CHILD).unwrap_or("");
                // 尝试从引用的子 Text 组件获取 label
                let label = self.resolve_child_text(component, child_id);
                let variant = component
                    .prop_str(prop_keys::VARIANT)
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
                let name = resolve_string_prop(component, prop_keys::NAME, self.binding, "?");
                let symbol = icon_to_symbol(name);
                RenderableWidget::Icon {
                    id: component.id().clone(),
                    area,
                    symbol,
                    style: ComponentStyle::from_component(component),
                }
            }
            "Image" => {
                let url = resolve_string_prop(component, prop_keys::URL, self.binding, "");
                RenderableWidget::Image {
                    id: component.id().clone(),
                    area,
                    url,
                }
            }
            "Tabs" => {
                let (titles, children_ids) = parse_tabs(component);
                RenderableWidget::Tabs {
                    id: component.id().clone(),
                    area,
                    titles,
                    children_ids,
                }
            }
            "ChoicePicker" => {
                let options: Vec<String> = component
                    .prop_str_list(prop_keys::OPTIONS)
                    .map(|list| list.into_iter().map(String::from).collect())
                    .unwrap_or_default();
                let selected: Vec<String> = component
                    .prop_str_list(prop_keys::VALUE)
                    .map(|list| list.into_iter().map(String::from).collect())
                    .unwrap_or_default();
                RenderableWidget::ChoicePicker {
                    id: component.id().clone(),
                    area,
                    options,
                    selected,
                }
            }
            "Video" => {
                let url = resolve_string_prop(component, prop_keys::URL, self.binding, "");
                RenderableWidget::Video {
                    id: component.id().clone(),
                    area,
                    url,
                }
            }
            "AudioPlayer" => {
                let url = resolve_string_prop(component, prop_keys::URL, self.binding, "");
                let desc = resolve_string_prop(component, prop_keys::DESCRIPTION, self.binding, "");
                RenderableWidget::AudioPlayer {
                    id: component.id().clone(),
                    area,
                    url,
                    description: desc,
                }
            }
            "Modal" => {
                let trigger = component
                    .prop_str(prop_keys::TRIGGER)
                    .unwrap_or("")
                    .to_string();
                let content = component
                    .prop_str(prop_keys::CONTENT)
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
                let label = resolve_string_prop(component, prop_keys::LABEL, self.binding, "");
                RenderableWidget::DateTimeInput {
                    id: component.id().clone(),
                    area,
                    label,
                }
            }
            "TextField" => {
                let value = resolve_string_prop(component, prop_keys::VALUE, self.binding, "");
                let placeholder =
                    resolve_string_prop(component, prop_keys::PLACEHOLDER, self.binding, "");
                RenderableWidget::TextField {
                    id: component.id().clone(),
                    area,
                    value,
                    placeholder,
                }
            }
            "CheckBox" => {
                let label = resolve_string_prop(component, prop_keys::LABEL, self.binding, "");
                let checked = component.prop_bool(prop_keys::CHECKED).unwrap_or(false);
                RenderableWidget::CheckBox {
                    id: component.id().clone(),
                    area,
                    label,
                    checked,
                }
            }
            "Slider" => {
                let value = component.prop_f64(prop_keys::VALUE).unwrap_or(0.0);
                let min = component.prop_f64(prop_keys::MIN).unwrap_or(0.0);
                let max = component.prop_f64(prop_keys::MAX).unwrap_or(100.0);
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
                            style: ComponentStyle::from_component(component),
                        }
                    }
                }
            }
        }
    }

    /// 从组件属性中提取文本，并在可用时解析 DataBinding 路径。
    fn extract_text(&self, component: &Component) -> String {
        resolve_string_prop(
            component,
            prop_keys::TEXT,
            self.binding,
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

/// 从 Tabs 组件解析标题和子组件列表（委托 core 的 tabs_decl 视图）
fn parse_tabs(component: &Component) -> (Vec<String>, Vec<ComponentId>) {
    component
        .tabs_decl()
        .map(|tabs| tabs.into_iter().map(|tab| (tab.title, tab.child)).unzip())
        .unwrap_or_default()
}

/// 经类型化访问器解析动态字符串 prop；键缺失时给 fallback
/// （语义对齐旧 resolve_dynamic_string_prop：字面量原样、path 经绑定、
/// call 与未命中 path 给占位符、非字符串字面量按显示文本）
fn resolve_string_prop(
    component: &Component,
    key: &str,
    binding: &DataBinding,
    fallback: &str,
) -> String {
    match component.prop_dynamic_value(key) {
        Some(dv) => resolve_str(&dv, Some(binding)),
        None => fallback.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use a2ui_core::prelude::json;
    use a2ui_renderer::CustomComponentRegistry;
    use ratatui::style::{Color, Modifier};

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
    fn test_modal_children_not_flattened() {
        // TUI 未实现 Modal 弹层：content/trigger 组件不应被平铺到主区域
        let mut forest = ComponentForest::new();
        let binding = DataBinding::new(DataModel::new(json!({})));
        let reg = CustomComponentRegistry::new();

        let modal: Component = Component::from_value(
            json!({"component":"Modal","id":"root","content":"body","trigger":"btn"}),
        )
        .unwrap();
        let body = Component::text(
            ComponentId::new("body").unwrap(),
            DynamicValue::Literal("modal body".to_string()),
        );
        let btn: Component =
            Component::from_value(json!({"component":"Button","id":"btn","label":"open"})).unwrap();
        forest.upsert("s1", modal).unwrap();
        forest.upsert("s1", body).unwrap();
        forest.upsert("s1", btn).unwrap();

        let builder = WidgetBuilder::new(&binding, &forest, &reg);
        let widgets = builder.build_tree("s1", Rect::new(0, 0, 80, 24));

        assert!(
            !widgets.iter().any(|w| w.id().as_str() == "body"),
            "Modal content 不应被平铺渲染"
        );
        assert!(
            !widgets.iter().any(|w| w.id().as_str() == "btn"),
            "Modal trigger 不应被平铺渲染"
        );
    }

    #[test]
    fn test_tabs_flattens_only_active_tab_child() {
        let mut forest = ComponentForest::new();
        let binding = DataBinding::new(DataModel::new(json!({})));
        let reg = CustomComponentRegistry::new();

        let tabs: Component = Component::from_value(json!({
            "component":"Tabs","id":"root",
            "tabs":[{"title":"T1","child":"a"},{"title":"T2","child":"b"}]
        }))
        .unwrap();
        let a = Component::text(
            ComponentId::new("a").unwrap(),
            DynamicValue::Literal("tab a".to_string()),
        );
        let b = Component::text(
            ComponentId::new("b").unwrap(),
            DynamicValue::Literal("tab b".to_string()),
        );
        forest.upsert("s1", tabs).unwrap();
        forest.upsert("s1", a).unwrap();
        forest.upsert("s1", b).unwrap();

        let builder = WidgetBuilder::new(&binding, &forest, &reg);
        let widgets = builder.build_tree("s1", Rect::new(0, 0, 80, 24));

        assert!(
            widgets.iter().any(|w| w.id().as_str() == "a"),
            "激活（第 0 个）tab 的子组件应被渲染"
        );
        assert!(
            !widgets.iter().any(|w| w.id().as_str() == "b"),
            "非激活 tab 的子组件不应被渲染"
        );
    }

    #[test]
    fn test_modal_missing_content_yields_placeholder() {
        let mut forest = ComponentForest::new();
        let binding = DataBinding::new(DataModel::new(json!({})));
        let reg = CustomComponentRegistry::new();

        let modal: Component =
            Component::from_value(json!({"component":"Modal","id":"root","content":"ghost"}))
                .unwrap();
        forest.upsert("s1", modal).unwrap();

        let builder = WidgetBuilder::new(&binding, &forest, &reg);
        let widgets = builder.build_tree("s1", Rect::new(0, 0, 80, 24));

        assert!(
            widgets.iter().any(|w| matches!(
                w,
                RenderableWidget::Placeholder { id, .. } if id.as_str() == "ghost"
            )),
            "缺失的 content 引用应产生占位符"
        );
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

        let root: Component = Component::from_value(json!({
            "id": "root",
            "component": "Column",
            "children": ["icon", "image", "video", "audio", "field", "check", "date"]
        }))
        .unwrap();
        let icon: Component = Component::from_value(json!({
            "id": "icon",
            "component": "Icon",
            "name": {"path": "/icon"}
        }))
        .unwrap();
        let image: Component = Component::from_value(json!({
            "id": "image",
            "component": "Image",
            "url": {"path": "/image"}
        }))
        .unwrap();
        let video: Component = Component::from_value(json!({
            "id": "video",
            "component": "Video",
            "url": {"path": "/video"}
        }))
        .unwrap();
        let audio: Component = Component::from_value(json!({
            "id": "audio",
            "component": "AudioPlayer",
            "url": {"path": "/audio/url"},
            "description": {"path": "/audio/description"}
        }))
        .unwrap();
        let field: Component = Component::from_value(json!({
            "id": "field",
            "component": "TextField",
            "value": {"path": "/form/value"},
            "placeholder": {"path": "/form/placeholder"}
        }))
        .unwrap();
        let check: Component = Component::from_value(json!({
            "id": "check",
            "component": "CheckBox",
            "checked": true,
            "label": {"path": "/form/label"}
        }))
        .unwrap();
        let date: Component = Component::from_value(json!({
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
        let tf: Component = Component::from_json(
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
        let cb: Component = Component::from_json(
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
        let sl: Component = Component::from_json(
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
        let btn: Component = Component::from_json(
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
        let div: Component = Component::from_json(r#"{"id":"div","component":"Divider"}"#).unwrap();
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
            Component::from_json(r#"{"id":"ic","component":"Icon","name":"star"}"#).unwrap();
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
        let root: Component = Component::from_value(json!({
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
        let root: Component = Component::from_value(json!({
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
        let img: Component = Component::from_json(
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
            Component::from_json(r#"{"id":"card","component":"Card","child":"inner"}"#).unwrap();
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
        let tabs: Component = Component::from_json(
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
        let cp: Component = Component::from_json(
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
            Component::from_json(r#"{"id":"vid","component":"Video","url":"http://x.mp4"}"#)
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
        let aud: Component = Component::from_json(
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
        let modal: Component = Component::from_json(
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
            Component::from_json(r#"{"id":"dt","component":"DateTimeInput","label":"Pick date"}"#)
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
