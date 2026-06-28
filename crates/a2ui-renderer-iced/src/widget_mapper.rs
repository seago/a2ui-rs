use crate::app::{Message, UserAction};
use crate::iced_renderer::IcedRenderer;
use a2ui_renderer::component_forest::ComponentTreeNode;
use a2ui_renderer::{ComponentStyle, StyleColor};
use iced::widget::text;
use iced::widget::text::Shaping;

/// 递归构建 iced Element 树 — 所有数据已 clone，返回 'static
pub fn build_element_tree(
    node: &ComponentTreeNode,
    renderer: &IcedRenderer,
    surface_id: &str,
) -> iced::Element<'static, Message> {
    let ctype = node.component.component_type();
    let props = node.component.properties();

    match ctype {
        "Text" => build_text(node, renderer, surface_id),
        "Button" => build_button(node, renderer, surface_id),
        "Column" => build_column(node, renderer, surface_id),
        "Row" => build_row(node, renderer, surface_id),
        "Image" => build_image(props, renderer),
        "Card" => build_card(node, renderer, surface_id),
        "CheckBox" => build_checkbox(node, renderer),
        "Divider" => build_divider(),
        "Icon" => build_icon(props),
        "List" => build_list(node, renderer, surface_id),
        "Tabs" => build_tabs(node, renderer, surface_id),
        "Modal" => build_modal(node, renderer, surface_id),
        "Slider" => build_slider(node, renderer),
        "TextField" => build_text_field(node, renderer),
        "ChoicePicker" => build_choice_picker(props),
        "DateTimeInput" => build_datetime_input(props),
        "Video" => build_placeholder("Video", props),
        "AudioPlayer" => build_placeholder("AudioPlayer", props),
        _ => {
            if renderer.custom_registry.is_registered(ctype) {
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
    let content = resolve_dynamic_string(props, "text", "[Text]", renderer, surface_id);
    apply_text_style(
        text(content).shaping(Shaping::Advanced),
        &ComponentStyle::from_component_props(props),
    )
    .into()
}

fn build_divider() -> iced::Element<'static, Message> {
    iced::widget::horizontal_rule(1).into()
}

fn build_icon(props: &serde_json::Value) -> iced::Element<'static, Message> {
    let name = extract_string_prop(props, "name", "?");
    let style = ComponentStyle::from_component_props(props);
    let label = text(name)
        .size(style.font_size.unwrap_or(24.0))
        .shaping(Shaping::Advanced);
    apply_text_style(label, &style).into()
}

fn build_placeholder(ctype: &str, props: &serde_json::Value) -> iced::Element<'static, Message> {
    let label = extract_string_prop(props, "url", ctype);
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
) -> iced::Element<'static, Message> {
    let props = node.component.properties();
    let id = node.component.id().clone();
    let id_str = id.as_str().to_string();
    let placeholder = extract_string_prop(props, "placeholder", "");
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
        .unwrap_or_default();

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
) -> iced::Element<'static, Message> {
    let props = node.component.properties();
    let id = node.component.id().clone();
    let id_str = id.as_str().to_string();
    let label = extract_string_prop(props, "label", "");
    let checked = renderer
        .checkbox_values
        .borrow()
        .get(&id_str)
        .copied()
        .unwrap_or_else(|| {
            props
                .get("value")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        });

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
) -> iced::Element<'static, Message> {
    let props = node.component.properties();
    let id = node.component.id().clone();
    let id_str = id.as_str().to_string();
    let initial_value = props.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
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
    props: &serde_json::Value,
    renderer: &IcedRenderer,
) -> iced::Element<'static, Message> {
    let url = extract_string_prop(props, "url", "");
    let style = ComponentStyle::from_component_props(props);
    if url.is_empty() {
        return apply_text_style(text("[Image: no URL]").shaping(Shaping::Advanced), &style).into();
    }

    if let Some(bytes) = renderer.load_image_bytes(&url) {
        let width = extract_length_prop(props, "width", iced::Length::Shrink);
        let height = extract_length_prop(props, "height", iced::Length::Shrink);
        let image = iced::widget::image(iced::widget::image::Handle::from_bytes(bytes))
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

fn build_tabs(
    node: &ComponentTreeNode,
    renderer: &IcedRenderer,
    surface_id: &str,
) -> iced::Element<'static, Message> {
    // 用简单的按钮行 + 第一个 tab 内容近似实现
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

    let content = if let Some(first_child) = node.children.first() {
        build_element_tree(first_child, renderer, surface_id)
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
    if let Some(content_child) = node.children.first() {
        let content = build_element_tree(content_child, renderer, surface_id);
        iced::widget::container(content).padding(16).into()
    } else {
        text("[Modal: empty]").shaping(Shaping::Advanced).into()
    }
}

fn build_choice_picker(props: &serde_json::Value) -> iced::Element<'static, Message> {
    let options: Vec<String> = props
        .get("options")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|o| o.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let label = extract_string_prop(props, "label", "选择:");
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

fn build_datetime_input(props: &serde_json::Value) -> iced::Element<'static, Message> {
    let label = extract_string_prop(props, "label", "[DateTimeInput]");
    iced::widget::button(text(label).shaping(Shaping::Advanced)).into()
}

