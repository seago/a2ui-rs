use crate::app::{Message, UserAction};
use crate::iced_renderer::{DynamicStringCacheKey, IcedRenderer};
use a2ui_renderer::component_forest::ComponentTreeNode;
use a2ui_renderer::{
    resolve_dynamic_string_prop_with_missing_path, ComponentStyle, DataBinding, StyleColor,
};
use iced::widget::text;
use iced::widget::text::Shaping;

/// 递归构建 iced Element 树 — 所有数据已 clone，返回 'static
pub fn build_element_tree(
    node: &ComponentTreeNode,
    renderer: &IcedRenderer,
    surface_id: &str,
) -> iced::Element<'static, Message> {
    renderer.record_element_build();

    let ctype = node.component.component_type();

    match ctype {
        "Text" => build_text(node, renderer, surface_id),
        "Button" => build_button(node, renderer, surface_id),
        "Column" => build_column(node, renderer, surface_id),
        "Row" => build_row(node, renderer, surface_id),
        "Image" => build_image(node, renderer, surface_id),
        "Card" => build_card(node, renderer, surface_id),
        "CheckBox" => build_checkbox(node, renderer, surface_id),
        "Divider" => build_divider(),
        "Icon" => build_icon(node, renderer, surface_id),
        "List" => build_list(node, renderer, surface_id),
        "Tabs" => build_tabs(node, renderer, surface_id),
        "Modal" => build_modal(node, renderer, surface_id),
        "Slider" => build_slider(node, renderer, surface_id),
        "TextField" => build_text_field(node, renderer, surface_id),
        "ChoicePicker" => build_choice_picker(node, renderer, surface_id),
        "DateTimeInput" => build_datetime_input(node, renderer, surface_id),
        "Video" => build_placeholder("Video", node, renderer, surface_id),
        "AudioPlayer" => build_placeholder("AudioPlayer", node, renderer, surface_id),
        _ => {
            if renderer.core.custom_registry().is_registered(ctype) {
                text(format!("[custom: {}]", ctype))
                    .shaping(Shaping::Advanced)
                    .into()
            } else {
                text(format!("[unknown: {}]", ctype))
                    .shaping(Shaping::Advanced)
                    .into()
            }
        }
    }
}

// ── 静态 widget ──

fn build_text(
    node: &ComponentTreeNode,
    renderer: &IcedRenderer,
    surface_id: &str,
) -> iced::Element<'static, Message> {
    let props = node.component.properties();
    let content = resolve_dynamic_string(
        props,
        node.component.id(),
        "text",
        "[Text]",
        renderer,
        surface_id,
    );
    apply_text_style(
        text(content).shaping(Shaping::Advanced),
        &ComponentStyle::from_component_props(props),
    )
    .into()
}

fn build_divider() -> iced::Element<'static, Message> {
    iced::widget::horizontal_rule(1).into()
}

fn build_icon(
    node: &ComponentTreeNode,
    renderer: &IcedRenderer,
    surface_id: &str,
) -> iced::Element<'static, Message> {
    let props = node.component.properties();
    let name = resolve_dynamic_string(
        props,
        node.component.id(),
        "name",
        "?",
        renderer,
        surface_id,
    );
    let style = ComponentStyle::from_component_props(props);
    let label = text(name)
        .size(style.font_size.unwrap_or(24.0))
        .shaping(Shaping::Advanced);
    apply_text_style(label, &style).into()
}

fn build_placeholder(
    ctype: &str,
    node: &ComponentTreeNode,
    renderer: &IcedRenderer,
    surface_id: &str,
) -> iced::Element<'static, Message> {
    let props = node.component.properties();
    let label = resolve_dynamic_string(
        props,
        node.component.id(),
        "url",
        ctype,
        renderer,
        surface_id,
    );
    text(format!("[{}: {}]", ctype, label))
        .shaping(Shaping::Advanced)
        .color(iced::Color::from_rgb(0.6, 0.6, 0.6))
        .into()
}

// ── 布局 widget ──

fn build_column(
    node: &ComponentTreeNode,
    renderer: &IcedRenderer,
    surface_id: &str,
) -> iced::Element<'static, Message> {
    let children: Vec<iced::Element<'static, Message>> = node
        .children
        .iter()
        .map(|child| build_element_tree(child, renderer, surface_id))
        .collect();
    let style = ComponentStyle::from_component_props(node.component.properties());
    iced::widget::column(children)
        .spacing(style.spacing.map(|spacing| spacing.y).unwrap_or(8.0))
        .width(iced::Length::Fill)
        .into()
}

