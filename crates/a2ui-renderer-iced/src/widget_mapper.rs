use crate::app::{Message, UserAction};
use crate::iced_renderer::{DynamicStringCacheKey, IcedRenderer};
use a2ui_core::component::prop_keys;
use a2ui_core::prelude::Component;
use a2ui_renderer::component_forest::ComponentTreeNode;
use a2ui_renderer::{
    checkbox_checked, choice_options, choice_selected, resolve_bool, resolve_f64,
    resolve_str_with_missing_path, ChoiceOption, ComponentStyle, DataBinding, StyleColor,
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
    let content = resolve_dynamic_string(
        &node.component,
        prop_keys::TEXT,
        "[Text]",
        renderer,
        surface_id,
    );
    let style = ComponentStyle::from_component(&node.component);
    let mut label = text(content).shaping(Shaping::Advanced);
    // 规范 variant=caption：小号弱化（显式 style 声明优先）
    if node.component.prop_str(prop_keys::VARIANT) == Some("caption") {
        if style.font_size.is_none() {
            label = label.size(12);
        }
        if style.color.is_none() {
            label = label.color(iced::Color::from_rgb(0.5, 0.5, 0.5));
        }
    }
    apply_text_style(label, &style).into()
}

fn build_divider() -> iced::Element<'static, Message> {
    iced::widget::horizontal_rule(1).into()
}

fn build_icon(
    node: &ComponentTreeNode,
    renderer: &IcedRenderer,
    surface_id: &str,
) -> iced::Element<'static, Message> {
    let name = resolve_dynamic_string(&node.component, prop_keys::NAME, "?", renderer, surface_id);
    let style = ComponentStyle::from_component(&node.component);
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
    let label =
        resolve_dynamic_string(&node.component, prop_keys::URL, ctype, renderer, surface_id);
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
    let style = ComponentStyle::from_component(&node.component);
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
    let style = ComponentStyle::from_component(&node.component);
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
        let style = ComponentStyle::from_component(&node.component);
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
    let style = ComponentStyle::from_component(&node.component);
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

    let mut button = iced::widget::button(text(label).shaping(Shaping::Advanced))
        .on_press(Message::UserAction(UserAction::Click { component_id: id }));
    // 规范 variant 三态：primary 主色调用、borderless 无边框；default 保持
    match node.component.prop_str(prop_keys::VARIANT) {
        Some("primary") => button = button.style(iced::widget::button::primary),
        Some("borderless") => button = button.style(iced::widget::button::text),
        _ => {}
    }
    button.into()
}

