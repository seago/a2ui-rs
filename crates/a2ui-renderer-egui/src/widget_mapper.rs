use a2ui_core::component::{prop_keys, ChildrenDecl};
use a2ui_core::prelude::*;
use a2ui_renderer::{
    resolve_bool, resolve_f64, resolve_str_with_missing_path, value_to_display_string,
    ComponentStyle, CustomComponentRegistry, StyleColor, StyleSpacing,
};
use std::collections::HashMap;

/// GUI 渲染目标 widget（用于 egui 即时模式渲染）
#[derive(Debug, Clone)]
pub enum RenderableGuiWidget {
    Text {
        id: ComponentId,
        text: String,
        style: ComponentStyle,
    },
    Button {
        id: ComponentId,
        label: String,
        child_id: ComponentId,
        variant: String,
    },
    Column {
        id: ComponentId,
        children_ids: Vec<ComponentId>,
        style: ComponentStyle,
    },
    Row {
        id: ComponentId,
        children_ids: Vec<ComponentId>,
        style: ComponentStyle,
    },
    Image {
        id: ComponentId,
        url: String,
        width: WidgetLength,
        height: WidgetLength,
        style: ComponentStyle,
    },
    Card {
        id: ComponentId,
        child_id: ComponentId,
        style: ComponentStyle,
    },
    CheckBox {
        id: ComponentId,
        checked: bool,
        label: String,
    },
    Divider {
        id: ComponentId,
    },
    Icon {
        id: ComponentId,
        name: String,
        style: ComponentStyle,
    },
    List {
        id: ComponentId,
        children_ids: Vec<ComponentId>,
        style: ComponentStyle,
    },
    Tabs {
        id: ComponentId,
        tabs_data: Vec<(String, String)>,
    },
    Modal {
        id: ComponentId,
        content_id: ComponentId,
        trigger_id: ComponentId,
    },
    Slider {
        id: ComponentId,
        value: f64,
        min: f64,
        max: f64,
    },
    TextField {
        id: ComponentId,
        value: String,
        placeholder: String,
        variant: String,
    },
    ChoicePicker {
        id: ComponentId,
        options: Vec<String>,
        selected: Vec<String>,
    },
    DateTimeInput {
        id: ComponentId,
        label: String,
    },
    Video {
        id: ComponentId,
        url: String,
    },
    AudioPlayer {
        id: ComponentId,
        url: String,
    },
    Placeholder {
        id: ComponentId,
        reason: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WidgetLength {
    Shrink,
    Fill,
    Fixed(f32),
}

/// Widget Mapper：将 A2UI 组件映射为 egui UI 指令
pub struct WidgetMapper;

impl WidgetMapper {
    /// 从组件属性中提取文本内容，可选地解析 DataBinding 路径
    /// `data_model` 为 `Some` 时，`{ "path": "/..." }` 会从数据模型中解析实际值
    pub fn extract_text(
        &self,
        component: &Component,
        data_model: Option<&a2ui_renderer::DataBinding>,
    ) -> String {
        resolve_dynamic_prop(
            component,
            prop_keys::TEXT,
            data_model,
            &format!("[{}]", component.component_type()),
        )
    }

    /// 判断组件是否可聚焦
    pub fn is_focusable(&self, component: &Component) -> bool {
        matches!(
            component.component_type(),
            "Button" | "TextField" | "CheckBox" | "ChoicePicker" | "Slider"
        )
    }

    /// 将 Component 映射为 RenderableGuiWidget
    ///
    /// `registry` 用于识别 Basic Catalog 以外的自定义组件类型。
    /// `data_model` 用于解析 `{ "path": "/..." }` 数据绑定。
    pub fn map_to_gui_widget(
        &self,
        component: &Component,
        registry: &CustomComponentRegistry,
        data_model: Option<&a2ui_renderer::DataBinding>,
    ) -> RenderableGuiWidget {
        let ctype = component.component_type();
        let props = component.properties();

        match ctype {
            "Text" => {
                let text = self.extract_text(component, data_model);
                RenderableGuiWidget::Text {
                    id: component.id().clone(),
                    text,
                    style: ComponentStyle::from_component(component),
                }
            }
            "Button" => {
                let label = self.extract_text(component, data_model);
                let child_id = component
                    .prop_component_id(prop_keys::CHILD)
                    .unwrap_or_else(|| ComponentId::new("__missing__").unwrap());
                let variant = component
                    .prop_str(prop_keys::VARIANT)
                    .unwrap_or("default")
                    .to_string();
                RenderableGuiWidget::Button {
                    id: component.id().clone(),
                    label,
                    child_id,
                    variant,
                }
            }
            "Column" => {
                let children_ids = extract_children_ids(component);
                RenderableGuiWidget::Column {
                    id: component.id().clone(),
                    children_ids,
                    style: ComponentStyle::from_component(component),
                }
            }
            "Row" => {
                let children_ids = extract_children_ids(component);
                RenderableGuiWidget::Row {
                    id: component.id().clone(),
                    children_ids,
                    style: ComponentStyle::from_component(component),
                }
            }
            "Image" => {
                let url = props
                    .get(prop_keys::URL)
                    .and_then(|v| resolve_dynamic_attr(v, data_model))
                    .unwrap_or_else(|| "{path:url}".to_string());
                let width = extract_length_prop(props, prop_keys::WIDTH, WidgetLength::Shrink);
                let height = extract_length_prop(props, prop_keys::HEIGHT, WidgetLength::Shrink);
                RenderableGuiWidget::Image {
                    id: component.id().clone(),
                    url,
                    width,
                    height,
                    style: ComponentStyle::from_component(component),
                }
            }
            "Card" => {
                let child_id = component
                    .prop_component_id(prop_keys::CHILD)
                    .unwrap_or_else(|| ComponentId::new("__missing__").unwrap());
                RenderableGuiWidget::Card {
                    id: component.id().clone(),
                    child_id,
                    style: ComponentStyle::from_component(component),
                }
            }
            "CheckBox" => {
                let checked = resolve_dynamic_bool(component, prop_keys::VALUE, data_model)
                    .or_else(|| resolve_dynamic_bool(component, prop_keys::CHECKED, data_model))
                    .unwrap_or(false);
                let label = resolve_dynamic_prop(component, prop_keys::LABEL, data_model, "");
                RenderableGuiWidget::CheckBox {
                    id: component.id().clone(),
                    checked,
                    label,
                }
            }
            "Divider" => RenderableGuiWidget::Divider {
                id: component.id().clone(),
            },
            "Icon" => {
                let name = resolve_dynamic_prop(component, prop_keys::NAME, data_model, "\u{2753}");
                RenderableGuiWidget::Icon {
                    id: component.id().clone(),
                    name,
                    style: ComponentStyle::from_component(component),
                }
            }
            "List" => {
                let children_ids = extract_children_ids(component);
                RenderableGuiWidget::List {
                    id: component.id().clone(),
                    children_ids,
                    style: ComponentStyle::from_component(component),
                }
            }
            "Tabs" => {
                let tabs_data = component
                    .tabs_decl()
                    .map(|tabs| {
                        tabs.into_iter()
                            .map(|tab| (tab.title, tab.child.as_str().to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                RenderableGuiWidget::Tabs {
                    id: component.id().clone(),
                    tabs_data,
                }
            }
            "Modal" => {
                let content_id = component
                    .prop_component_id(prop_keys::CONTENT)
                    .unwrap_or_else(|| ComponentId::new("__missing__").unwrap());
                let trigger_id = component
                    .prop_component_id(prop_keys::TRIGGER)
                    .unwrap_or_else(|| ComponentId::new("__missing__").unwrap());
                RenderableGuiWidget::Modal {
                    id: component.id().clone(),
                    content_id,
                    trigger_id,
                }
            }
            "Slider" => {
                let value =
                    resolve_dynamic_number(component, prop_keys::VALUE, data_model).unwrap_or(0.0);
                let min =
                    resolve_dynamic_number(component, prop_keys::MIN, data_model).unwrap_or(0.0);
                let max =
                    resolve_dynamic_number(component, prop_keys::MAX, data_model).unwrap_or(100.0);
                RenderableGuiWidget::Slider {
                    id: component.id().clone(),
                    value,
                    min,
                    max,
                }
            }
            "TextField" => {
                let value = resolve_dynamic_prop(component, prop_keys::VALUE, data_model, "");
                let placeholder = resolve_dynamic_prop(
                    component,
                    prop_keys::PLACEHOLDER,
                    data_model,
                    "Enter text...",
                );
                let variant = component
                    .prop_str(prop_keys::VARIANT)
                    .unwrap_or("shortText")
                    .to_string();
                RenderableGuiWidget::TextField {
                    id: component.id().clone(),
                    value,
                    placeholder,
                    variant,
                }
            }
            "ChoicePicker" => {
                let options = component
                    .prop_str_list(prop_keys::OPTIONS)
                    .map(|list| list.into_iter().map(String::from).collect())
                    .unwrap_or_default();
                let selected = component
                    .prop_str_list(prop_keys::VALUE)
                    .map(|list| list.into_iter().map(String::from).collect())
                    .unwrap_or_default();
                RenderableGuiWidget::ChoicePicker {
                    id: component.id().clone(),
                    options,
                    selected,
                }
            }
            "DateTimeInput" => {
                let label = resolve_dynamic_prop(
                    component,
                    prop_keys::LABEL,
                    data_model,
                    "Select date/time",
                );
                RenderableGuiWidget::DateTimeInput {
                    id: component.id().clone(),
                    label,
                }
            }
            "Video" => {
                let url = resolve_dynamic_prop(component, prop_keys::URL, data_model, "");
                RenderableGuiWidget::Video {
                    id: component.id().clone(),
                    url,
                }
            }
            "AudioPlayer" => {
                let url = resolve_dynamic_prop(component, prop_keys::URL, data_model, "");
                RenderableGuiWidget::AudioPlayer {
                    id: component.id().clone(),
                    url,
                }
            }
            _ => {
                // 先检查自定义组件注册表
                if registry.is_registered(ctype) {
                    RenderableGuiWidget::Placeholder {
                        id: component.id().clone(),
                        reason: format!("custom component: {}", ctype),
                    }
                } else {
                    RenderableGuiWidget::Placeholder {
                        id: component.id().clone(),
                        reason: format!("unknown component type: {}", ctype),
                    }
                }
            }
        }
    }

    /// 将 RenderableGuiWidget 渲染到 egui::Ui
    /// user_events: 收集渲染过程中产生的用户交互事件
    ///
    /// depth: 递归深度（入口传 0）。本函数按 id 从 widget_map 间接递归，
    /// Modal content/trigger 等边可绕过 build_tree 的环检测，须自带
    /// 深度防护（纵深防御）：超过 MAX_TREE_DEPTH 时渲染占位并终止。
    #[allow(clippy::too_many_arguments)]
    pub fn render_gui_widget(
        &self,
        widget: &RenderableGuiWidget,
        ui: &mut egui::Ui,
        widget_map: &HashMap<String, RenderableGuiWidget>,
        response_tracker: &mut HashMap<String, egui::Response>,
        user_events: &mut Vec<a2ui_renderer::UserEvent>,
        image_textures: &HashMap<String, (egui::TextureId, [usize; 2])>,
        depth: usize,
    ) {
        if depth >= a2ui_renderer::error::MAX_TREE_DEPTH {
            ui.label("[depth limit reached]");
            return;
        }
        match widget {
            RenderableGuiWidget::Text { id, text, style } => {
                let rich_text = apply_text_style(egui::RichText::new(text.clone()), style);
                let label = egui::Label::new(rich_text).wrap(true);
                let response = ui.add(label);
                response_tracker.insert(id.as_str().to_string(), response);
            }
            RenderableGuiWidget::Button {
                id, label, variant, ..
            } => {
                let rich_label = if variant == "primary" {
                    egui::RichText::new(label.clone()).color(egui::Color32::WHITE)
                } else {
                    egui::RichText::new(label.clone())
                };
                let mut button = egui::Button::new(rich_label);
                if variant == "primary" {
                    button = button.fill(egui::Color32::from_rgb(25, 118, 210));
                }
                let response = ui.add(button);
                if response.clicked() {
                    user_events.push(a2ui_renderer::UserEvent::Click {
                        component_id: id.clone(),
                    });
                }
                if response.has_focus() {
                    ui.painter().rect_stroke(
                        response.rect.expand(2.0),
                        2.0,
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(25, 118, 210)),
                    );
                }
                response_tracker.insert(id.as_str().to_string(), response);
            }
            RenderableGuiWidget::Column {
                children_ids,
                style,
                ..
            } => {
                let width = ui.available_width();
                let contains_expanding_child = children_ids.iter().any(|child_id| {
                    matches!(
                        widget_map.get(child_id.as_str()),
                        Some(RenderableGuiWidget::List { .. })
                    )
                });

                if contains_expanding_child {
                    ui.allocate_ui_with_layout(
                        egui::vec2(width, ui.available_height()),
                        egui::Layout::top_down_justified(egui::Align::Center),
                        |ui| {
                            ui.set_width(width);
                            if let Some(spacing) = style.spacing {
                                ui.spacing_mut().item_spacing = to_egui_spacing(spacing);
                            }
                            for child_id in children_ids {
                                if let Some(child) = widget_map.get(child_id.as_str()) {
                                    self.render_gui_widget(
                                        child,
                                        ui,
                                        widget_map,
                                        response_tracker,
                                        user_events,
                                        image_textures,
                                        depth + 1,
                                    );
                                }
                            }
                        },
                    );
                } else {
                    ui.set_width(width);
                    if let Some(spacing) = style.spacing {
                        ui.spacing_mut().item_spacing = to_egui_spacing(spacing);
                    }
                    for child_id in children_ids {
                        if let Some(child) = widget_map.get(child_id.as_str()) {
                            self.render_gui_widget(
                                child,
                                ui,
                                widget_map,
                                response_tracker,
                                user_events,
                                image_textures,
                                depth + 1,
                            );
                        }
                    }
                }
            }
            RenderableGuiWidget::Row {
                children_ids,
                style,
                ..
            } => {
                ui.horizontal(|ui| {
                    if let Some(spacing) = style.spacing {
                        ui.spacing_mut().item_spacing = to_egui_spacing(spacing);
                    }
                    for child_id in children_ids {
                        if let Some(child) = widget_map.get(child_id.as_str()) {
                            self.render_gui_widget(
                                child,
                                ui,
                                widget_map,
                                response_tracker,
                                user_events,
                                image_textures,
                                depth + 1,
                            );
                        }
                    }
                });
            }
            RenderableGuiWidget::Image {
                id,
                url: _,
                width,
                height,
                style,
            } => {
                if let Some((tex_id, size)) = image_textures.get(id.as_str()) {
                    let original = egui::vec2(size[0] as f32, size[1] as f32);
                    let target_width = match width {
                        WidgetLength::Fill => ui.available_width().max(1.0),
                        WidgetLength::Fixed(value) => *value,
                        WidgetLength::Shrink => original.x.min(ui.available_width().max(1.0)),
                    };
                    let target_height = match height {
                        WidgetLength::Fixed(value) => *value,
                        WidgetLength::Fill => ui.available_height().max(1.0),
                        WidgetLength::Shrink => {
                            if original.x > 0.0 {
                                original.y * (target_width / original.x)
                            } else {
                                original.y
                            }
                        }
                    };
                    let target_size = egui::vec2(target_width, target_height);
                    let (rect, response) =
                        ui.allocate_exact_size(target_size, egui::Sense::hover());
                    if ui.is_rect_visible(rect) {
                        let uv = cover_uv(original, target_size);
                        let mut image = egui::Image::from_texture(egui::load::SizedTexture::new(
                            *tex_id, original,
                        ))
                        .uv(uv);
                        if let Some(radius) = style.radius {
                            image = image.rounding(radius);
                        }
                        image.paint_at(ui, rect);
                    }
                    response_tracker.insert(id.as_str().to_string(), response);
                } else {
                    let response = ui.label("🖼 Loading...");
                    response_tracker.insert(id.as_str().to_string(), response);
                }
            }
            RenderableGuiWidget::Card {
                child_id, style, ..
            } => {
                let width = ui.available_width();
                ui.set_width(width);
                let mut frame = egui::Frame::none();
                if let Some(fill) = style.fill {
                    frame = frame.fill(to_egui_color(fill));
                }
                if let Some(padding) = style.padding {
                    frame = frame.inner_margin(egui::Margin::same(padding));
                }
                if let Some(radius) = style.radius {
                    frame = frame.rounding(radius);
                }
                frame.show(ui, |ui| {
                    let inner_width = ui.available_width();
                    ui.set_width(inner_width);
                    if let Some(spacing) = style.spacing {
                        ui.spacing_mut().item_spacing = to_egui_spacing(spacing);
                    }
                    if let Some(child) = widget_map.get(child_id.as_str()) {
                        self.render_gui_widget(
                            child,
                            ui,
                            widget_map,
                            response_tracker,
                            user_events,
                            image_textures,
                            depth + 1,
                        );
                    } else {
                        ui.label(format!("[missing: {}]", child_id));
                    }
                });
            }
            RenderableGuiWidget::CheckBox { id, checked, label } => {
                let state_id = egui::Id::new(("cb", id.as_str()));
                let mut is_checked = ui
                    .memory_mut(|mem| mem.data.get_temp::<bool>(state_id))
                    .unwrap_or(*checked);
                let response = ui.checkbox(&mut is_checked, label.clone());
                if response.changed() {
                    ui.memory_mut(|mem| mem.data.insert_temp::<bool>(state_id, is_checked));
                    user_events.push(a2ui_renderer::UserEvent::CheckToggle {
                        component_id: id.clone(),
                        checked: is_checked,
                    });
                }
                response_tracker.insert(id.as_str().to_string(), response);
            }
            RenderableGuiWidget::Divider { .. } => {
                ui.separator();
            }
            RenderableGuiWidget::Icon { id, name, style } => {
                let mut rich_text = egui::RichText::new(name.clone());
                if style.font_size.is_none() {
                    rich_text = rich_text.size(24.0);
                }
                let rich_text = apply_text_style(rich_text, style);
                let label = egui::Label::new(rich_text);
                let response = ui.add(label);
                response_tracker.insert(id.as_str().to_string(), response);
            }
            RenderableGuiWidget::List {
                id,
                children_ids,
                style,
                ..
            } => {
                let viewport_rect = ui.available_rect_before_wrap();
                ui.allocate_ui_at_rect(viewport_rect, |ui| {
                    ui.set_min_size(viewport_rect.size());
                    ui.set_width(viewport_rect.width());
                    egui::ScrollArea::vertical()
                        .id_source(id.as_str())
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            if let Some(spacing) = style.spacing {
                                ui.spacing_mut().item_spacing = to_egui_spacing(spacing);
                            }
                            ui.add_space(8.0);
                            for (index, child_id) in children_ids.iter().enumerate() {
                                if index > 0 {
                                    ui.add_space(style.spacing.map(|s| s.y).unwrap_or(16.0));
                                }
                                if let Some(child) = widget_map.get(child_id.as_str()) {
                                    self.render_gui_widget(
                                        child,
                                        ui,
                                        widget_map,
                                        response_tracker,
                                        user_events,
                                        image_textures,
                                        depth + 1,
                                    );
                                } else {
                                    ui.label(format!("[missing: {}]", child_id));
                                }
                            }
                            ui.add_space(8.0);
                        });
                });
            }
            RenderableGuiWidget::Tabs { id, tabs_data, .. } => {
                if tabs_data.is_empty() {
                    return;
                }
                // 用 egui memory 存储活动 tab 索引
                let state_id = ui.make_persistent_id(format!("tabs_{}", id.as_str()));
                let mut active =
                    ui.memory_mut(|mem| *mem.data.get_temp_mut_or::<usize>(state_id, 0));
                if active >= tabs_data.len() {
                    active = 0;
                }

                ui.horizontal(|ui| {
                    for (i, (title, _)) in tabs_data.iter().enumerate() {
                        let btn = egui::Button::new(if i == active {
                            egui::RichText::new(title.clone()).strong()
                        } else {
                            egui::RichText::new(title.clone())
                        });
                        if ui.add(btn).clicked() {
                            active = i;
                            ui.memory_mut(|mem| mem.data.insert_temp::<usize>(state_id, active));
                        }
                    }
                });
                ui.separator();

                // 渲染活动 tab 的子组件
                if let Some((_, child_id)) = tabs_data.get(active) {
                    if let Some(child) = widget_map.get(child_id.as_str()) {
                        self.render_gui_widget(
                            child,
                            ui,
                            widget_map,
                            response_tracker,
                            user_events,
                            image_textures,
                            depth + 1,
                        );
                    }
                }
            }
            RenderableGuiWidget::Modal {
                id,
                content_id,
                trigger_id,
                ..
            } => {
                let modal_id = ui.make_persistent_id(format!("modal_{}", id.as_str()));
                let mut open =
                    ui.memory_mut(|mem| *mem.data.get_temp_mut_or::<bool>(modal_id, false));

                // 渲染 trigger
                if let Some(trigger) = widget_map.get(trigger_id.as_str()) {
                    self.render_gui_widget(
                        trigger,
                        ui,
                        widget_map,
                        response_tracker,
                        user_events,
                        image_textures,
                        depth + 1,
                    );
                    // 检查 trigger 是否被点击
                    if let Some(resp) = response_tracker.get(trigger_id.as_str()) {
                        if resp.clicked() {
                            open = !open;
                            ui.memory_mut(|mem| mem.data.insert_temp::<bool>(modal_id, open));
                        }
                    }
                }

                // 渲染 modal 内容
                if open {
                    egui::Window::new(format!("Modal"))
                        .open(&mut open)
                        .show(ui.ctx(), |ui| {
                            if let Some(content) = widget_map.get(content_id.as_str()) {
                                self.render_gui_widget(
                                    content,
                                    ui,
                                    widget_map,
                                    response_tracker,
                                    user_events,
                                    image_textures,
                                    depth + 1,
                                );
                            }
                        });
                    if !open {
                        ui.memory_mut(|mem| mem.data.insert_temp::<bool>(modal_id, false));
                    }
                }
            }
            RenderableGuiWidget::Slider {
                id,
                value,
                min,
                max,
            } => {
                let state_id = egui::Id::new(("slider", id.as_str()));
                let mut val = ui
                    .memory_mut(|mem| mem.data.get_temp::<f64>(state_id))
                    .unwrap_or(*value);
                let response = ui.add(egui::Slider::new(&mut val, *min..=*max));
                if response.changed() {
                    ui.memory_mut(|mem| mem.data.insert_temp::<f64>(state_id, val));
                    user_events.push(a2ui_renderer::UserEvent::SliderChange {
                        component_id: id.clone(),
                        value: val,
                    });
                }
                response_tracker.insert(id.as_str().to_string(), response);
            }
            RenderableGuiWidget::TextField {
                id,
                value,
                placeholder,
                variant,
            } => {
                let state_id = egui::Id::new(("tf", id.as_str()));
                let mut text = ui
                    .memory_mut(|mem| mem.data.get_temp::<String>(state_id))
                    .unwrap_or_else(|| value.clone());
                let is_password = variant == "obscured";
                let text_edit = if is_password {
                    egui::TextEdit::singleline(&mut text).password(true)
                } else {
                    egui::TextEdit::singleline(&mut text)
                };
                let response = ui.add(text_edit.hint_text(placeholder.clone()));
                if response.changed() {
                    ui.memory_mut(|mem| mem.data.insert_temp::<String>(state_id, text.clone()));
                    user_events.push(a2ui_renderer::UserEvent::TextInput {
                        component_id: id.clone(),
                        value: text,
                    });
                }
                response_tracker.insert(id.as_str().to_string(), response);
            }
            RenderableGuiWidget::ChoicePicker {
                options, selected, ..
            } => {
                ui.horizontal(|ui| {
                    for opt in options {
                        let is_sel = selected.contains(opt);
                        ui.label(format!("[{}] {}", if is_sel { "x" } else { " " }, opt));
                    }
                });
            }
            RenderableGuiWidget::DateTimeInput { id, label } => {
                let response = ui.button(format!("\u{1F4C5} {}", label));
                response_tracker.insert(id.as_str().to_string(), response);
            }
            RenderableGuiWidget::Video { id, url } => {
                let label = egui::Label::new(
                    egui::RichText::new(format!("\u{1F3AC} Video: {}", url))
                        .color(egui::Color32::GRAY),
                );
                let response = ui.add(label);
                response_tracker.insert(id.as_str().to_string(), response);
            }
            RenderableGuiWidget::AudioPlayer { id, url } => {
                let label = egui::Label::new(
                    egui::RichText::new(format!("\u{1F50A} Audio: {}", url))
                        .color(egui::Color32::GRAY),
                );
                let response = ui.add(label);
                response_tracker.insert(id.as_str().to_string(), response);
            }
            RenderableGuiWidget::Placeholder { id, reason } => {
                let label = egui::Label::new(
                    egui::RichText::new(format!("[{}]", reason)).color(egui::Color32::RED),
                );
                let response = ui.add(label);
                response_tracker.insert(id.as_str().to_string(), response);
            }
        }
    }
}

/// 从组件属性中提取 children 的 ComponentId 列表
fn extract_children_ids(component: &Component) -> Vec<ComponentId> {
    match component.children_decl() {
        Some(ChildrenDecl::Ids(ids)) => ids,
        // 模板形态在核心层展开为数组；其余形态无直接子引用（对齐旧行为）
        _ => Vec::new(),
    }
}

fn resolve_dynamic_prop(
    component: &Component,
    key: &str,
    data_model: Option<&a2ui_renderer::DataBinding>,
    fallback: &str,
) -> String {
    match component.prop_dynamic_value(key) {
        Some(dv) => resolve_str_with_missing_path(&dv, data_model, |path| format!("{{{}…}}", path)),
        None => fallback.to_string(),
    }
}

fn resolve_dynamic_bool(
    component: &Component,
    key: &str,
    data_model: Option<&a2ui_renderer::DataBinding>,
) -> Option<bool> {
    component
        .prop_dynamic_bool(key)
        .and_then(|dv| resolve_bool(&dv, data_model))
}

fn resolve_dynamic_number(
    component: &Component,
    key: &str,
    data_model: Option<&a2ui_renderer::DataBinding>,
) -> Option<f64> {
    component
        .prop_dynamic_f64(key)
        .and_then(|dv| resolve_f64(&dv, data_model))
}

/// 解析动态属性值（字面量字符串或 { "path": "..." } 绑定）
///
/// 与通用字符串解析不同：非字符串字面量与 `{"call":...}` 包装返回
/// `None`（调用方自备默认值），此语义仅 Image url 使用，原样保留。
fn resolve_dynamic_attr(
    v: &a2ui_core::Value,
    data_model: Option<&a2ui_renderer::DataBinding>,
) -> Option<String> {
    match v {
        a2ui_core::Value::String(s) => Some(s.clone()),
        a2ui_core::Value::Object(obj) if obj.contains_key("path") => {
            match obj.get("path").and_then(|p| p.as_str()) {
                Some(path) => match data_model.and_then(|binding| binding.get(path)) {
                    Some(resolved) => Some(value_to_display_string(resolved)),
                    None => Some(format!("{{{}…}}", path)),
                },
                // path 非字符串：对齐旧实现兜底为整个对象的显示文本
                None => match obj.get("call").and_then(|c| c.as_str()) {
                    Some(call) => Some(format!("{{call:{}}}", call)),
                    None => Some(value_to_display_string(v)),
                },
            }
        }
        _ => None,
    }
}

fn extract_length_prop(props: &a2ui_core::Value, key: &str, default: WidgetLength) -> WidgetLength {
    match props.get(key) {
        Some(a2ui_core::Value::Number(n)) => n
            .as_f64()
            .map(|value| WidgetLength::Fixed(value as f32))
            .unwrap_or(default),
        Some(a2ui_core::Value::String(s)) if s == "fill" => WidgetLength::Fill,
        Some(a2ui_core::Value::String(s)) if s == "shrink" => WidgetLength::Shrink,
        _ => default,
    }
}

fn apply_text_style(mut text: egui::RichText, style: &ComponentStyle) -> egui::RichText {
    if let Some(size) = style.font_size {
        text = text.size(size);
    }
    if style.strong {
        text = text.strong();
    }
    if let Some(color) = style.color {
        text = text.color(to_egui_color(color));
    }
    text
}

fn to_egui_color(color: StyleColor) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}

fn to_egui_spacing(spacing: StyleSpacing) -> egui::Vec2 {
    egui::vec2(spacing.x, spacing.y)
}

fn cover_uv(image_size: egui::Vec2, target_size: egui::Vec2) -> egui::Rect {
    if image_size.x <= 0.0 || image_size.y <= 0.0 || target_size.x <= 0.0 || target_size.y <= 0.0 {
        return egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0));
    }