fn build_row(
    node: &ComponentTreeNode,
    renderer: &IcedRenderer,
    surface_id: &str,
) -> iced::Element<'static, Message> {
    let children: Vec<iced::Element<'static, Message>> = node
        .children
        .iter()
        .map(|child| build_element_tree(child, renderer, surface_id))
        .collect();
    let style = ComponentStyle::from_component_props(node.component.properties());
    iced::widget::row(children)
        .spacing(style.spacing.map(|spacing| spacing.x).unwrap_or(8.0))
        .width(iced::Length::Fill)
        .into()
}

fn build_card(
    node: &ComponentTreeNode,
    renderer: &IcedRenderer,
    surface_id: &str,
) -> iced::Element<'static, Message> {
    if let Some(child) = node.children.first() {
        let child_el = build_element_tree(child, renderer, surface_id);
        let style = ComponentStyle::from_component_props(node.component.properties());
        let mut container = iced::widget::container(child_el)
            .padding(style.padding.unwrap_or(16.0))
            .width(iced::Length::Fill);

        if style.fill.is_some() || style.radius.is_some() {
            let fill = style.fill;
            let radius = style.radius.unwrap_or(0.0);
            container = container.style(move |_theme| iced::widget::container::Style {
                background: fill.map(|color| iced::Background::Color(to_iced_color(color))),
                border: iced::Border {
                    radius: radius.into(),
                    ..iced::Border::default()
                },
                ..iced::widget::container::Style::default()
            });
        }

        container.into()
    } else {
        text("[Card: empty]").shaping(Shaping::Advanced).into()
    }
}

fn build_list(
    node: &ComponentTreeNode,
    renderer: &IcedRenderer,
    surface_id: &str,
) -> iced::Element<'static, Message> {
    let children: Vec<iced::Element<'static, Message>> = node
        .children
        .iter()
        .map(|child| build_element_tree(child, renderer, surface_id))
        .collect();
    let style = ComponentStyle::from_component_props(node.component.properties());
    let content = iced::widget::column(children)
        .spacing(style.spacing.map(|spacing| spacing.y).unwrap_or(4.0))
        .width(iced::Length::Fill)
        .clip(true);

    let scrollable = iced::widget::scrollable(content)
        .width(iced::Length::Fill)
        .height(iced::Length::Fill);

    iced::widget::container(scrollable)
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .clip(true)
        .into()
}

// ── 交互 widget ──

fn build_button(
    node: &ComponentTreeNode,
    renderer: &IcedRenderer,
    surface_id: &str,
) -> iced::Element<'static, Message> {
    let id = node.component.id().clone();
    let label = resolve_button_label(node, renderer, surface_id);

    iced::widget::button(text(label).shaping(Shaping::Advanced))
        .on_press(Message::UserAction(UserAction::Click { component_id: id }))
        .into()
}

fn build_text_field(
    node: &ComponentTreeNode,
    renderer: &IcedRenderer,
    surface_id: &str,
) -> iced::Element<'static, Message> {
    let props = node.component.properties();
    let id = node.component.id().clone();
    let id_str = id.as_str().to_string();
    let placeholder = resolve_dynamic_string(
        props,
        node.component.id(),
        "placeholder",
        "",
        renderer,
        surface_id,
    );
    let initial_value = resolve_dynamic_string(
        props,
        node.component.id(),
        "value",
        "",
        renderer,
        surface_id,
    );
    let is_secure = matches!(
        props.get("variant").and_then(|v| v.as_str()),
        Some("obscured")
    );

    // 从本地状态读取当前输入值（用户输入后更新）
    let current_value = renderer
        .text_input_values
        .borrow()
        .get(&id_str)
        .cloned()
        .unwrap_or(initial_value);

    let id_for_input = id.clone();
    iced::widget::text_input(&placeholder, &current_value)
        .secure(is_secure)
        .on_input(move |new_value| {
            Message::UserAction(UserAction::TextInput {
                component_id: id_for_input.clone(),
                value: new_value,
            })
        })
        .into()
}

fn build_checkbox(
    node: &ComponentTreeNode,
    renderer: &IcedRenderer,
    surface_id: &str,
) -> iced::Element<'static, Message> {
    let props = node.component.properties();
    let id = node.component.id().clone();
    let id_str = id.as_str().to_string();
    let label = resolve_dynamic_string(
        props,
        node.component.id(),
        "label",
        "",
        renderer,
        surface_id,
    );
    let checked = renderer
        .checkbox_values
        .borrow()
        .get(&id_str)
        .copied()
        .unwrap_or_else(|| resolve_dynamic_bool(props, "value", false, renderer, surface_id));

    iced::widget::checkbox(label, checked)
        .text_shaping(Shaping::Advanced)
        .on_toggle(move |is_checked| {
            Message::UserAction(UserAction::CheckToggle {
                component_id: id.clone(),
                checked: is_checked,
            })
        })
        .into()
}