fn build_text_field(
    node: &ComponentTreeNode,
    renderer: &IcedRenderer,
    surface_id: &str,
) -> iced::Element<'static, Message> {
    let id = node.component.id().clone();
    let id_str = id.as_str().to_string();
    let placeholder = resolve_dynamic_string(
        &node.component,
        prop_keys::PLACEHOLDER,
        "",
        renderer,
        surface_id,
    );
    let initial_value =
        resolve_dynamic_string(&node.component, prop_keys::VALUE, "", renderer, surface_id);
    let is_secure = matches!(
        node.component.prop_str(prop_keys::VARIANT),
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
    let id = node.component.id().clone();
    let id_str = id.as_str().to_string();
    let label = resolve_dynamic_string(&node.component, prop_keys::LABEL, "", renderer, surface_id);
    let checked = renderer
        .checkbox_values
        .borrow()
        .get(&id_str)
        .copied()
        // 本地受控缓存优先（平台专属，§3.6 豁免）；未命中时的声明状态
        // 解析走公共 helper（value 优先、checked 回退）
        .unwrap_or_else(|| checkbox_checked(&node.component, binding_for(renderer, surface_id)));

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
    let id = node.component.id().clone();
    let id_str = id.as_str().to_string();
    let initial_value =
        resolve_dynamic_number(&node.component, prop_keys::VALUE, 0.0, renderer, surface_id);
    let value = renderer
        .slider_values
        .borrow()
        .get(&id_str)
        .copied()
        .unwrap_or(initial_value);
    let min = node.component.prop_f64(prop_keys::MIN).unwrap_or(0.0);
    let max = node.component.prop_f64(prop_keys::MAX).unwrap_or(100.0);

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
    let url = resolve_dynamic_string(&node.component, prop_keys::URL, "", renderer, surface_id);
    let style = ComponentStyle::from_component(&node.component);
    if url.is_empty() {
        return apply_text_style(text("[Image: no URL]").shaping(Shaping::Advanced), &style).into();
    }

    if let Some(handle) = renderer.load_image_handle(&url) {
        let props = node.component.properties();
        let width = extract_length_prop(props, prop_keys::WIDTH, iced::Length::Shrink);
        let height = extract_length_prop(props, prop_keys::HEIGHT, iced::Length::Shrink);
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
    let target = node.component.prop_str(prop_key)?.to_string();
    node.children
        .iter()
        .find(|c| c.component.id().as_str() == target)
}

/// 按 tabs\[index\].child 的组件 id 在 node.children 中查找对应子节点。
fn select_tab_child(node: &ComponentTreeNode, index: usize) -> Option<&ComponentTreeNode> {
    let tabs = node.component.tabs_decl()?;
    let target = tabs.into_iter().nth(index)?.child;
    node.children.iter().find(|c| c.component.id() == &target)
}

fn build_tabs(
    node: &ComponentTreeNode,
    renderer: &IcedRenderer,
    surface_id: &str,
) -> iced::Element<'static, Message> {
    // 用简单的按钮行 + 激活（第 0 个）tab 内容近似实现
    let tab_labels: Vec<String> = node
        .component
        .tabs_decl()
        .map(|tabs| tabs.into_iter().map(|tab| tab.title).collect())
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
    let binding = binding_for(renderer, surface_id);
    let options = choice_options(&node.component, binding);
    let selected = choice_selected(&node.component, binding);

    let label = resolve_dynamic_string(
        &node.component,
        prop_keys::LABEL,
        "选择:",
        renderer,
        surface_id,
    );
    let variant = node
        .component
        .prop_str(prop_keys::VARIANT)
        .map(String::from);
    let component_id = node.component.id().clone();

    let option_rows: Vec<iced::Element<'static, Message>> =
        choice_display_lines(&options, &selected)
            .into_iter()
            .zip(options.iter())
            .map(|(line, opt)| {
                // 点击经 toggle_choice 计算完整新选中集（单选替换/多选切换）
                let next = a2ui_renderer::toggle_choice(&selected, &opt.value, variant.as_deref());
                iced::widget::button(text(line).shaping(Shaping::Advanced))
                    .style(iced::widget::button::text)
                    .on_press(Message::UserAction(UserAction::ChoiceSelect {
                        component_id: component_id.clone(),
                        values: next,
                    }))
                    .into()
            })
            .collect();

    iced::widget::column(vec![
        text(label).size(16).shaping(Shaping::Advanced).into(),
        iced::widget::column(option_rows).spacing(4).into(),
    ])
    .spacing(4)
    .into()
}

/// 选项展示行：选中匹配按选项稳定值，展示用 label
fn choice_display_lines(options: &[ChoiceOption], selected: &[String]) -> Vec<String> {
    options
        .iter()
        .map(|opt| {
            let marker = if selected.contains(&opt.value) {
                "●"
            } else {
                " "
            };
            format!("({}) {}", marker, opt.label)
        })
        .collect()
}

