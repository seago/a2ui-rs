use a2ui_core::prelude::*;
use a2ui_renderer::CustomComponentRegistry;
use std::collections::HashMap;

/// GUI 渲染目标 widget（用于 egui 即时模式渲染）
#[derive(Debug, Clone)]
pub enum RenderableGuiWidget {
    Text {
        id: ComponentId,
        text: String,
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
    },
    Row {
        id: ComponentId,
        children_ids: Vec<ComponentId>,
    },
    Image {
        id: ComponentId,
        url: String,
        width: WidgetLength,
        height: WidgetLength,
    },
    Card {
        id: ComponentId,
        child_id: ComponentId,
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
    },
    List {
        id: ComponentId,
        children_ids: Vec<ComponentId>,
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
        let props = component.properties();
        if let Some(text_val) = props.get("text") {
            if let Some(s) = text_val.as_str() {
                return s.to_string();
            }
            if let Some(obj) = text_val.as_object() {
                // 路径绑定：从 DataModel 中解析实际值
                if let Some(path_val) = obj.get("path") {
                    if let Some(p) = path_val.as_str() {
                        if let Some(binding) = data_model {
                            if let Some(resolved) = binding.get(p) {
                                if !resolved.is_null() {
                                    return match resolved {
                                        serde_json::Value::String(s) => s.clone(),
                                        other => other.to_string(),
                                    };
                                }
                            }
                        }
                        // 数据模型不可用时回退显示路径
                        return format!("{{{}…}}", p);
                    }
                }
                if let Some(call_val) = obj.get("call") {
                    if let Some(c) = call_val.as_str() {
                        return format!("{{call:{}}}", c);
                    }
                }
            }
        }
        format!("[{}]", component.component_type())
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
                }
            }
            "Button" => {
                let label = self.extract_text(component, data_model);
                let child_id = props
                    .get("child")
                    .and_then(|v| v.as_str())
                    .and_then(|s| ComponentId::new(s).ok())
                    .unwrap_or_else(|| ComponentId::new("__missing__").unwrap());
                let variant = props
                    .get("variant")
                    .and_then(|v| v.as_str())
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
                let children_ids = extract_children_ids(props);
                RenderableGuiWidget::Column {
                    id: component.id().clone(),
                    children_ids,
                }
            }
            "Row" => {
                let children_ids = extract_children_ids(props);
                RenderableGuiWidget::Row {
                    id: component.id().clone(),
                    children_ids,
                }
            }
            "Image" => {
                let url = props
                    .get("url")
                    .and_then(|v| resolve_dynamic_attr(v, data_model))
                    .unwrap_or_else(|| "{path:url}".to_string());
                let width = extract_length_prop(props, "width", WidgetLength::Shrink);
                let height = extract_length_prop(props, "height", WidgetLength::Shrink);
                RenderableGuiWidget::Image {
                    id: component.id().clone(),
                    url,
                    width,
                    height,
                }
            }
            "Card" => {
                let child_id = props
                    .get("child")
                    .and_then(|v| v.as_str())
                    .and_then(|s| ComponentId::new(s).ok())
                    .unwrap_or_else(|| ComponentId::new("__missing__").unwrap());
                RenderableGuiWidget::Card {
                    id: component.id().clone(),
                    child_id,
                }
            }
            "CheckBox" => {
                let checked = props
                    .get("value")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                // Also check "checked" property (TUI uses "checked")
                let checked = if !checked {
                    props
                        .get("checked")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                } else {
                    true
                };
                let label = props
                    .get("label")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
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
                let name = props
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("\u{2753}")
                    .to_string();
                RenderableGuiWidget::Icon {
                    id: component.id().clone(),
                    name,
                }
            }
            "List" => {
                let children_ids = extract_children_ids(props);
                RenderableGuiWidget::List {
                    id: component.id().clone(),
                    children_ids,
                }
            }
            "Tabs" => {
                let tabs_data = props
                    .get("tabs")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|tab| {
                                let title = tab.get("title")?.as_str()?.to_string();
                                let child = tab.get("child")?.as_str()?.to_string();
                                Some((title, child))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                RenderableGuiWidget::Tabs {
                    id: component.id().clone(),
                    tabs_data,
                }
            }
            "Modal" => {
                let content_id = props
                    .get("content")
                    .and_then(|v| v.as_str())
                    .and_then(|s| ComponentId::new(s).ok())
                    .unwrap_or_else(|| ComponentId::new("__missing__").unwrap());
                let trigger_id = props
                    .get("trigger")
                    .and_then(|v| v.as_str())
                    .and_then(|s| ComponentId::new(s).ok())
                    .unwrap_or_else(|| ComponentId::new("__missing__").unwrap());
                RenderableGuiWidget::Modal {
                    id: component.id().clone(),
                    content_id,
                    trigger_id,
                }
            }
            "Slider" => {
                let value = props.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let min = props.get("min").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let max = props.get("max").and_then(|v| v.as_f64()).unwrap_or(100.0);
                RenderableGuiWidget::Slider {
                    id: component.id().clone(),
                    value,
                    min,
                    max,
                }
            }
            "TextField" => {
                let value = props
                    .get("value")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let placeholder = props
                    .get("placeholder")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Enter text...")
                    .to_string();
                let variant = props
                    .get("variant")
                    .and_then(|v| v.as_str())
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
                let options = props
                    .get("options")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|o| o.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                let selected = props
                    .get("value")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|s| s.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                RenderableGuiWidget::ChoicePicker {
                    id: component.id().clone(),
                    options,
                    selected,
                }
            }
            "DateTimeInput" => {
                let label = props
                    .get("label")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Select date/time")
                    .to_string();
                RenderableGuiWidget::DateTimeInput {
                    id: component.id().clone(),
                    label,
                }
            }
            "Video" => {
                let url = props
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                RenderableGuiWidget::Video {
                    id: component.id().clone(),
                    url,
                }
            }
            "AudioPlayer" => {
                let url = props
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
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
    pub fn render_gui_widget(
        &self,
        widget: &RenderableGuiWidget,
        ui: &mut egui::Ui,
        widget_map: &HashMap<String, RenderableGuiWidget>,
        response_tracker: &mut HashMap<String, egui::Response>,
        user_events: &mut Vec<a2ui_renderer::UserEvent>,
        image_textures: &HashMap<String, (egui::TextureId, [usize; 2])>,
    ) {
        match widget {
            RenderableGuiWidget::Text { id, text } => {
                let label = egui::Label::new(egui::RichText::new(text.clone())).wrap(true);
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
                        response.rect.expand(2.0), 2.0,
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(25, 118, 210)),
                    );
                }
                response_tracker.insert(id.as_str().to_string(), response);
            }
            RenderableGuiWidget::Column { children_ids, .. } => {
                let width = ui.available_width();
                let height = ui.available_height();
                ui.allocate_ui_with_layout(
                    egui::vec2(width, height),
                    egui::Layout::top_down_justified(egui::Align::Center),
                    |ui| {
                        ui.set_width(width);
                        for child_id in children_ids {
                            if let Some(child) = widget_map.get(child_id.as_str()) {
                                self.render_gui_widget(
                                    child,
                                    ui,
                                    widget_map,
                                    response_tracker,
                                    user_events,
                                    image_textures,
                                );
                            }
                        }
                    },
                );
            }
            RenderableGuiWidget::Row { children_ids, .. } => {
                ui.horizontal(|ui| {
                    for child_id in children_ids {
                        if let Some(child) = widget_map.get(child_id.as_str()) {
                            self.render_gui_widget(
                                child,
                                ui,
                                widget_map,
                                response_tracker,
                                user_events,
                                image_textures,
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
                    let img =
                        egui::Image::from_texture(egui::load::SizedTexture::new(*tex_id, original))
                            .fit_to_exact_size(egui::vec2(target_width, target_height))
                            .maintain_aspect_ratio(false);
                    let response = ui.add(img);
                    response_tracker.insert(id.as_str().to_string(), response);
                } else {
                    let response = ui.label("🖼 Loading...");
                    response_tracker.insert(id.as_str().to_string(), response);
                }
            }
            RenderableGuiWidget::Card { child_id, .. } => {
                let width = ui.available_width();
                ui.set_width(width);
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.set_width(width);
                    if let Some(child) = widget_map.get(child_id.as_str()) {
                        self.render_gui_widget(
                            child,
                            ui,
                            widget_map,
                            response_tracker,
                            user_events,
                            image_textures,
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
            RenderableGuiWidget::Icon { id, name } => {
                let label = egui::Label::new(egui::RichText::new(name.clone()).size(24.0));
                let response = ui.add(label);
                response_tracker.insert(id.as_str().to_string(), response);
            }
            RenderableGuiWidget::List { children_ids, .. } => {
                let viewport_height = ui.available_height().max(0.0);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .min_scrolled_height(viewport_height)
                    .show(ui, |ui| {
                        let list_width = ui.available_width();
                        ui.allocate_ui_with_layout(
                            egui::vec2(list_width, 0.0),
                            egui::Layout::top_down_justified(egui::Align::Center),
                            |ui| {
                                ui.set_width(list_width);
                                for (index, child_id) in children_ids.iter().enumerate() {
                                    if index > 0 {
                                        ui.add_space(12.0);
                                    }

                                    if let Some(child) = widget_map.get(child_id.as_str()) {
                                        self.render_gui_widget(
                                            child,
                                            ui,
                                            widget_map,
                                            response_tracker,
                                            user_events,
                                            image_textures,
                                        );
                                    } else {
                                        ui.label(format!("[missing: {}]", child_id));
                                    }
                                }
                            },
                        );
                    });
            }
            RenderableGuiWidget::Tabs {
                id, tabs_data, ..
            } => {
                if tabs_data.is_empty() {
                    return;
                }
                // 用 egui memory 存储活动 tab 索引
                let state_id = ui.make_persistent_id(format!("tabs_{}", id.as_str()));
                let mut active = ui
                    .memory_mut(|mem| *mem.data.get_temp_mut_or::<usize>(state_id, 0));
                if active >= tabs_data.len() {
                    active = 0;
                }

                ui.horizontal(|ui| {
                    for (i, (title, _)) in tabs_data.iter().enumerate() {
                        let btn = egui::Button::new(
                            if i == active {
                                egui::RichText::new(title.clone()).strong()
                            } else {
                                egui::RichText::new(title.clone())
                            },
                        );
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
                            child, ui, widget_map, response_tracker,
                            user_events, image_textures,
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
                let mut open = ui
                    .memory_mut(|mem| *mem.data.get_temp_mut_or::<bool>(modal_id, false));

                // 渲染 trigger
                if let Some(trigger) = widget_map.get(trigger_id.as_str()) {
                    self.render_gui_widget(
                        trigger, ui, widget_map, response_tracker,
                        user_events, image_textures,
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
                                    content, ui, widget_map, response_tracker,
                                    user_events, image_textures,
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
fn extract_children_ids(props: &serde_json::Value) -> Vec<ComponentId> {
    props
        .get("children")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|id_val| id_val.as_str())
                .filter_map(|id_str| ComponentId::new(id_str).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// 解析动态属性值（字面量字符串或 { "path": "..." } 绑定）
fn resolve_dynamic_attr(
    v: &serde_json::Value,
    data_model: Option<&a2ui_renderer::DataBinding>,
) -> Option<String> {
    match v {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Object(obj) => {
            if let Some(p) = obj.get("path").and_then(|v| v.as_str()) {
                if let Some(binding) = data_model {
                    if let Some(resolved) = binding.get(p) {
                        if !resolved.is_null() {
                            return match resolved {
                                serde_json::Value::String(s) => Some(s.clone()),
                                other => Some(other.to_string()),
                            };
                        }
                    }
                }
                Some(format!("{{{}…}}", p))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn extract_length_prop(
    props: &serde_json::Value,
    key: &str,
    default: WidgetLength,
) -> WidgetLength {
    match props.get(key) {
        Some(serde_json::Value::Number(n)) => n
            .as_f64()
            .map(|value| WidgetLength::Fixed(value as f32))
            .unwrap_or(default),
        Some(serde_json::Value::String(s)) if s == "fill" => WidgetLength::Fill,
        Some(serde_json::Value::String(s)) if s == "shrink" => WidgetLength::Shrink,
        _ => default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn empty_registry() -> CustomComponentRegistry {
        CustomComponentRegistry::new()
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
    fn test_map_button_to_gui_widget() {
        let mapper = WidgetMapper;
        let comp: Component = serde_json::from_str(
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
            serde_json::from_str(r#"{"id":"col1","component":"Column","children":["c1","c2"]}"#)
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
            serde_json::from_str(r#"{"id":"row1","component":"Row","children":["c1"]}"#).unwrap();
        let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
        assert!(matches!(widget, RenderableGuiWidget::Row { .. }));
    }

    #[test]
    fn test_map_image_to_gui_widget() {
        let mapper = WidgetMapper;
        let comp: Component = serde_json::from_str(
            r#"{"id":"img1","component":"Image","url":"https://example.com/img.png"}"#,
        )
        .unwrap();
        let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
        assert!(
            matches!(widget, RenderableGuiWidget::Image { ref url, .. } if url == "https://example.com/img.png")
        );
    }

    #[test]
    fn test_map_card_to_gui_widget() {
        let mapper = WidgetMapper;
        let comp: Component =
            serde_json::from_str(r#"{"id":"card1","component":"Card","child":"inner1"}"#).unwrap();
        let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
        assert!(matches!(widget, RenderableGuiWidget::Card { .. }));
    }

    #[test]
    fn test_map_checkbox_to_gui_widget() {
        let mapper = WidgetMapper;
        let comp: Component = serde_json::from_str(
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
            serde_json::from_str(r#"{"id":"div1","component":"Divider"}"#).unwrap();
        let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
        assert!(matches!(widget, RenderableGuiWidget::Divider { .. }));
    }

    #[test]
    fn test_map_icon_to_gui_widget() {
        let mapper = WidgetMapper;
        let comp: Component =
            serde_json::from_str(r#"{"id":"icon1","component":"Icon","name":"star"}"#).unwrap();
        let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
        assert!(matches!(widget, RenderableGuiWidget::Icon { ref name, .. } if name == "star"));
    }

    #[test]
    fn test_map_list_to_gui_widget() {
        let mapper = WidgetMapper;
        let comp: Component = serde_json::from_str(
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
        let comp: Component = serde_json::from_str(
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
        let comp: Component = serde_json::from_str(
            r#"{"id":"modal1","component":"Modal","content":"content1","trigger":"btn1"}"#,
        )
        .unwrap();
        let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
        assert!(matches!(widget, RenderableGuiWidget::Modal { .. }));
    }

    #[test]
    fn test_map_slider_to_gui_widget() {
        let mapper = WidgetMapper;
        let comp: Component = serde_json::from_str(
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
        let comp: Component = serde_json::from_str(
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
        let comp: Component = serde_json::from_str(
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
            serde_json::from_str(r#"{"id":"dt1","component":"DateTimeInput","label":"Pick date"}"#)
                .unwrap();
        let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
        assert!(
            matches!(widget, RenderableGuiWidget::DateTimeInput { ref label, .. } if label == "Pick date")
        );
    }

    #[test]
    fn test_map_video_to_gui_widget() {
        let mapper = WidgetMapper;
        let comp: Component = serde_json::from_str(
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
        let comp: Component = serde_json::from_str(
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
            serde_json::from_str(r#"{"id":"unk1","component":"UnknownType"}"#).unwrap();
        let widget = mapper.map_to_gui_widget(&comp, &empty_registry(), None);
        assert!(
            matches!(widget, RenderableGuiWidget::Placeholder { ref reason, .. } if reason.contains("unknown"))
        );
    }

    #[test]
    fn test_all_18_types_map_without_panic() {
        let mapper = WidgetMapper;
        let test_cases: Vec<(&str, serde_json::Value)> = vec![
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
            let comp: Component = serde_json::from_value(json_val.clone()).unwrap_or_else(|_| {
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