fn build_slider(
    node: &ComponentTreeNode,
    renderer: &IcedRenderer,
    surface_id: &str,
) -> iced::Element<'static, Message> {
    let props = node.component.properties();
    let id = node.component.id().clone();
    let id_str = id.as_str().to_string();
    let initial_value = resolve_dynamic_number(props, "value", 0.0, renderer, surface_id);
    let value = renderer
        .slider_values
        .borrow()
        .get(&id_str)
        .copied()
        .unwrap_or(initial_value);
    let min = props.get("min").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let max = props.get("max").and_then(|v| v.as_f64()).unwrap_or(100.0);

    iced::widget::slider(min..=max, value, move |val| {
        Message::UserAction(UserAction::SliderChange {
            component_id: id.clone(),
            value: val,
        })
    })
    .into()
}

// ── 复杂 widget ──

fn build_image(
    node: &ComponentTreeNode,
    renderer: &IcedRenderer,
    surface_id: &str,
) -> iced::Element<'static, Message> {
    let props = node.component.properties();
    let url = resolve_dynamic_string(props, node.component.id(), "url", "", renderer, surface_id);
    let style = ComponentStyle::from_component_props(props);
    if url.is_empty() {
        return apply_text_style(text("[Image: no URL]").shaping(Shaping::Advanced), &style).into();
    }

    if let Some(handle) = renderer.load_image_handle(&url) {
        let width = extract_length_prop(props, "width", iced::Length::Shrink);
        let height = extract_length_prop(props, "height", iced::Length::Shrink);
        let image = iced::widget::image(handle)
            .width(width)
            .height(height)
            .content_fit(iced::ContentFit::Cover);

        let mut container = iced::widget::container(image)
            .width(width)
            .height(height)
            .clip(true);

        if style.fill.is_some() || style.radius.is_some() {
            let fill = style.fill;
            let radius = style.radius.unwrap_or(0.0);
            container = container.style(move |_theme| iced::widget::container::Style {
                background: fill.map(|color| iced::Background::Color(to_iced_color(color))),
                border: iced::Border {
                    radius: radius.into(),
                    ..iced::Border::default()
                },
                ..iced::widget::container::Style::default()
            });
        }

        container.into()
    } else {
        apply_text_style(
            text(format!("[Image: {}]", url))
                .shaping(Shaping::Advanced)
                .color(iced::Color::from_rgb(0.6, 0.6, 0.6)),
            &style,
        )
        .into()
    }
}

/// 在 node.children 中按 props\[prop_key\] 指定的组件 id 查找子节点。
/// 组件树的 children 顺序是构建细节，消费端一律按 id 匹配。
fn select_child_by_prop<'a>(
    node: &'a ComponentTreeNode,
    prop_key: &str,
) -> Option<&'a ComponentTreeNode> {
    let target = node.component.properties().get(prop_key)?.as_str()?;
    node.children
        .iter()
        .find(|c| c.component.id().as_str() == target)
}

/// 按 tabs\[index\].child 的组件 id 在 node.children 中查找对应子节点。
fn select_tab_child(node: &ComponentTreeNode, index: usize) -> Option<&ComponentTreeNode> {
    let tabs = node.component.properties().get("tabs")?.as_array()?;
    let target = tabs.get(index)?.get("child")?.as_str()?;
    node.children
        .iter()
        .find(|c| c.component.id().as_str() == target)
}

fn build_tabs(
    node: &ComponentTreeNode,
    renderer: &IcedRenderer,
    surface_id: &str,
) -> iced::Element<'static, Message> {
    // 用简单的按钮行 + 激活（第 0 个）tab 内容近似实现
    let tab_labels: Vec<String> = node
        .component
        .properties()
        .get("tabs")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|tab| tab.get("title").and_then(|t| t.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let tab_buttons: Vec<iced::Element<'static, Message>> = tab_labels
        .iter()
        .map(|title| iced::widget::button(text(title.clone()).shaping(Shaping::Advanced)).into())
        .collect();

    // 按 id 匹配激活 tab 的子节点；无 tabs 元数据时降级取第一个子节点
    // （有元数据但组件缺失时不降级，避免渲染到错误的子节点）
    let active_child = if node.component.properties().get("tabs").is_some() {
        select_tab_child(node, 0)
    } else {
        node.children.first()
    };
    let content = if let Some(child) = active_child {
        build_element_tree(child, renderer, surface_id)
    } else {
        text("[Tabs: no content]").shaping(Shaping::Advanced).into()
    };

    iced::widget::column(vec![
        iced::widget::row(tab_buttons).spacing(4).into(),
        iced::widget::horizontal_rule(1).into(),
        content,
    ])
    .spacing(8)
    .into()
}