    let image_aspect = image_size.x / image_size.y;
    let target_aspect = target_size.x / target_size.y;

    if image_aspect > target_aspect {
        let visible_width = target_aspect / image_aspect;
        let inset = (1.0 - visible_width) / 2.0;
        egui::Rect::from_min_max(egui::pos2(inset, 0.0), egui::pos2(1.0 - inset, 1.0))
    } else {
        let visible_height = image_aspect / target_aspect;
        let inset = (1.0 - visible_height) / 2.0;
        egui::Rect::from_min_max(egui::pos2(0.0, inset), egui::pos2(1.0, 1.0 - inset))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use a2ui_core::prelude::json;

    /// 构造 chain 层 Card 链（card0 → card1 → ... → leaf），渲染后返回
    /// response_tracker 是否含链尾叶子 "leaf"（Card 不进 tracker，Text 叶子会）
    fn render_card_chain_reaches_leaf(chain: usize) -> bool {
        let mapper = WidgetMapper;
        let mut widget_map: HashMap<String, RenderableGuiWidget> = HashMap::new();
        for i in 0..chain {
            let child = if i + 1 == chain {
                "leaf".to_string()
            } else {
                format!("card{}", i + 1)
            };
            widget_map.insert(
                format!("card{i}"),
                RenderableGuiWidget::Card {
                    id: ComponentId::new(format!("card{i}")).unwrap(),
                    child_id: ComponentId::new(child).unwrap(),
                    style: ComponentStyle::default(),
                },
            );
        }
        widget_map.insert(
            "leaf".to_string(),
            RenderableGuiWidget::Text {
                id: ComponentId::new("leaf").unwrap(),
                text: "leaf".into(),
                style: ComponentStyle::default(),
            },
        );

        let ctx = egui::Context::default();
        let mut response_tracker = HashMap::new();
        let mut user_events = Vec::new();
        let image_textures = HashMap::new();
        let root = widget_map.get("card0").unwrap().clone();

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                mapper.render_gui_widget(
                    &root,
                    ui,
                    &widget_map,
                    &mut response_tracker,
                    &mut user_events,
                    &image_textures,
                    0,
                );
            });
        });
        response_tracker.contains_key("leaf")
    }

    #[test]
    fn test_render_gui_widget_depth_limit_renders_placeholder_not_overflow() {
        // render_gui_widget 按 id 间接递归、可绕过 build_tree 的深度检查，
        // 必须自带深度防护：上限内正常渲染，超限终止而非栈溢出。
        assert!(
            render_card_chain_reaches_leaf(30),
            "深度上限内的叶子应正常渲染"
        );
        assert!(
            !render_card_chain_reaches_leaf(60),
            "超过深度上限（50）的叶子不应被渲染"
        );
    }

    #[test]
    fn test_render_gui_widget_self_reference_terminates() {
        // 自引用 Card（widget_map 数据成环）：必须有限终止而非栈溢出
        let mapper = WidgetMapper;
        let mut widget_map: HashMap<String, RenderableGuiWidget> = HashMap::new();
        widget_map.insert(
            "loop_card".to_string(),
            RenderableGuiWidget::Card {
                id: ComponentId::new("loop_card").unwrap(),
                child_id: ComponentId::new("loop_card").unwrap(),
                style: ComponentStyle::default(),
            },
        );

        let ctx = egui::Context::default();
        let mut response_tracker = HashMap::new();
        let mut user_events = Vec::new();
        let image_textures = HashMap::new();
        let root = widget_map.get("loop_card").unwrap().clone();

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                mapper.render_gui_widget(
                    &root,
                    ui,
                    &widget_map,
                    &mut response_tracker,
                    &mut user_events,
                    &image_textures,
                    0,
                );
            });
        });
        // 走到这里即证明递归有限终止
    }

    fn empty_registry() -> CustomComponentRegistry {
        CustomComponentRegistry::new()
    }

    fn style_contract_props(component: &str) -> a2ui_core::Value {
        json!({
            "id": "styled",
            "component": component,
            "text": "Styled",
            "name": "star",
            "url": "https://example.com/image.png",
            "child": "child",
            "children": ["child"],
            "style": {
                "fontSize": 18,
                "strong": true,
                "color": "#112233",
                "fill": "#44556680",
                "padding": 9,
                "spacing": {"x": 7, "y": 11},
                "radius": 5
            }
        })
    }

    fn expected_contract_style() -> ComponentStyle {
        ComponentStyle {
            font_size: Some(18.0),
            strong: true,
            color: Some(StyleColor {
                r: 17,
                g: 34,
                b: 51,
                a: 255,
            }),
            fill: Some(StyleColor {
                r: 68,
                g: 85,
                b: 102,
                a: 128,
            }),
            padding: Some(9.0),
            spacing: Some(StyleSpacing { x: 7.0, y: 11.0 }),
            radius: Some(5.0),
        }
    }

    #[test]
    fn test_supported_style_components_keep_shared_contract_style() {
        let mapper = WidgetMapper;
        let expected = expected_contract_style();

        for component_type in ["Text", "Icon", "Row", "Column", "List", "Card", "Image"] {
            let comp: Component =
                Component::from_value(style_contract_props(component_type)).unwrap();
            let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
            let actual = match widget {
                RenderableGuiWidget::Text { style, .. }
                | RenderableGuiWidget::Icon { style, .. }
                | RenderableGuiWidget::Row { style, .. }
                | RenderableGuiWidget::Column { style, .. }
                | RenderableGuiWidget::List { style, .. }
                | RenderableGuiWidget::Card { style, .. }
                | RenderableGuiWidget::Image { style, .. } => style,
                other => panic!("{component_type} mapped to unexpected widget: {other:?}"),
            };

            assert_eq!(
                actual, expected,
                "{component_type} should keep shared style"
            );
        }
    }

    #[test]
    fn test_extract_text_literal() {
        let mapper = WidgetMapper;
        let comp = Component::text(
            ComponentId::new("t").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        assert_eq!(mapper.extract_text(&comp, None), "Hello");
    }

    #[test]
    fn test_dynamic_string_props_resolve_with_egui_missing_path_format() {
        let mapper = WidgetMapper;
        let binding = a2ui_renderer::DataBinding::new(DataModel::new(json!({
            "title": "Welcome",
            "remember": "记住密码",
            "rememberChecked": true,
            "form": {
                "value": "Alice",
                "placeholder": "请输入用户名",
                "volume": 42.0
            }
        })));

        let text: Component = Component::from_value(json!({
            "id": "title",
            "component": "Text",
            "text": {"path": "/title"}
        }))
        .unwrap();
        assert_eq!(mapper.extract_text(&text, Some(&binding)), "Welcome");

        let checkbox: Component = Component::from_value(json!({
            "id": "remember",
            "component": "CheckBox",
            "label": {"path": "/remember"},
            "value": {"path": "/rememberChecked"}
        }))
        .unwrap();
        let widget = mapper.map_to_gui_widget(&checkbox, &empty_registry(), Some(&binding));
        assert!(
            matches!(widget, RenderableGuiWidget::CheckBox { checked: true, ref label, .. } if label == "记住密码")
        );

        let text_field: Component = Component::from_value(json!({
            "id": "username",
            "component": "TextField",
            "value": {"path": "/form/value"},
            "placeholder": {"path": "/form/placeholder"}
        }))
        .unwrap();
        let widget = mapper.map_to_gui_widget(&text_field, &empty_registry(), Some(&binding));
        assert!(matches!(
            widget,
            RenderableGuiWidget::TextField {
                ref value,
                ref placeholder,
                ..
            } if value == "Alice" && placeholder == "请输入用户名"
        ));

        let slider: Component = Component::from_value(json!({
            "id": "volume",
            "component": "Slider",
            "value": {"path": "/form/volume"},
            "min": 0,
            "max": 100
        }))
        .unwrap();
        let widget = mapper.map_to_gui_widget(&slider, &empty_registry(), Some(&binding));
        assert!(matches!(
            widget,
            RenderableGuiWidget::Slider {
                value: 42.0,
                min: 0.0,
                max: 100.0,
                ..
            }
        ));

        let missing: Component = Component::from_value(json!({
            "id": "missing",
            "component": "Text",
            "text": {"path": "/missing"}
        }))
        .unwrap();
        assert_eq!(mapper.extract_text(&missing, Some(&binding)), "{/missing…}");
    }

    #[test]
    fn test_dynamic_image_url_keeps_existing_fallback_behavior() {
        let mapper = WidgetMapper;
        let binding = a2ui_renderer::DataBinding::new(DataModel::new(
            json!({"image": "https://example.com/a.png"}),
        ));

        let image: Component = Component::from_value(json!({
            "id": "image",
            "component": "Image",
            "url": {"path": "/image"}
        }))
        .unwrap();
        let widget = mapper.map_to_gui_widget(&image, &empty_registry(), Some(&binding));
        assert!(
            matches!(widget, RenderableGuiWidget::Image { ref url, .. } if url == "https://example.com/a.png")
        );

        let unsupported: Component = Component::from_value(json!({
            "id": "image",
            "component": "Image",
            "url": {"call": "imageUrl"}
        }))
        .unwrap();
        let widget = mapper.map_to_gui_widget(&unsupported, &empty_registry(), Some(&binding));
        assert!(
            matches!(widget, RenderableGuiWidget::Image { ref url, .. } if url == "{path:url}")
        );
    }

    #[test]
    fn test_button_is_focusable() {
        let mapper = WidgetMapper;
        let comp = Component::button(
            ComponentId::new("btn").unwrap(),
            ComponentId::new("lbl").unwrap(),
        );
        assert!(mapper.is_focusable(&comp));
    }

    #[test]
    fn test_text_is_not_focusable() {
        let mapper = WidgetMapper;
        let comp = Component::text(
            ComponentId::new("t").unwrap(),
            DynamicValue::Literal("hi".to_string()),
        );
        assert!(!mapper.is_focusable(&comp));
    }

    // ===== 18 个组件类型的 widget 映射测试 =====

    #[test]
    fn test_map_text_to_gui_widget() {
        let mapper = WidgetMapper;
        let comp = Component::text(
            ComponentId::new("t1").unwrap(),
            DynamicValue::Literal("Hello World".to_string()),
        );
        let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
        assert!(
            matches!(widget, RenderableGuiWidget::Text { ref text, .. } if text == "Hello World")
        );
    }

    #[test]
    fn test_map_text_style_to_gui_widget() {
        let mapper = WidgetMapper;
        let comp: Component = Component::from_value(json!({
            "id": "title",
            "component": "Text",
            "text": "Styled",
            "style": {
                "fontSize": 22,
                "strong": true,
                "color": "#1976d2"
            }
        }))
        .unwrap();
        let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
        assert!(matches!(
            widget,
            RenderableGuiWidget::Text {
                style: ComponentStyle {
                    font_size: Some(22.0),
                    strong: true,
                    color: Some(color),
                    ..
                },
                ..
            } if color == (StyleColor { r: 25, g: 118, b: 210, a: 255 })
        ));
    }

    #[test]
    fn test_map_button_to_gui_widget() {
        let mapper = WidgetMapper;
        let comp: Component = Component::from_json(
            r#"{"id":"btn1","component":"Button","child":"label1","text":"Click Me"}"#,
        )
        .unwrap();
        let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
        assert!(matches!(widget, RenderableGuiWidget::Button { .. }));
    }

    #[test]
    fn test_map_column_to_gui_widget() {
        let mapper = WidgetMapper;
        let comp: Component =
            Component::from_json(r#"{"id":"col1","component":"Column","children":["c1","c2"]}"#)
                .unwrap();
        let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
        assert!(
            matches!(widget, RenderableGuiWidget::Column { ref children_ids, .. } if children_ids.len() == 2)
        );
    }

    #[test]
    fn test_map_row_to_gui_widget() {
        let mapper = WidgetMapper;
        let comp: Component =
            Component::from_json(r#"{"id":"row1","component":"Row","children":["c1"]}"#).unwrap();
        let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
        assert!(matches!(widget, RenderableGuiWidget::Row { .. }));
    }

    #[test]
    fn test_map_row_style_to_gui_widget() {
        let mapper = WidgetMapper;
        let comp: Component = Component::from_value(json!({
            "id": "row1",
            "component": "Row",
            "children": ["c1"],
            "style": {"spacing": {"x": 6, "y": 0}}
        }))
        .unwrap();
        let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
        assert!(matches!(
            widget,
            RenderableGuiWidget::Row {
                style: ComponentStyle {
                    spacing: Some(spacing),
                    ..
                },
                ..
            } if spacing == (StyleSpacing { x: 6.0, y: 0.0 })
        ));
    }

    #[test]
    fn test_map_image_to_gui_widget() {
        let mapper = WidgetMapper;
        let comp: Component = Component::from_json(
            r#"{"id":"img1","component":"Image","url":"https://example.com/img.png"}"#,
        )
        .unwrap();
        let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
        assert!(
            matches!(widget, RenderableGuiWidget::Image { ref url, .. } if url == "https://example.com/img.png")
        );
    }

    #[test]
    fn test_map_image_style_to_gui_widget() {
        let mapper = WidgetMapper;
        let comp: Component = Component::from_value(json!({
            "id": "img1",
            "component": "Image",
            "url": "https://example.com/img.png",
            "style": {"radius": 10}
        }))
        .unwrap();
        let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
        assert!(matches!(
            widget,
            RenderableGuiWidget::Image {
                style: ComponentStyle {
                    radius: Some(10.0),
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn test_map_card_to_gui_widget() {
        let mapper = WidgetMapper;
        let comp: Component =
            Component::from_json(r#"{"id":"card1","component":"Card","child":"inner1"}"#).unwrap();
        let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
        assert!(matches!(widget, RenderableGuiWidget::Card { .. }));
    }

    #[test]
    fn test_map_card_style_to_gui_widget() {
        let mapper = WidgetMapper;
        let comp: Component = Component::from_value(json!({
            "id": "card1",
            "component": "Card",
            "child": "inner1",
            "style": {
                "fill": "#fafafa",
                "padding": 12,
                "spacing": {"x": 0, "y": 10}
            }
        }))
        .unwrap();
        let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
        assert!(matches!(
            widget,
            RenderableGuiWidget::Card {
                style: ComponentStyle {
                    fill: Some(fill),
                    padding: Some(12.0),
                    spacing: Some(spacing),
                    ..
                },
                ..
            } if fill == (StyleColor { r: 250, g: 250, b: 250, a: 255 })
                && spacing == (StyleSpacing { x: 0.0, y: 10.0 })
        ));
    }

    #[test]
    fn test_map_checkbox_to_gui_widget() {
        let mapper = WidgetMapper;
        let comp: Component = Component::from_json(
            r#"{"id":"cb1","component":"CheckBox","checked":true,"label":"Accept"}"#,
        )
        .unwrap();
        let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
        assert!(
            matches!(widget, RenderableGuiWidget::CheckBox { checked: true, ref label, .. } if label == "Accept")
        );
    }

    #[test]
    fn test_map_divider_to_gui_widget() {
        let mapper = WidgetMapper;
        let comp: Component =
            Component::from_json(r#"{"id":"div1","component":"Divider"}"#).unwrap();
        let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
        assert!(matches!(widget, RenderableGuiWidget::Divider { .. }));
    }

    #[test]
    fn test_map_icon_to_gui_widget() {
        let mapper = WidgetMapper;
        let comp: Component =
            Component::from_json(r#"{"id":"icon1","component":"Icon","name":"star"}"#).unwrap();
        let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
        assert!(matches!(widget, RenderableGuiWidget::Icon { ref name, .. } if name == "star"));
    }

    #[test]
    fn test_map_list_to_gui_widget() {
        let mapper = WidgetMapper;
        let comp: Component = Component::from_json(
            r#"{"id":"list1","component":"List","children":["item1","item2","item3"]}"#,
        )
        .unwrap();
        let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
        assert!(
            matches!(widget, RenderableGuiWidget::List { ref children_ids, .. } if children_ids.len() == 3)
        );
    }

    #[test]
    fn test_map_tabs_to_gui_widget() {
        let mapper = WidgetMapper;
        let comp: Component = Component::from_json(
            r#"{"id":"tabs1","component":"Tabs","tabs":[{"title":"Tab A","child":"a"},{"title":"Tab B","child":"b"}]}"#
        ).unwrap();
        let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
        assert!(
            matches!(widget, RenderableGuiWidget::Tabs { ref tabs_data, .. } if tabs_data.len() == 2)
        );
    }

    #[test]
    fn test_map_modal_to_gui_widget() {
        let mapper = WidgetMapper;
        let comp: Component = Component::from_json(
            r#"{"id":"modal1","component":"Modal","content":"content1","trigger":"btn1"}"#,
        )
        .unwrap();
        let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
        assert!(matches!(widget, RenderableGuiWidget::Modal { .. }));
    }

    #[test]
    fn test_map_slider_to_gui_widget() {
        let mapper = WidgetMapper;
        let comp: Component = Component::from_json(
            r#"{"id":"sl1","component":"Slider","value":50,"min":0,"max":100}"#,
        )
        .unwrap();
        let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
        assert!(matches!(
            widget,
            RenderableGuiWidget::Slider {
                value: 50.0,
                min: 0.0,
                max: 100.0,
                ..
            }
        ));
    }

    #[test]
    fn test_map_textfield_to_gui_widget() {
        let mapper = WidgetMapper;
        let comp: Component = Component::from_json(
            r#"{"id":"tf1","component":"TextField","value":"Hello","placeholder":"Enter text"}"#,
        )
        .unwrap();
        let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
        assert!(
            matches!(widget, RenderableGuiWidget::TextField { ref value, .. } if value == "Hello")
        );
    }

    #[test]
    fn test_map_choicepicker_to_gui_widget() {
        let mapper = WidgetMapper;
        let comp: Component = Component::from_json(
            r#"{"id":"cp1","component":"ChoicePicker","options":["A","B","C"],"value":["A"]}"#,
        )
        .unwrap();
        let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
        assert!(
            matches!(widget, RenderableGuiWidget::ChoicePicker { ref options, .. } if options.len() == 3)
        );
    }

    #[test]
    fn test_map_datetimeinput_to_gui_widget() {
        let mapper = WidgetMapper;
        let comp: Component =
            Component::from_json(r#"{"id":"dt1","component":"DateTimeInput","label":"Pick date"}"#)
                .unwrap();
        let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
        assert!(
            matches!(widget, RenderableGuiWidget::DateTimeInput { ref label, .. } if label == "Pick date")
        );
    }

    #[test]
    fn test_map_video_to_gui_widget() {
        let mapper = WidgetMapper;
        let comp: Component = Component::from_json(
            r#"{"id":"vid1","component":"Video","url":"https://example.com/video.mp4"}"#,
        )
        .unwrap();
        let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
        assert!(
            matches!(widget, RenderableGuiWidget::Video { ref url, .. } if url == "https://example.com/video.mp4")
        );
    }

    #[test]
    fn test_map_audioplayer_to_gui_widget() {
        let mapper = WidgetMapper;
        let comp: Component = Component::from_json(
            r#"{"id":"aud1","component":"AudioPlayer","url":"https://example.com/audio.mp3"}"#,
        )
        .unwrap();
        let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
        assert!(
            matches!(widget, RenderableGuiWidget::AudioPlayer { ref url, .. } if url == "https://example.com/audio.mp3")
        );
    }

    #[test]
    fn test_map_unknown_component_to_placeholder() {
        let mapper = WidgetMapper;
        let comp: Component =
            Component::from_json(r#"{"id":"unk1","component":"UnknownType"}"#).unwrap();
        let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
        assert!(
            matches!(widget, RenderableGuiWidget::Placeholder { ref reason, .. } if reason.contains("unknown"))
        );
    }

    #[test]
    fn test_all_18_types_map_without_panic() {
        let mapper = WidgetMapper;
        let test_cases: Vec<(&str, a2ui_core::Value)> = vec![
            ("Text", json!({"id":"t","component":"Text","text":"Hello"})),
            (
                "Button",
                json!({"id":"b","component":"Button","child":"lbl","text":"Click"}),
            ),
            (
                "TextField",
                json!({"id":"tf","component":"TextField","value":"test"}),
            ),
            (
                "Column",
                json!({"id":"c","component":"Column","children":["a","b"]}),
            ),
            ("Row", json!({"id":"r","component":"Row","children":["x"]})),
            (
                "Image",
                json!({"id":"i","component":"Image","url":"http://example.com/pic.png"}),
            ),
            (
                "Card",
                json!({"id":"cd","component":"Card","child":"inner"}),
            ),
            (
                "CheckBox",
                json!({"id":"cb","component":"CheckBox","checked":false,"label":"Check"}),
            ),
            ("Divider", json!({"id":"d","component":"Divider"})),
            ("Icon", json!({"id":"ic","component":"Icon","name":"home"})),
            (
                "List",
                json!({"id":"l","component":"List","children":["i1","i2"]}),
            ),
            (
                "Tabs",
                json!({"id":"tb","component":"Tabs","tabs":[{"title":"A","child":"a"},{"title":"B","child":"b"}]}),
            ),
            (
                "Modal",
                json!({"id":"m","component":"Modal","content":"c","trigger":"t"}),
            ),
            (
                "Slider",
                json!({"id":"s","component":"Slider","value":30,"min":0,"max":100}),
            ),
            (
                "ChoicePicker",
                json!({"id":"cp","component":"ChoicePicker","options":["X","Y"],"value":["X"]}),
            ),
            (
                "DateTimeInput",
                json!({"id":"dt","component":"DateTimeInput","label":"Date"}),
            ),
            (
                "Video",
                json!({"id":"v","component":"Video","url":"http://example.com/v.mp4"}),
            ),
            (
                "AudioPlayer",
                json!({"id":"ap","component":"AudioPlayer","url":"http://example.com/a.mp3"}),
            ),
        ];

        for (type_name, json_val) in &test_cases {
            let comp: Component = Component::from_value(json_val.clone()).unwrap_or_else(|_| {
                panic!("Failed to deserialize component of type {}", type_name)
            });
            let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
            // 不应该 panic — 每个类型都应该成功映射（不能是 Placeholder）
            match &widget {
                RenderableGuiWidget::Placeholder { reason, .. } => {
                    panic!(
                        "Component type '{}' should not map to Placeholder: {}",
                        type_name, reason
                    );
                }
                _ => {} // 成功 — 任何具体的 widget 类型都可以
            }
        }
    }
}