// ── 辅助函数 ──

fn resolve_dynamic_string(
    props: &serde_json::Value,
    key: &str,
    default: &str,
    renderer: &IcedRenderer,
    surface_id: &str,
) -> String {
    if let Some(val) = props.get(key) {
        // 字面量字符串
        if let Some(s) = val.as_str() {
            return s.to_string();
        }
        // path 绑定: {"path": "/login_status"}
        if let Some(obj) = val.as_object() {
            if let Some(p) = obj.get("path").and_then(|v| v.as_str()) {
                if let Some(binding) = renderer.data_bindings.get(surface_id) {
                    if let Some(resolved) = binding.get(p) {
                        if !resolved.is_null() {
                            return match resolved {
                                serde_json::Value::String(s) => s.clone(),
                                other => other.to_string(),
                            };
                        }
                    }
                }
                return format!("{{{}…}}", p);
            }
        }
    }
    default.to_string()
}

fn extract_string_prop(props: &serde_json::Value, key: &str, default: &str) -> String {
    props
        .get(key)
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| default.to_string())
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
    let text = extract_string_prop(props, "text", "");

    // 如果 text 属性非空且不是占位符，直接使用
    if !text.is_empty()
        && !text.starts_with('[')
        && !text.starts_with("{\"path\"")
        && !text.starts_with("{\"call\"")
    {
        return text;
    }

    // 尝试解析 path 绑定
    if let Some(path) = props.get("text").and_then(|v| v.as_object()) {
        if let Some(p) = path.get("path").and_then(|v| v.as_str()) {
            if let Some(binding) = renderer.data_bindings.get(surface_id) {
                if let Some(resolved) = binding.get(p) {
                    if let Some(s) = resolved.as_str() {
                        return s.to_string();
                    }
                    return resolved.to_string();
                }
            }
            return format!("{{{}…}}", p);
        }
    }

    // 回退到 child Text 组件
    for child in &node.children {
        if child.component.component_type() == "Text" {
            let child_text = extract_string_prop(child.component.properties(), "text", "");
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
    use a2ui_core::DataModel;
    use a2ui_renderer::DataBinding;
    use serde_json::json;

    #[test]
    fn test_extract_string_prop_literal() {
        let props = json!({"text": "Hello"});
        assert_eq!(extract_string_prop(&props, "text", ""), "Hello");
    }

    #[test]
    fn test_extract_string_prop_default() {
        let props = json!({});
        assert_eq!(
            extract_string_prop(&props, "missing", "fallback"),
            "fallback"
        );
    }

    #[test]
    fn test_resolve_button_label_from_props() {
        let mut renderer = IcedRenderer::new();
        let surface_id = "s1";
        let dm = DataModel::new(json!({"label": "Click Me"}));
        renderer
            .data_bindings
            .insert(surface_id.to_string(), DataBinding::new(dm));
        renderer.surface_order.push(surface_id.to_string());

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