fn build_modal(
    node: &ComponentTreeNode,
    renderer: &IcedRenderer,
    surface_id: &str,
) -> iced::Element<'static, Message> {
    // 按 props.content 的 id 匹配（children 可能同时含 trigger）；
    // 无 content 元数据时降级取第一个子节点
    // （有元数据但组件缺失时不降级，避免把 trigger 当 content 渲染）
    let content_child = if node.component.properties().get("content").is_some() {
        select_child_by_prop(node, "content")
    } else {
        node.children.first()
    };
    if let Some(content_child) = content_child {
        let content = build_element_tree(content_child, renderer, surface_id);
        iced::widget::container(content).padding(16).into()
    } else {
        text("[Modal: empty]").shaping(Shaping::Advanced).into()
    }
}

fn build_choice_picker(
    node: &ComponentTreeNode,
    renderer: &IcedRenderer,
    surface_id: &str,
) -> iced::Element<'static, Message> {
    let props = node.component.properties();
    let options: Vec<String> = props
        .get("options")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|o| o.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let label = resolve_dynamic_string(
        props,
        node.component.id(),
        "label",
        "选择:",
        renderer,
        surface_id,
    );
    let option_texts: Vec<iced::Element<'static, Message>> = options
        .iter()
        .map(|opt| text(opt.clone()).shaping(Shaping::Advanced).into())
        .collect();

    iced::widget::column(vec![
        text(label).size(16).shaping(Shaping::Advanced).into(),
        iced::widget::column(option_texts).spacing(4).into(),
    ])
    .spacing(4)
    .into()
}

fn build_datetime_input(
    node: &ComponentTreeNode,
    renderer: &IcedRenderer,
    surface_id: &str,
) -> iced::Element<'static, Message> {
    let props = node.component.properties();
    let label = resolve_dynamic_string(
        props,
        node.component.id(),
        "label",
        "[DateTimeInput]",
        renderer,
        surface_id,
    );
    iced::widget::button(text(label).shaping(Shaping::Advanced)).into()
}

// ── 辅助函数 ──

fn binding_for<'a>(renderer: &'a IcedRenderer, surface_id: &str) -> Option<&'a DataBinding> {
    renderer.core.binding(surface_id)
}

fn resolve_dynamic_string(
    props: &serde_json::Value,
    component_id: &a2ui_core::prelude::ComponentId,
    key: &str,
    default: &str,
    renderer: &IcedRenderer,
    surface_id: &str,
) -> String {
    let cache_key = DynamicStringCacheKey {
        surface_id: surface_id.to_string(),
        component_id: component_id.as_str().to_string(),
        prop: key.to_string(),
    };

    if let Some(cached) = renderer
        .dynamic_string_cache
        .borrow()
        .get(&cache_key)
        .cloned()
    {
        renderer.record_dynamic_string_cache_hit();
        return cached;
    }

    let resolved = resolve_dynamic_string_prop_with_missing_path(
        props,
        key,
        binding_for(renderer, surface_id),
        default,
        |path| format!("{{{}…}}", path),
    );
    renderer
        .dynamic_string_cache
        .borrow_mut()
        .insert(cache_key, resolved.clone());
    renderer.record_dynamic_string_cache_miss();
    resolved
}

fn resolve_dynamic_bool(
    props: &serde_json::Value,
    key: &str,
    default: bool,
    renderer: &IcedRenderer,
    surface_id: &str,
) -> bool {
    let Some(value) = props.get(key) else {
        return default;
    };
    if let Some(value) = value.as_bool() {
        return value;
    }
    let Some(path) = value
        .as_object()
        .and_then(|obj| obj.get("path"))
        .and_then(|v| v.as_str())
    else {
        return default;
    };
    binding_for(renderer, surface_id)
        .and_then(|binding| binding.get(path))
        .and_then(|resolved| resolved.as_bool())
        .unwrap_or(default)
}

fn resolve_dynamic_number(
    props: &serde_json::Value,
    key: &str,
    default: f64,
    renderer: &IcedRenderer,
    surface_id: &str,
) -> f64 {
    let Some(value) = props.get(key) else {
        return default;
    };
    if let Some(value) = value.as_f64() {
        return value;
    }
    let Some(path) = value
        .as_object()
        .and_then(|obj| obj.get("path"))
        .and_then(|v| v.as_str())
    else {
        return default;
    };
    binding_for(renderer, surface_id)
        .and_then(|binding| binding.get(path))
        .and_then(|resolved| resolved.as_f64())
        .unwrap_or(default)
}

fn extract_length_prop(
    props: &serde_json::Value,
    key: &str,
    default: iced::Length,
) -> iced::Length {
    match props.get(key) {
        Some(serde_json::Value::Number(n)) => n
            .as_f64()
            .map(|value| iced::Length::Fixed(value as f32))
            .unwrap_or(default),
        Some(serde_json::Value::String(s)) if s == "fill" => iced::Length::Fill,
        Some(serde_json::Value::String(s)) if s == "shrink" => iced::Length::Shrink,
        _ => default,
    }
}