fn build_datetime_input(
    node: &ComponentTreeNode,
    renderer: &IcedRenderer,
    surface_id: &str,
) -> iced::Element<'static, Message> {
    let label = resolve_dynamic_string(
        &node.component,
        prop_keys::LABEL,
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
    component: &Component,
    key: &str,
    default: &str,
    renderer: &IcedRenderer,
    surface_id: &str,
) -> String {
    let cache_key = DynamicStringCacheKey {
        surface_id: surface_id.to_string(),
        component_id: component.id().as_str().to_string(),
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

    let resolved = match component.prop_dynamic_value(key) {
        Some(dv) => resolve_str_with_missing_path(&dv, binding_for(renderer, surface_id), |path| {
            format!("{{{}…}}", path)
        }),
        None => default.to_string(),
    };
    renderer
        .dynamic_string_cache
        .borrow_mut()
        .insert(cache_key, resolved.clone());
    renderer.record_dynamic_string_cache_miss();
    resolved
}

// CheckBox 已迁移到公共 `checkbox_checked`，暂无生产消费者（按禁删约定保留）
#[allow(dead_code)]
fn resolve_dynamic_bool(
    component: &Component,
    key: &str,
    default: bool,
    renderer: &IcedRenderer,
    surface_id: &str,
) -> bool {
    component
        .prop_dynamic_bool(key)
        .and_then(|dv| resolve_bool(&dv, binding_for(renderer, surface_id)))
        .unwrap_or(default)
}

fn resolve_dynamic_number(
    component: &Component,
    key: &str,
    default: f64,
    renderer: &IcedRenderer,
    surface_id: &str,
) -> f64 {
    component
        .prop_dynamic_f64(key)
        .and_then(|dv| resolve_f64(&dv, binding_for(renderer, surface_id)))
        .unwrap_or(default)
}

fn extract_length_prop(props: &a2ui_core::Value, key: &str, default: iced::Length) -> iced::Length {
    match props.get(key) {
        Some(a2ui_core::Value::Number(n)) => n
            .as_f64()
            .map(|value| iced::Length::Fixed(value as f32))
            .unwrap_or(default),
        Some(a2ui_core::Value::String(s)) if s == "fill" => iced::Length::Fill,
        Some(a2ui_core::Value::String(s)) if s == "shrink" => iced::Length::Shrink,
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
    if node.component.properties().get(prop_keys::TEXT).is_some() {
        let label =
            resolve_dynamic_string(&node.component, prop_keys::TEXT, "", renderer, surface_id);
        if !label.is_empty() {
            return label;
        }
    }

    // 回退到 child Text 组件
    for child in &node.children {
        if child.component.component_type() == "Text" {
            let child_text =
                resolve_dynamic_string(&child.component, prop_keys::TEXT, "", renderer, surface_id);
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
    use a2ui_core::prelude::json;
    use a2ui_core::ComponentId;

    fn tree_node(json: a2ui_core::Value) -> ComponentTreeNode {
        ComponentTreeNode::new(Component::from_value(json).unwrap())
    }

    /// 经核心创建带数据模型的 surface（pub 字段已收敛为 core 访问器，
    /// 测试不再直接拼装 data_bindings）；带一个占位根组件以满足
    /// 核心 createSurface 流水线（空 forest 无法展开模板）
    fn renderer_with_binding(surface_id: &str, data_model: a2ui_core::Value) -> IcedRenderer {
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
    fn test_choice_display_lines_mark_selected_by_value() {
        // 选中匹配按选项稳定值，展示用 label（iced 补选中态渲染）
        let options = vec![
            a2ui_renderer::ChoiceOption {
                label: "Email".into(),
                value: "email".into(),
            },
            a2ui_renderer::ChoiceOption {
                label: "SMS".into(),
                value: "sms".into(),
            },
        ];
        assert_eq!(
            choice_display_lines(&options, &["email".to_string()]),
            vec!["(●) Email".to_string(), "( ) SMS".to_string()]
        );
    }

    #[test]
    fn test_build_choice_picker_accepts_spec_object_options() {
        // 规范形态冒烟：对象 options + 绑定 value 构树不 panic
        let renderer = renderer_with_binding("s1", json!({"contact":{"preference":["email"]}}));
        let node = tree_node(json!({
            "component":"ChoicePicker","id":"cp",
            "options":[{"label":"Email","value":"email"},{"label":"SMS","value":"sms"}],
            "value":{"path":"/contact/preference"}
        }));
        let _ = build_element_tree(&node, &renderer, "s1");
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

        let comp: Component = Component::from_value(json!({
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

        let title = Component::from_value(json!({
            "component": "Text", "id": "title", "text": {"path": "/title"}
        }))
        .unwrap();
        assert_eq!(
            resolve_dynamic_string(&title, "text", "[Text]", &renderer, surface_id),
            "Welcome"
        );

        let icon = Component::from_value(json!({
            "component": "Icon", "id": "icon", "name": {"path": "/icon"}
        }))
        .unwrap();
        assert_eq!(
            resolve_dynamic_string(&icon, "name", "?", &renderer, surface_id),
            "star"
        );

        let checkbox = Component::from_value(json!({
            "component": "CheckBox", "id": "remember", "label": {"path": "/remember"}
        }))
        .unwrap();
        assert_eq!(
            resolve_dynamic_string(&checkbox, "label", "", &renderer, surface_id),
            "记住密码"
        );

        let text_field = Component::from_value(json!({
            "component": "TextField", "id": "field",
            "value": {"path": "/form/value"},
            "placeholder": {"path": "/form/placeholder"}
        }))
        .unwrap();
        assert_eq!(
            resolve_dynamic_string(&text_field, "value", "", &renderer, surface_id),
            "Alice"
        );
        assert_eq!(
            resolve_dynamic_string(&text_field, "placeholder", "", &renderer, surface_id),
            "请输入用户名"
        );

        let missing = Component::from_value(json!({
            "component": "Text", "id": "missing", "text": {"path": "/missing"}
        }))
        .unwrap();
        assert_eq!(
            resolve_dynamic_string(&missing, "text", "", &renderer, surface_id),
            "{/missing…}"
        );
    }

    #[test]
    fn test_dynamic_string_cache_profiles_hits_and_misses() {
        let surface_id = "s1";
        let renderer = renderer_with_binding(surface_id, json!({"title": "Welcome"}));
        let component = Component::from_value(json!({
            "component": "Text", "id": "title", "text": {"path": "/title"}
        }))
        .unwrap();

        assert_eq!(
            resolve_dynamic_string(&component, "text", "", &renderer, surface_id),
            "Welcome"
        );
        assert_eq!(
            resolve_dynamic_string(&component, "text", "", &renderer, surface_id),
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

        let checkbox = Component::from_value(json!({
            "component": "CheckBox", "id": "cb", "value": {"path": "/remember"}
        }))
        .unwrap();
        assert!(resolve_dynamic_bool(
            &checkbox, "value", false, &renderer, surface_id
        ));

        let slider = Component::from_value(json!({
            "component": "Slider", "id": "sl", "value": {"path": "/volume"}
        }))
        .unwrap();
        assert_eq!(
            resolve_dynamic_number(&slider, "value", 0.0, &renderer, surface_id),
            75.0
        );
    }

    #[test]
    fn test_resolve_button_label_from_dynamic_child_text() {
        let surface_id = "s1";
        let renderer = renderer_with_binding(surface_id, json!({"child_label": "Child Label"}));

        let child = ComponentTreeNode::new(
            Component::from_value(json!({
                "component": "Text",
                "id": "label",
                "text": {"path": "/child_label"}
            }))
            .unwrap(),
        );
        let button = ComponentTreeNode::new(
            Component::from_value(json!({
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

        let username: Component = Component::from_value(json!({
            "component": "TextField",
            "id": "username_field",
            "value": {"path": "/username"},
            "placeholder": {"path": "/username_placeholder"},
            "variant": "shortText"
        }))
        .unwrap();
        let password: Component = Component::from_value(json!({
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

        let username: Component = Component::from_value(json!({
            "component": "TextField",
            "id": "username_field",
            "value": {"path": "/username"},
            "placeholder": {"path": "/username_placeholder"},
            "variant": "shortText"
        }))
        .unwrap();
        let password: Component = Component::from_value(json!({
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
            Component::from_value(json!({
                "component": "Text",
                "id": "styled_text",
                "text": "Styled",
                "style": style.clone()
            }))
            .unwrap(),
        );
        let icon_node = ComponentTreeNode::new(
            Component::from_value(json!({
                "component": "Icon",
                "id": "styled_icon",
                "name": "star",
                "style": style.clone()
            }))
            .unwrap(),
        );
        let row_node = ComponentTreeNode::new(
            Component::from_value(json!({
                "component": "Row",
                "id": "styled_row",
                "children": ["styled_text", "styled_icon"],
                "style": style.clone()
            }))
            .unwrap(),
        )
        .with_children(vec![text_node, icon_node]);
        let image_node = ComponentTreeNode::new(
            Component::from_value(json!({
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
            Component::from_value(json!({
                "component": "Card",
                "id": "styled_card",
                "child": "styled_row",
                "style": style.clone()
            }))
            .unwrap(),
        )
        .with_children(vec![row_node]);
        let column_node = ComponentTreeNode::new(
            Component::from_value(json!({
                "component": "Column",
                "id": "styled_column",
                "children": ["styled_card", "styled_image"],
                "style": style.clone()
            }))
            .unwrap(),
        )
        .with_children(vec![card_node, image_node]);
        let list_node = ComponentTreeNode::new(
            Component::from_value(json!({
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

        let username: Component = Component::from_value(json!({
            "component": "TextField",
            "id": "username_field",
            "value": "",
            "placeholder": "请输入用户名",
            "variant": "shortText"
        }))
        .unwrap();
        let password: Component = Component::from_value(json!({
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

        let checkbox: Component = Component::from_value(json!({
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

        let slider: Component = Component::from_value(json!({
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
            (
                "Text(caption)",
                json!({"component":"Text","id":"tc","text":"note","variant":"caption"}),
            ),
            ("Button", json!({"component":"Button","id":"b","child":"t"})),
            (
                "Button(primary)",
                json!({"component":"Button","id":"bp","child":"t","variant":"primary"}),
            ),
            (
                "Button(borderless)",
                json!({"component":"Button","id":"bb","child":"t","variant":"borderless"}),
            ),
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
            let comp: Component = Component::from_value(json_val.clone()).unwrap();
            let node = ComponentTreeNode::new(comp);
            // 不应 panic
            let _el = build_element_tree(&node, &renderer, surface_id);
        }
    }
}