fn apply_text_style(
    mut label: iced::widget::Text<'static, iced::Theme, iced::Renderer>,
    style: &ComponentStyle,
) -> iced::widget::Text<'static, iced::Theme, iced::Renderer> {
    if let Some(size) = style.font_size {
        label = label.size(size);
    }
    if let Some(color) = style.color {
        label = label.color(to_iced_color(color));
    }
    label
}

fn to_iced_color(color: StyleColor) -> iced::Color {
    iced::Color::from_rgba8(color.r, color.g, color.b, color.a as f32 / 255.0)
}

fn resolve_button_label(
    node: &ComponentTreeNode,
    renderer: &IcedRenderer,
    surface_id: &str,
) -> String {
    let props = node.component.properties();
    if props.get("text").is_some() {
        let label =
            resolve_dynamic_string(props, node.component.id(), "text", "", renderer, surface_id);
        if !label.is_empty() {
            return label;
        }
    }

    // 回退到 child Text 组件
    for child in &node.children {
        if child.component.component_type() == "Text" {
            let child_text = resolve_dynamic_string(
                child.component.properties(),
                child.component.id(),
                "text",
                "",
                renderer,
                surface_id,
            );
            if !child_text.is_empty() {
                return child_text;
            }
        }
    }

    format!("[Button {}]", node.component.id())
}

#[cfg(test)]
mod tests {
    use super::*;
    use a2ui_core::component::component::Component;
    use a2ui_core::message::server_to_client::CreateSurface;
    use a2ui_core::ComponentId;
    use serde_json::json;

    fn tree_node(json: serde_json::Value) -> ComponentTreeNode {
        ComponentTreeNode::new(serde_json::from_value(json).unwrap())
    }

    /// 经核心创建带数据模型的 surface（pub 字段已收敛为 core 访问器，
    /// 测试不再直接拼装 data_bindings）；带一个占位根组件以满足
    /// 核心 createSurface 流水线（空 forest 无法展开模板）
    fn renderer_with_binding(surface_id: &str, data_model: serde_json::Value) -> IcedRenderer {
        let mut renderer = IcedRenderer::new();
        let root = Component::text(
            ComponentId::new("surface_root").unwrap(),
            a2ui_core::prelude::DynamicValue::Literal(String::new()),
        );
        pollster::block_on(renderer.core.create_surface(CreateSurface {
            surface_id: surface_id.into(),
            catalog_id: "a2ui://catalogs/basic/v1".into(),
            surface_properties: None,
            send_data_model: false,
            components: Some(vec![root]),
            data_model: Some(data_model),
        }))
        .unwrap();
        renderer
    }

    #[test]
    fn test_select_child_by_prop_matches_by_id_not_position() {
        // children 故意乱序为 [trigger, content]，必须按 props.content 的 id 匹配
        let modal = tree_node(json!({
            "component":"Modal","id":"m","content":"body","trigger":"btn"
        }))
        .with_children(vec![
            tree_node(json!({"component":"Button","id":"btn","label":"open"})),
            tree_node(json!({"component":"Text","id":"body","text":"hello"})),
        ]);

        let selected = select_child_by_prop(&modal, "content").expect("content child");
        assert_eq!(selected.component.id().as_str(), "body");
    }

    #[test]
    fn test_select_child_by_prop_missing_returns_none() {
        let modal = tree_node(json!({"component":"Modal","id":"m","content":"ghost"}));
        assert!(select_child_by_prop(&modal, "content").is_none());
    }

    #[test]
    fn test_select_tab_child_matches_by_id() {
        let tabs = tree_node(json!({
            "component":"Tabs","id":"t",
            "tabs":[{"title":"T1","child":"a"},{"title":"T2","child":"b"}]
        }))
        .with_children(vec![
            // 乱序：b 在前
            tree_node(json!({"component":"Text","id":"b","text":"tab b"})),
            tree_node(json!({"component":"Text","id":"a","text":"tab a"})),
        ]);

        let first = select_tab_child(&tabs, 0).expect("tab 0 child");
        assert_eq!(first.component.id().as_str(), "a");
        let second = select_tab_child(&tabs, 1).expect("tab 1 child");
        assert_eq!(second.component.id().as_str(), "b");
        assert!(select_tab_child(&tabs, 2).is_none());
    }

    #[test]
    fn test_resolve_button_label_from_props() {
        let surface_id = "s1";
        let renderer = renderer_with_binding(surface_id, json!({"label": "Click Me"}));

        let comp: Component = serde_json::from_value(json!({
            "component": "Button",
            "id": "btn",
            "text": {"path": "/label"}
        }))
        .unwrap();
        let node = ComponentTreeNode::new(comp);

        let label = resolve_button_label(&node, &renderer, surface_id);
        assert_eq!(label, "Click Me");
    }

    #[test]
    fn test_dynamic_string_props_resolve_from_surface_binding() {
        let surface_id = "s1";
        let renderer = renderer_with_binding(
            surface_id,
            json!({
                "title": "Welcome",
                "icon": "star",
                "remember": "记住密码",
                "form": {
                    "value": "Alice",
                    "placeholder": "请输入用户名"
                }
            }),
        );

        let text_id = ComponentId::new("title").unwrap();
        let icon_id = ComponentId::new("icon").unwrap();
        let checkbox_id = ComponentId::new("remember").unwrap();
        let field_id = ComponentId::new("field").unwrap();
        let missing_id = ComponentId::new("missing").unwrap();

        let title = json!({"text": {"path": "/title"}});
        assert_eq!(
            resolve_dynamic_string(&title, &text_id, "text", "[Text]", &renderer, surface_id),
            "Welcome"
        );

        let icon = json!({"name": {"path": "/icon"}});
        assert_eq!(
            resolve_dynamic_string(&icon, &icon_id, "name", "?", &renderer, surface_id),
            "star"
        );

        let checkbox = json!({"label": {"path": "/remember"}});
        assert_eq!(
            resolve_dynamic_string(&checkbox, &checkbox_id, "label", "", &renderer, surface_id),
            "记住密码"
        );

        let text_field = json!({
            "value": {"path": "/form/value"},
            "placeholder": {"path": "/form/placeholder"}
        });
        assert_eq!(
            resolve_dynamic_string(&text_field, &field_id, "value", "", &renderer, surface_id),
            "Alice"
        );
        assert_eq!(
            resolve_dynamic_string(
                &text_field,
                &field_id,
                "placeholder",
                "",
                &renderer,
                surface_id
            ),
            "请输入用户名"
        );

        let missing = json!({"text": {"path": "/missing"}});
        assert_eq!(
            resolve_dynamic_string(&missing, &missing_id, "text", "", &renderer, surface_id),
            "{/missing…}"
        );
    }

    #[test]
    fn test_dynamic_string_cache_profiles_hits_and_misses() {
        let surface_id = "s1";
        let renderer = renderer_with_binding(surface_id, json!({"title": "Welcome"}));
        let component_id = ComponentId::new("title").unwrap();
        let props = json!({"text": {"path": "/title"}});

        assert_eq!(
            resolve_dynamic_string(&props, &component_id, "text", "", &renderer, surface_id),
            "Welcome"
        );
        assert_eq!(
            resolve_dynamic_string(&props, &component_id, "text", "", &renderer, surface_id),
            "Welcome"
        );

        let profile = renderer.profile_snapshot();
        assert_eq!(profile.dynamic_string_cache_misses, 1);
        assert_eq!(profile.dynamic_string_cache_hits, 1);
    }

    #[test]
    fn test_dynamic_bool_and_number_resolve_from_surface_binding() {
        let surface_id = "s1";
        let renderer = renderer_with_binding(
            surface_id,
            json!({
                "remember": true,
                "volume": 75.0
            }),
        );

        let checkbox_props = json!({"value": {"path": "/remember"}});
        assert!(resolve_dynamic_bool(
            &checkbox_props,
            "value",
            false,
            &renderer,
            surface_id
        ));

        let slider_props = json!({"value": {"path": "/volume"}});
        assert_eq!(
            resolve_dynamic_number(&slider_props, "value", 0.0, &renderer, surface_id),
            75.0
        );
    }

    #[test]
    fn test_resolve_button_label_from_dynamic_child_text() {
        let surface_id = "s1";
        let renderer = renderer_with_binding(surface_id, json!({"child_label": "Child Label"}));

        let child = ComponentTreeNode::new(
            serde_json::from_value(json!({
                "component": "Text",
                "id": "label",
                "text": {"path": "/child_label"}
            }))
            .unwrap(),
        );
        let button = ComponentTreeNode::new(
            serde_json::from_value(json!({
                "component": "Button",
                "id": "btn",
                "child": "label"
            }))
            .unwrap(),
        )
        .with_children(vec![child]);

        assert_eq!(
            resolve_button_label(&button, &renderer, surface_id),
            "Child Label"
        );
    }

    #[test]
    fn test_text_field_uses_dynamic_initial_value_without_merging_component_state() {
        let surface_id = "login";
        let renderer = renderer_with_binding(
            surface_id,
            json!({
                "username": "Alice",
                "password": "Secret",
                "username_placeholder": "请输入用户名",
                "password_placeholder": "请输入密码"
            }),
        );

        let username: Component = serde_json::from_value(json!({
            "component": "TextField",
            "id": "username_field",
            "value": {"path": "/username"},
            "placeholder": {"path": "/username_placeholder"},
            "variant": "shortText"
        }))
        .unwrap();
        let password: Component = serde_json::from_value(json!({
            "component": "TextField",
            "id": "password_field",
            "value": {"path": "/password"},
            "placeholder": {"path": "/password_placeholder"},
            "variant": "obscured"
        }))
        .unwrap();

        let _username_el =
            build_element_tree(&ComponentTreeNode::new(username), &renderer, surface_id);
        let _password_el =
            build_element_tree(&ComponentTreeNode::new(password), &renderer, surface_id);

        assert!(renderer.text_input_values.borrow().is_empty());

        renderer
            .text_input_values
            .borrow_mut()
            .insert("username_field".to_string(), "Bob".to_string());
        renderer
            .text_input_values
            .borrow_mut()
            .insert("password_field".to_string(), "New Secret".to_string());

        let username: Component = serde_json::from_value(json!({
            "component": "TextField",
            "id": "username_field",
            "value": {"path": "/username"},
            "placeholder": {"path": "/username_placeholder"},
            "variant": "shortText"
        }))
        .unwrap();
        let password: Component = serde_json::from_value(json!({
            "component": "TextField",
            "id": "password_field",
            "value": {"path": "/password"},
            "placeholder": {"path": "/password_placeholder"},
            "variant": "obscured"
        }))
        .unwrap();

        let _username_el =
            build_element_tree(&ComponentTreeNode::new(username), &renderer, surface_id);
        let _password_el =
            build_element_tree(&ComponentTreeNode::new(password), &renderer, surface_id);

        let values = renderer.text_input_values.borrow();
        assert_eq!(
            values.get("username_field").map(String::as_str),
            Some("Bob")
        );
        assert_eq!(
            values.get("password_field").map(String::as_str),
            Some("New Secret")
        );
        assert!(!values.contains_key("__input__"));
    }

    #[test]
    fn test_styled_widgets_build_without_panic() {
        let renderer = IcedRenderer::new();
        let surface_id = "styled";
        let image_url = "cached://image";
        renderer
            .image_cache
            .borrow_mut()
            .insert(image_url.to_string(), vec![137, 80, 78, 71]);
        let style = json!({
            "fontSize": 18,
            "strong": true,
            "color": "#112233",
            "fill": "#44556680",
            "padding": 9,
            "spacing": {"x": 7, "y": 11},
            "radius": 5
        });

        let text_node = ComponentTreeNode::new(
            serde_json::from_value(json!({
                "component": "Text",
                "id": "styled_text",
                "text": "Styled",
                "style": style.clone()
            }))
            .unwrap(),
        );
        let icon_node = ComponentTreeNode::new(
            serde_json::from_value(json!({
                "component": "Icon",
                "id": "styled_icon",
                "name": "star",
                "style": style.clone()
            }))
            .unwrap(),
        );
        let row_node = ComponentTreeNode::new(
            serde_json::from_value(json!({
                "component": "Row",
                "id": "styled_row",
                "children": ["styled_text", "styled_icon"],
                "style": style.clone()
            }))
            .unwrap(),
        )
        .with_children(vec![text_node, icon_node]);
        let image_node = ComponentTreeNode::new(
            serde_json::from_value(json!({
                "component": "Image",
                "id": "styled_image",
                "url": image_url,
                "width": 120,
                "height": 80,
                "style": style.clone()
            }))
            .unwrap(),
        );
        let card_node = ComponentTreeNode::new(
            serde_json::from_value(json!({
                "component": "Card",
                "id": "styled_card",
                "child": "styled_row",
                "style": style.clone()
            }))
            .unwrap(),
        )
        .with_children(vec![row_node]);
        let column_node = ComponentTreeNode::new(
            serde_json::from_value(json!({
                "component": "Column",
                "id": "styled_column",
                "children": ["styled_card", "styled_image"],
                "style": style.clone()
            }))
            .unwrap(),
        )
        .with_children(vec![card_node, image_node]);
        let list_node = ComponentTreeNode::new(
            serde_json::from_value(json!({
                "component": "List",
                "id": "styled_list",
                "children": ["styled_column"],
                "style": style
            }))
            .unwrap(),
        )
        .with_children(vec![column_node]);

        let _el = build_element_tree(&list_node, &renderer, surface_id);
    }

    #[test]
    fn test_style_color_converts_to_iced_rgba() {
        let color = to_iced_color(StyleColor {
            r: 17,
            g: 34,
            b: 51,
            a: 128,
        });

        assert!((color.r - 17.0 / 255.0).abs() < f32::EPSILON);
        assert!((color.g - 34.0 / 255.0).abs() < f32::EPSILON);
        assert!((color.b - 51.0 / 255.0).abs() < f32::EPSILON);
        assert!((color.a - 128.0 / 255.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_text_field_state_uses_component_id() {
        let renderer = IcedRenderer::new();
        renderer
            .text_input_values
            .borrow_mut()
            .insert("username_field".to_string(), "alice".to_string());
        renderer
            .text_input_values
            .borrow_mut()
            .insert("password_field".to_string(), "secret".to_string());

        let username: Component = serde_json::from_value(json!({
            "component": "TextField",
            "id": "username_field",
            "value": "",
            "placeholder": "请输入用户名",
            "variant": "shortText"
        }))
        .unwrap();
        let password: Component = serde_json::from_value(json!({
            "component": "TextField",
            "id": "password_field",
            "value": "",
            "placeholder": "请输入密码",
            "variant": "obscured"
        }))
        .unwrap();

        let _username_el =
            build_element_tree(&ComponentTreeNode::new(username), &renderer, "login");
        let _password_el =
            build_element_tree(&ComponentTreeNode::new(password), &renderer, "login");

        let values = renderer.text_input_values.borrow();
        assert_eq!(
            values.get("username_field").map(String::as_str),
            Some("alice")
        );
        assert_eq!(
            values.get("password_field").map(String::as_str),
            Some("secret")
        );
        assert!(!values.contains_key("__input__"));
    }

    #[test]
    fn test_checkbox_state_uses_component_id() {
        let renderer = IcedRenderer::new();
        renderer
            .checkbox_values
            .borrow_mut()
            .insert("remember_cb".to_string(), true);

        let checkbox: Component = serde_json::from_value(json!({
            "component": "CheckBox",
            "id": "remember_cb",
            "value": false,
            "label": "记住密码"
        }))
        .unwrap();

        let _checkbox_el =
            build_element_tree(&ComponentTreeNode::new(checkbox), &renderer, "login");

        let values = renderer.checkbox_values.borrow();
        assert_eq!(values.get("remember_cb").copied(), Some(true));
        assert!(!values.contains_key("__cb__"));
    }

    #[test]
    fn test_slider_state_uses_component_id() {
        let renderer = IcedRenderer::new();
        renderer
            .slider_values
            .borrow_mut()
            .insert("volume_slider".to_string(), 75.0);

        let slider: Component = serde_json::from_value(json!({
            "component": "Slider",
            "id": "volume_slider",
            "value": 10.0,
            "min": 0.0,
            "max": 100.0
        }))
        .unwrap();

        let _slider_el = build_element_tree(&ComponentTreeNode::new(slider), &renderer, "settings");

        let values = renderer.slider_values.borrow();
        assert_eq!(values.get("volume_slider").copied(), Some(75.0));
        assert!(!values.contains_key("__slider__"));
    }

    #[test]
    fn test_all_18_types_build_without_panic() {
        let renderer = IcedRenderer::new();
        let surface_id = "s1";
        let types = [
            ("Text", json!({"component":"Text","id":"t","text":"hi"})),
            ("Button", json!({"component":"Button","id":"b","child":"t"})),
            (
                "Column",
                json!({"component":"Column","id":"col","children":[]}),
            ),
            ("Row", json!({"component":"Row","id":"r","children":[]})),
            ("Image", json!({"component":"Image","id":"img","url":""})),
            ("Card", json!({"component":"Card","id":"c","child":"t"})),
            (
                "CheckBox",
                json!({"component":"CheckBox","id":"cb","value":false,"label":"x"}),
            ),
            ("Divider", json!({"component":"Divider","id":"d"})),
            ("Icon", json!({"component":"Icon","id":"ic","name":"★"})),
            ("List", json!({"component":"List","id":"l","children":[]})),
            ("Tabs", json!({"component":"Tabs","id":"tabs","tabs":[]})),
            ("Modal", json!({"component":"Modal","id":"m","content":"m"})),
            (
                "Slider",
                json!({"component":"Slider","id":"s","value":50.0,"min":0.0,"max":100.0}),
            ),
            (
                "TextField",
                json!({"component":"TextField","id":"tf","value":"","placeholder":"输入"}),
            ),
            (
                "ChoicePicker",
                json!({"component":"ChoicePicker","id":"cp","options":["a","b"]}),
            ),
            (
                "DateTimeInput",
                json!({"component":"DateTimeInput","id":"dt","label":"日期"}),
            ),
            (
                "Video",
                json!({"component":"Video","id":"v","url":"http://x.com/v.mp4"}),
            ),
            (
                "AudioPlayer",
                json!({"component":"AudioPlayer","id":"ap","url":"http://x.com/a.mp3"}),
            ),
        ];

        for (_name, json_val) in &types {
            let comp: Component = serde_json::from_value(json_val.clone()).unwrap();
            let node = ComponentTreeNode::new(comp);
            // 不应 panic
            let _el = build_element_tree(&node, &renderer, surface_id);
        }
    }
}
