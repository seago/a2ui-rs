use crate::focus_manager::FocusManager;
use crate::{
    widget_builder::{component_style_to_tui, RenderableWidget},
    WidgetBuilder, WidgetMapper,
};
use a2ui_core::message::{
    client_to_server::FunctionResponse,
    server_to_client::{
        ActionResponse, CallFunction, CreateSurface, DeleteSurface, UpdateComponents,
        UpdateDataModel,
    },
};
use a2ui_core::prelude::*;
use a2ui_core::ClientEnvelope;
use a2ui_renderer::{
    choice_options, choice_selected, toggle_choice, RenderResult, Renderer, RendererCore,
    SurfaceHandle, UserEvent,
};
use ratatui::{
    layout::Rect,
    widgets::{Block, Paragraph},
    Frame,
};

/// TUI 渲染器实现
///
/// 协议状态与消息处理全部委托 [`RendererCore`]，本类型只保留平台特有部分：
/// 焦点管理（KeyPress 本地转译）与 ratatui 帧绘制。
#[derive(Debug)]
pub struct TuiRenderer {
    /// 渲染器公共核心（协议状态 + 消息流水线；pub 供同 crate 测试访问）
    pub core: RendererCore,
    /// 焦点管理器
    pub focus_manager: FocusManager,
    /// ChoicePicker 的选项游标（component_id → 选项下标，平台本地 UI 状态）
    pub choice_cursors: std::collections::HashMap<String, usize>,
    /// 最近一帧构建的 widget 数量（render() 填充，测试用）
    pub last_frame_widget_count: usize,
}

impl TuiRenderer {
    /// 创建新的 TUI 渲染器
    pub fn new() -> Self {
        Self {
            core: RendererCore::new(),
            focus_manager: FocusManager::new(),
            choice_cursors: std::collections::HashMap::new(),
            last_frame_widget_count: 0,
        }
    }

    /// 获取依赖图的只读引用（用于测试和查询）
    pub fn dependency_graph(&self) -> &a2ui_renderer::DependencyGraph {
        self.core.dependency_graph()
    }

    /// 焦点 ChoicePicker 的选项游标移动（焦点不是 ChoicePicker 时 no-op）
    fn move_choice_cursor(&mut self, component_id: &ComponentId, forward: bool) {
        let Some(count) = self.choice_option_count(component_id) else {
            return;
        };
        if count == 0 {
            return;
        }
        let cursor = self
            .choice_cursors
            .entry(component_id.as_str().to_string())
            .or_insert(0);
        *cursor = if forward {
            (*cursor + 1) % count
        } else {
            (*cursor + count - 1) % count
        };
    }

    fn choice_option_count(&self, component_id: &ComponentId) -> Option<usize> {
        let surface_id = self.core.forest().surface_of(component_id)?.to_string();
        let component = self.core.forest().get(&surface_id, component_id)?;
        if component.component_type() != "ChoicePicker" {
            return None;
        }
        Some(choice_options(component, self.core.binding(&surface_id)).len())
    }

    /// 焦点组件是 ChoicePicker 时构造 ChoiceSelect：游标指向的选项经
    /// toggle_choice（单选替换/多选切换）计算完整新选中集
    fn choice_select_event(&self, component_id: &ComponentId) -> Option<UserEvent> {
        let surface_id = self.core.forest().surface_of(component_id)?.to_string();
        let component = self.core.forest().get(&surface_id, component_id)?;
        if component.component_type() != "ChoicePicker" {
            return None;
        }
        let binding = self.core.binding(&surface_id);
        let options = choice_options(component, binding);
        if options.is_empty() {
            return None;
        }
        let cursor = self
            .choice_cursors
            .get(component_id.as_str())
            .copied()
            .unwrap_or(0)
            .min(options.len() - 1);
        let selected = choice_selected(component, binding);
        let variant = component.prop_str(a2ui_core::component::prop_keys::VARIANT);
        Some(UserEvent::ChoiceSelect {
            component_id: component_id.clone(),
            values: toggle_choice(&selected, &options[cursor].value, variant),
        })
    }

    /// 注册客户端函数（供 callableFrom enforcement 使用）
    pub fn register_function(
        &mut self,
        name: impl Into<String>,
        callable_from: a2ui_renderer::CallableFrom,
    ) {
        self.core.register_function(name, callable_from);
    }

    /// 获取已注册函数列表
    pub fn registered_functions(&self) -> Vec<&String> {
        self.core.registered_functions()
    }

    /// 注册 Catalog（用于 catalogId 信任链校验）
    pub fn register_catalog(&mut self, catalog: a2ui_core::Catalog) -> RenderResult<()> {
        self.core.register_catalog(catalog)
    }

    /// 获取 Catalog 注册表的只读引用
    pub fn catalog_registry(&self) -> &a2ui_renderer::CatalogRegistry {
        self.core.catalog_registry()
    }

    /// 注册自定义组件类型
    pub fn register_custom_component(
        &mut self,
        def: a2ui_renderer::CustomComponentDef,
    ) -> Result<(), String> {
        self.core.register_custom_component(def)
    }

    /// 注册待响应的 action_id → (surface_id, response_path) 映射
    /// surface_id 用于响应到达时精确定位写回目标（组件 id 只在 surface 内唯一）
    pub fn register_pending_response(
        &mut self,
        action_id: impl Into<String>,
        surface_id: impl Into<String>,
        response_path: impl Into<String>,
    ) {
        self.core
            .register_pending_response(action_id, surface_id, response_path);
    }

    /// 使用 ratatui Terminal 执行实际帧绘制
    pub async fn render_frame<B>(&mut self, terminal: &mut ratatui::Terminal<B>) -> RenderResult<()>
    where
        B: ratatui::backend::Backend,
    {
        // 帧准备：构建所有 surface 的 widget 树
        let surface_widgets = self.prepare_frame().await?;

        let widgets_to_draw: Vec<_> = surface_widgets
            .into_iter()
            .flat_map(|(_, widgets)| widgets)
            .collect();

        terminal
            .draw(|frame: &mut Frame| {
                for widget in &widgets_to_draw {
                    self.draw_widget(frame, widget.clone());
                }
            })
            .map_err(|e| {
                a2ui_renderer::error::RendererError::BindingError(format!(
                    "terminal draw error: {}",
                    e
                ))
            })?;

        self.core.clear_dirty();
        Ok(())
    }

    /// 准备帧：构建所有 surface 的 widget 树
    /// 返回 (surface_id, widgets) 列表
    async fn prepare_frame(&mut self) -> RenderResult<Vec<(String, Vec<RenderableWidget>)>> {
        let surfaces_to_render: Vec<_> = if self.core.dirty_surfaces().is_empty() {
            self.core.surfaces().values().cloned().collect()
        } else {
            self.core.dirty_surfaces().iter().cloned().collect()
        };

        let mapper = WidgetMapper;
        let mut all_widgets = Vec::new();

        for surface_id in &surfaces_to_render {
            let binding = match self.core.binding(surface_id) {
                Some(b) => b,
                None => continue,
            };

            let builder =
                WidgetBuilder::new(binding, self.core.forest(), self.core.custom_registry());
            let area = Rect::new(0, 0, 80, 24);
            let widgets = builder.build_tree(surface_id, area);
            all_widgets.push((surface_id.clone(), widgets));
        }

        let total: usize = all_widgets.iter().map(|(_, w)| w.len()).sum();
        self.last_frame_widget_count = total;

        // 收集所有可聚焦组件 ID
        let mut focusable_ids = Vec::new();
        for (surface_id, _) in &all_widgets {
            for comp in self.core.forest().components_of(surface_id) {
                if mapper.is_focusable(comp) {
                    focusable_ids.push(comp.id().clone());
                }
            }
        }
        self.focus_manager.set_focusable(focusable_ids);

        Ok(all_widgets)
    }

    /// 将单个 RenderableWidget 绘制到 Frame
    fn draw_widget(&self, frame: &mut Frame, widget: RenderableWidget) {
        // 检查此 widget 是否为当前焦点组件
        let is_focused = self
            .focus_manager
            .current()
            .map(|focused_id| focused_id == widget.id())
            .unwrap_or(false);

        match widget {
            RenderableWidget::Paragraph {
                area, text, style, ..
            } => {
                let mut style = component_style_to_tui(&style);
                if is_focused {
                    style = style.add_modifier(ratatui::style::Modifier::REVERSED);
                }
                let para = Paragraph::new(text).style(style);
                frame.render_widget(para, area);
            }
            RenderableWidget::Block { area, title, .. } => {
                let block = Block::default().title(title);
                frame.render_widget(block, area);
            }
            RenderableWidget::Placeholder { area, reason, .. } => {
                let text = Paragraph::new(format!("[{}]", reason));
                frame.render_widget(text, area);
            }
            RenderableWidget::TextField {
                area,
                value,
                placeholder,
                ..
            } => {
                let display = if value.is_empty() {
                    placeholder.as_str()
                } else {
                    value.as_str()
                };
                let text = Paragraph::new(format!("[{}]", display));
                frame.render_widget(text, area);
            }
            RenderableWidget::CheckBox {
                area,
                label,
                checked,
                ..
            } => {
                let status = if checked { "[x]" } else { "[ ]" };
                let text = Paragraph::new(format!("{} {}", status, label));
                frame.render_widget(text, area);
            }
            RenderableWidget::Slider {
                area,
                value,
                min,
                max,
                ..
            } => {
                let range = max - min;
                let ratio = if range == 0.0 {
                    0.0
                } else {
                    ((value - min) / range).clamp(0.0, 1.0)
                };
                let filled = (ratio * 20.0).round() as usize;
                let bar = format!("[{}{}]", "=".repeat(filled), " ".repeat(20 - filled));
                let text = Paragraph::new(bar);
                frame.render_widget(text, area);
            }
            RenderableWidget::Button {
                area,
                label,
                variant,
                ..
            } => {
                let display = if variant == "primary" {
                    format!("[ {} ]", label)
                } else {
                    format!("< {} >", label)
                };
                let p = Paragraph::new(display);
                frame.render_widget(p, area);
            }
            RenderableWidget::Card { area, .. } => {
                let block = Block::default().title("┌─┐");
                frame.render_widget(block, area);
            }
            RenderableWidget::Divider { area, .. } => {
                let line = "─".repeat(area.width as usize);
                frame.render_widget(Paragraph::new(line), area);
            }
            RenderableWidget::Icon {
                area,
                symbol,
                style,
                ..
            } => {
                frame.render_widget(
                    Paragraph::new(symbol).style(component_style_to_tui(&style)),
                    area,
                );
            }
            RenderableWidget::Image { area, url, .. } => {
                frame.render_widget(Paragraph::new(format!("🖼 {}", url)), area);
            }
            RenderableWidget::Tabs { area, titles, .. } => {
                let header = titles.join(" │ ");
                frame.render_widget(Paragraph::new(header), area);
            }
            RenderableWidget::ChoicePicker {
                id,
                area,
                options,
                selected,
            } => {
                // 焦点时以 ▸ 标记选项游标（键盘 Left/Right 移动、Enter 切换）
                let cursor = if is_focused && !options.is_empty() {
                    Some(
                        self.choice_cursors
                            .get(id.as_str())
                            .copied()
                            .unwrap_or(0)
                            .min(options.len() - 1),
                    )
                } else {
                    None
                };
                let display: Vec<String> = options
                    .iter()
                    .enumerate()
                    .map(|(i, o)| {
                        // 选中匹配按选项稳定值，展示用 label（两者在裸字符串
                        // 兼容形态下相同）
                        let marker = if selected.contains(&o.value) {
                            "(●)"
                        } else {
                            "( )"
                        };
                        let prefix = if cursor == Some(i) { "▸" } else { "" };
                        format!("{}{} {}", prefix, marker, o.label)
                    })
                    .collect();
                frame.render_widget(Paragraph::new(display.join("  ")), area);
            }
            RenderableWidget::Video { area, url, .. } => {
                frame.render_widget(Paragraph::new(format!("🎬 {}", url)), area);
            }
            RenderableWidget::AudioPlayer {
                area,
                url,
                description,
                ..
            } => {
                let text = if description.is_empty() {
                    format!("🔊 {}", url)
                } else {
                    format!("🔊 {} — {}", url, description)
                };
                frame.render_widget(Paragraph::new(text), area);
            }
            RenderableWidget::Modal {
                area,
                trigger_id,
                content_id,
                ..
            } => {
                frame.render_widget(
                    Paragraph::new(format!(
                        "[Modal: trigger={} content={}]",
                        trigger_id, content_id
                    )),
                    area,
                );
            }
            RenderableWidget::DateTimeInput { area, label, .. } => {
                frame.render_widget(Paragraph::new(format!("📅 {}", label)), area);
            }
        }
    }

    /// 渲染单个组件到 Frame（简化实现）
    pub fn render_component(&self, component: &Component, frame: &mut Frame, area: Rect) {
        let mapper = WidgetMapper;
        let paragraph = mapper.map_to_paragraph(component);
        frame.render_widget(paragraph, area);
    }
}

impl Default for TuiRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Renderer for TuiRenderer {
    async fn create_surface(&mut self, msg: CreateSurface) -> RenderResult<SurfaceHandle> {
        tracing::trace!(surface_id = %msg.surface_id, "createSurface");
        self.core
            .create_surface(msg)
            .await
            .map(|(handle, _)| handle)
    }

    async fn update_components(&mut self, msg: UpdateComponents) -> RenderResult<()> {
        self.core.update_components(msg).await.map(|_| ())
    }

    async fn update_data_model(&mut self, msg: UpdateDataModel) -> RenderResult<()> {
        tracing::debug!(surface_id = %msg.surface_id, path = ?msg.path, "updateDataModel");
        self.core.update_data_model(msg).await.map(|_| ())
    }

    async fn delete_surface(&mut self, msg: DeleteSurface) -> RenderResult<()> {
        tracing::info!(surface_id = %msg.surface_id, "deleteSurface");
        self.core.delete_surface(msg).await.map(|_| ())
    }

    async fn action_response(&mut self, msg: ActionResponse) -> RenderResult<()> {
        self.core.action_response(msg).await.map(|_| ())
    }

    async fn call_function(&mut self, msg: CallFunction) -> RenderResult<FunctionResponse> {
        self.core.call_function(msg).await
    }

    async fn render(&mut self) -> RenderResult<()> {
        // 帧准备：构建所有 surface 的 widget 树，验证组件引用和路径解析
        self.prepare_frame().await?;
        Ok(())
    }

    async fn handle_user_event(
        &mut self,
        event: UserEvent,
    ) -> RenderResult<Option<ClientEnvelope>> {
        // KeyPress 是渲染器本地行为（docs/refactor-step0 D7）：
        // Tab/Up/Down 导航焦点不产消息；Left/Right 移动焦点 ChoicePicker
        // 的选项游标；Enter/空格 对 ChoicePicker 产生 ChoiceSelect（经
        // toggle_choice），其余组件转译为 Click，再交公共核心
        let event = match event {
            UserEvent::KeyPress { key } => match key.as_str() {
                "Tab" | "Down" => {
                    self.focus_manager.next();
                    return Ok(None);
                }
                "Up" => {
                    self.focus_manager.previous();
                    return Ok(None);
                }
                "Left" | "Right" => {
                    if let Some(comp_id) = self.focus_manager.current().cloned() {
                        self.move_choice_cursor(&comp_id, key == "Right");
                    }
                    return Ok(None);
                }
                "Enter" | " " => match self.focus_manager.current().cloned() {
                    Some(comp_id) => match self.choice_select_event(&comp_id) {
                        Some(select) => select,
                        None => UserEvent::Click {
                            component_id: comp_id,
                        },
                    },
                    None => return Ok(None),
                },
                _ => return Ok(None),
            },
            other => other,
        };
        let (envelope, _effects) = self.core.handle_user_event(&event).await?;
        Ok(envelope)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use a2ui_core::message::client_to_server::V1_0ClientMessage;
    use a2ui_core::prelude::json;
    use a2ui_core::ComponentId;
    use ratatui::{
        style::{Color, Modifier},
        Terminal,
    };

    #[test]
    fn test_tui_renderer_new() {
        let renderer = TuiRenderer::new();
        assert!(renderer.core.surfaces().is_empty());
    }

    #[test]
    fn test_create_surface() {
        let renderer = TuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        let _msg = CreateSurface {
            surface_id: "s1".to_string(),
            catalog_id: "a2ui://catalogs/basic/v1".to_string(),
            surface_properties: None,
            send_data_model: false,
            components: Some(vec![comp]),
            data_model: None,
        };

        // 结构验证
        assert!(renderer.core.surfaces().is_empty());
    }

    #[test]
    fn test_delete_surface_removes_bindings() {
        let renderer = TuiRenderer::new();
        // 结构验证
        assert!(renderer.core.binding("s1").is_none());
    }

    #[tokio::test]
    async fn test_dependency_graph_populated_on_create_surface() {
        let mut renderer = TuiRenderer::new();
        let comp_a = Component::text(
            ComponentId::new("name_label").unwrap(),
            DynamicValue::Path {
                path: "/user/name".into(),
            },
        );
        let comp_b = Component::text(
            ComponentId::new("count_label").unwrap(),
            DynamicValue::Path {
                path: "/user/count".into(),
            },
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp_a, comp_b]),
                data_model: None,
            })
            .await
            .unwrap();

        let graph = renderer.dependency_graph();
        assert!(graph
            .get_dependencies(&ComponentId::new("name_label").unwrap())
            .unwrap()
            .contains("/user/name"));
        assert!(graph
            .get_dependencies(&ComponentId::new("count_label").unwrap())
            .unwrap()
            .contains("/user/count"));
    }

    #[tokio::test]
    async fn test_dependency_graph_isolates_affected_components() {
        let mut renderer = TuiRenderer::new();
        let comp_a = Component::text(
            ComponentId::new("name_label").unwrap(),
            DynamicValue::Path {
                path: "/user/name".into(),
            },
        );
        let comp_b = Component::text(
            ComponentId::new("count_label").unwrap(),
            DynamicValue::Path {
                path: "/user/count".into(),
            },
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp_a, comp_b]),
                data_model: None,
            })
            .await
            .unwrap();

        renderer
            .update_data_model(UpdateDataModel {
                surface_id: "s1".into(),
                path: Some("/user/name".into()),
                value: Some(json!("Alice")),
            })
            .await
            .unwrap();

        let graph = renderer.dependency_graph();
        let affected = graph.dependents("/user/name");
        assert_eq!(affected.len(), 1);
        assert_eq!(affected[0].as_str(), "name_label");
    }

    #[tokio::test]
    async fn test_render_frame_produces_widgets() {
        use ratatui::backend::TestBackend;

        let mut renderer = TuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("title").unwrap(),
            DynamicValue::Literal("Hello".into()),
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: None,
            })
            .await
            .unwrap();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        renderer.render_frame(&mut terminal).await.unwrap();

        let buf = terminal.backend().buffer();
        assert!(buf.area().width > 0);
    }

    #[tokio::test]
    async fn test_render_frame_with_column_layout() {
        use ratatui::backend::TestBackend;

        let mut renderer = TuiRenderer::new();
        let title = Component::text(
            ComponentId::new("title").unwrap(),
            DynamicValue::Literal("Title".into()),
        );
        let root = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("title").unwrap()],
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![root, title]),
                data_model: None,
            })
            .await
            .unwrap();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        renderer.render_frame(&mut terminal).await.unwrap();

        let buf = terminal.backend().buffer();
        assert!(buf.area().width > 0);
        assert!(buf.area().height > 0);
    }

    #[tokio::test]
    async fn test_render_frame_applies_styled_text_degraded_tui_style() {
        use ratatui::backend::TestBackend;

        let mut renderer = TuiRenderer::new();
        let comp: Component = Component::from_value(json!({
            "id": "root",
            "component": "Text",
            "text": "Styled",
            "style": {
                "strong": true,
                "color": "#112233",
                "fill": "#445566",
                "padding": 9,
                "radius": 5
            }
        }))
        .unwrap();
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: None,
            })
            .await
            .unwrap();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        renderer.render_frame(&mut terminal).await.unwrap();

        let cell = terminal.backend().buffer().get(0, 0);
        assert_eq!(cell.symbol(), "S");
        assert_eq!(cell.fg, Color::Rgb(17, 34, 51));
        assert_eq!(cell.bg, Color::Reset);
        assert!(cell.modifier.contains(Modifier::BOLD));
    }

    #[tokio::test]
    async fn test_render_frame_applies_styled_icon_degraded_tui_style() {
        use ratatui::backend::TestBackend;

        let mut renderer = TuiRenderer::new();
        let comp: Component = Component::from_value(json!({
            "id": "root",
            "component": "Icon",
            "name": "star",
            "style": {
                "strong": true,
                "color": "#112233",
                "fill": "#445566",
                "padding": 9,
                "radius": 5
            }
        }))
        .unwrap();
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: None,
            })
            .await
            .unwrap();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        renderer.render_frame(&mut terminal).await.unwrap();

        let cell = terminal.backend().buffer().get(0, 0);
        assert_eq!(cell.symbol(), "★");
        assert_eq!(cell.fg, Color::Rgb(17, 34, 51));
        assert_eq!(cell.bg, Color::Reset);
        assert!(cell.modifier.contains(Modifier::BOLD));
    }

    // --- Surface/component limit（核心流水线单测覆盖细节，此处为委托冒烟） ---

    #[tokio::test]
    async fn test_surface_limit_delegates_to_core_lru() {
        // 新语义（RendererCore）：满额时创建新 surface 先经 LRU 驱逐最旧者，
        // 创建成功而非报错；限额错误仅在驱逐机制不可用时兜底
        let mut renderer = TuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );

        // 创建 100 个 Surface（达到上限）
        for i in 0..100 {
            renderer
                .create_surface(CreateSurface {
                    surface_id: format!("s{}", i),
                    catalog_id: "a2ui://catalogs/basic/v1".into(),
                    surface_properties: None,
                    send_data_model: false,
                    components: Some(vec![comp.clone()]),
                    data_model: None,
                })
                .await
                .unwrap();
        }

        // 第 101 个：LRU 驱逐最旧的 s0 后创建成功
        renderer
            .create_surface(CreateSurface {
                surface_id: "overflow".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: None,
            })
            .await
            .unwrap();
        assert!(
            renderer.core.binding("s0").is_none(),
            "最旧 surface 应被驱逐"
        );
        assert!(renderer.core.binding("overflow").is_some());
    }

    #[tokio::test]
    async fn test_component_limit_enforced() {
        let mut renderer = TuiRenderer::new();
        // 创建 1001 个组件（超过上限 1000）
        let components: Vec<_> = (0..1001)
            .map(|i| {
                Component::text(
                    ComponentId::new(format!("c{}", i)).unwrap(),
                    DynamicValue::Literal(format!("text {}", i)),
                )
            })
            .collect();

        let result = renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(components),
                data_model: None,
            })
            .await;
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("component limit exceeded"));
        }
    }

    #[tokio::test]
    async fn test_component_limit_allows_under_max() {
        let mut renderer = TuiRenderer::new();
        let components: Vec<_> = (0..100)
            .map(|i| {
                Component::text(
                    ComponentId::new(format!("c{}", i)).unwrap(),
                    DynamicValue::Literal(format!("text {}", i)),
                )
            })
            .collect();

        let result = renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(components),
                data_model: None,
            })
            .await;
        assert!(result.is_ok());
    }

    // --- 状态机收紧（RendererCore）：重复 create / 未 create 就 update 被拒绝 ---

    #[tokio::test]
    async fn test_duplicate_create_surface_rejected() {
        let mut renderer = TuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        let msg = CreateSurface {
            surface_id: "s1".into(),
            catalog_id: "a2ui://catalogs/basic/v1".into(),
            surface_properties: None,
            send_data_model: false,
            components: Some(vec![comp]),
            data_model: None,
        };
        renderer.create_surface(msg.clone()).await.unwrap();
        renderer
            .create_surface(msg)
            .await
            .expect_err("同 id 重复 createSurface 应被状态机拒绝");
    }

    #[tokio::test]
    async fn test_update_before_create_rejected() {
        let mut renderer = TuiRenderer::new();
        renderer
            .update_components(UpdateComponents {
                surface_id: "ghost".into(),
                components: vec![Component::text(
                    ComponentId::new("c").unwrap(),
                    DynamicValue::Literal("x".to_string()),
                )],
            })
            .await
            .expect_err("未 createSurface 就 update 应被拒绝（不得隐式建 surface）");
        assert_eq!(renderer.core.forest().component_count("ghost"), 0);
    }

    // --- P0-4: callableFrom enforcement tests ---

    #[tokio::test]
    async fn test_call_function_client_only_allowed() {
        let mut renderer = TuiRenderer::new();
        renderer.register_function("validate", a2ui_renderer::CallableFrom::ClientOnly);

        let result = renderer
            .call_function(CallFunction {
                function_call_id: "fc1".into(),
                want_response: true,
                call: a2ui_core::message::server_to_client::CallFunctionPayload {
                    call: "validate".into(),
                    args: json!({"value": "test"}),
                },
            })
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_call_function_remote_only_rejected() {
        let mut renderer = TuiRenderer::new();
        renderer.register_function("fetch", a2ui_renderer::CallableFrom::RemoteOnly);

        let result = renderer
            .call_function(CallFunction {
                function_call_id: "fc1".into(),
                want_response: true,
                call: a2ui_core::message::server_to_client::CallFunctionPayload {
                    call: "fetch".into(),
                    args: json!({"url": "https://example.com"}),
                },
            })
            .await;
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("invalid function call"));
        }
    }

    #[tokio::test]
    async fn test_call_function_unregistered_rejected() {
        let mut renderer = TuiRenderer::new();

        let result = renderer
            .call_function(CallFunction {
                function_call_id: "fc1".into(),
                want_response: true,
                call: a2ui_core::message::server_to_client::CallFunctionPayload {
                    call: "unknown".into(),
                    args: json!({}),
                },
            })
            .await;
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("not available"));
        }
    }

    #[tokio::test]
    async fn test_call_function_client_or_remote_allowed() {
        let mut renderer = TuiRenderer::new();
        renderer.register_function("formatString", a2ui_renderer::CallableFrom::ClientOrRemote);

        let result = renderer
            .call_function(CallFunction {
                function_call_id: "fc1".into(),
                want_response: true,
                call: a2ui_core::message::server_to_client::CallFunctionPayload {
                    call: "formatString".into(),
                    args: json!({"template": "Hello"}),
                },
            })
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_catalog_trust_chain_rejects_unregistered() {
        let mut renderer = TuiRenderer::new();
        // 注册一个 Catalog
        let catalog = a2ui_core::Catalog::new("my-catalog".to_string());
        renderer.register_catalog(catalog).unwrap();

        // 使用未注册的 catalogId 创建 Surface → 应被拒绝
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        let result = renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "unknown-catalog".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: None,
            })
            .await;
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("not found"));
        }
    }

    #[tokio::test]
    async fn test_catalog_trust_chain_accepts_registered() {
        let mut renderer = TuiRenderer::new();
        let catalog = a2ui_core::Catalog::new("basic".to_string());
        renderer.register_catalog(catalog).unwrap();

        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        let result = renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: None,
            })
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_catalog_trust_chain_accepts_registered_catalog() {
        let mut renderer = TuiRenderer::new();
        // Basic Catalog 已自动注册 → 使用已注册的 catalogId 应成功
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        let result = renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: None,
            })
            .await;
        assert!(result.is_ok());
    }

    // --- P1-1: @index scope system — integration tests ---

    #[tokio::test]
    async fn test_template_expansion_integration() {
        let mut renderer = TuiRenderer::new();

        // Template: Text with relative path
        let template = Component::text(
            ComponentId::new("item_tmpl").unwrap(),
            DynamicValue::Path {
                path: "name".into(),
            },
        );

        // Parent with ChildList::Object
        let parent: Component =
            Component::from_value(json!({"component": "Column", "id": "list", "children": {"template": "item_tmpl", "path": "/items"}}))
                .unwrap();

        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![parent, template]),
                data_model: Some(json!({"items": [{"name": "a"}, {"name": "b"}]})),
            })
            .await
            .unwrap();

        // 验证展开的组件保留了路径绑定（相对路径被转为绝对路径）
        let comp0 = renderer
            .core
            .forest()
            .get("s1", &ComponentId::new("item_tmpl_0").unwrap());
        assert!(comp0.is_some());
        assert_eq!(
            comp0.unwrap().properties().get("text"),
            Some(&json!({"path": "/items/0/name"}))
        );
    }

    #[tokio::test]
    async fn test_template_expansion_with_at_index_integration() {
        let mut renderer = TuiRenderer::new();

        // Template using @index
        let template = Component::text(
            ComponentId::new("idx_tmpl").unwrap(),
            DynamicValue::FunctionCall {
                call: "@index".into(),
                args: json!({}),
            },
        );

        let parent: Component =
            Component::from_value(json!({"component": "Column", "id": "list", "children": {"template": "idx_tmpl", "path": "/items"}}))
                .unwrap();

        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![parent, template]),
                data_model: Some(json!({"items": [1, 2, 3]})),
            })
            .await
            .unwrap();

        let comp1 = renderer
            .core
            .forest()
            .get("s1", &ComponentId::new("idx_tmpl_1").unwrap());
        assert!(comp1.is_some());
        assert_eq!(comp1.unwrap().properties().get("text"), Some(&json!(1)));
    }

    // --- P1-2: responsePath 写回 ---

    #[tokio::test]
    async fn test_action_response_targets_registered_surface() {
        // s1、s2 的 data model 都含 /result：写回必须精确命中注册时指定的
        // surface，而不是 HashMap 迭代序里第一个含该路径的
        let mut renderer = TuiRenderer::new();
        for sid in ["s1", "s2"] {
            let comp = Component::text(
                ComponentId::new("root").unwrap(),
                DynamicValue::Literal("Hello".to_string()),
            );
            renderer
                .create_surface(CreateSurface {
                    surface_id: sid.into(),
                    catalog_id: "a2ui://catalogs/basic/v1".into(),
                    surface_properties: None,
                    send_data_model: false,
                    components: Some(vec![comp]),
                    data_model: Some(json!({"result": "pending"})),
                })
                .await
                .unwrap();
        }

        renderer.register_pending_response("action-1", "s2", "/result");
        renderer
            .action_response(ActionResponse {
                action_id: "action-1".into(),
                response: a2ui_core::message::server_to_client::ActionResponsePayload::Success(
                    json!("done"),
                ),
            })
            .await
            .unwrap();

        assert_eq!(
            renderer.core.binding("s2").unwrap().get("/result"),
            Some(&json!("done")),
            "注册到 s2 的响应应写入 s2"
        );
        assert_eq!(
            renderer.core.binding("s1").unwrap().get("/result"),
            Some(&json!("pending")),
            "s1 不应被误写"
        );
    }

    #[tokio::test]
    async fn test_action_response_missing_surface_warns_and_drops() {
        let mut renderer = TuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: Some(json!({"result": "pending"})),
            })
            .await
            .unwrap();

        // 注册到不存在的 surface：响应应被丢弃（warn），不写任何 binding、不报错
        renderer.register_pending_response("action-1", "s_gone", "/result");
        renderer
            .action_response(ActionResponse {
                action_id: "action-1".into(),
                response: a2ui_core::message::server_to_client::ActionResponsePayload::Success(
                    json!("done"),
                ),
            })
            .await
            .unwrap();

        assert_eq!(
            renderer.core.binding("s1").unwrap().get("/result"),
            Some(&json!("pending")),
            "任何 binding 都不应被写入"
        );
    }

    #[tokio::test]
    async fn test_action_response_writes_back_to_datamodel() {
        let mut renderer = TuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: Some(json!({"result": "pending"})),
            })
            .await
            .unwrap();

        // 注册 pending response
        renderer.register_pending_response("action-1", "s1", "/result");

        // 模拟服务器响应
        renderer
            .action_response(ActionResponse {
                action_id: "action-1".into(),
                response: a2ui_core::message::server_to_client::ActionResponsePayload::Success(
                    json!("done"),
                ),
            })
            .await
            .unwrap();

        // 验证 DataModel 已被更新
        let binding = renderer.core.binding("s1").unwrap();
        assert_eq!(binding.get("/result"), Some(&json!("done")));
    }

    #[tokio::test]
    async fn test_action_response_writes_error_to_datamodel() {
        let mut renderer = TuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: Some(json!({"error": null})),
            })
            .await
            .unwrap();

        renderer.register_pending_response("action-2", "s1", "/error");

        renderer
            .action_response(ActionResponse {
                action_id: "action-2".into(),
                response: a2ui_core::message::server_to_client::ActionResponsePayload::Error(
                    a2ui_core::message::server_to_client::ResponseError {
                        code: "TIMEOUT".into(),
                        message: "request timed out".into(),
                    },
                ),
            })
            .await
            .unwrap();

        let binding = renderer.core.binding("s1").unwrap();
        assert_eq!(binding.get("/error"), Some(&json!("request timed out")));
    }

    #[tokio::test]
    async fn test_action_response_unknown_action_id_is_noop() {
        let mut renderer = TuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: Some(json!({"result": "pending"})),
            })
            .await
            .unwrap();

        // 未注册的 action_id → 不应崩溃
        let result = renderer
            .action_response(ActionResponse {
                action_id: "unknown-action".into(),
                response: a2ui_core::message::server_to_client::ActionResponsePayload::Success(
                    json!("done"),
                ),
            })
            .await;
        assert!(result.is_ok());

        // DataModel 应未被修改
        let binding = renderer.core.binding("s1").unwrap();
        assert_eq!(binding.get("/result"), Some(&json!("pending")));
    }

    // --- 用户事件（docs/refactor-step0 规范语义）---

    /// 创建含单个绑定组件的 surface，返回 renderer
    async fn renderer_with_bound_component(component: a2ui_core::Value) -> TuiRenderer {
        let mut renderer = TuiRenderer::new();
        let comp: Component = Component::from_value(component).unwrap();
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: true,
                components: Some(vec![comp]),
                data_model: Some(json!({"form": {"username": "old"}})),
            })
            .await
            .unwrap();
        renderer.core.clear_dirty();
        renderer
    }

    #[tokio::test]
    async fn test_text_input_writes_back_without_message() {
        let mut renderer = renderer_with_bound_component(json!({
            "component":"TextField","id":"root","value":{"path":"/form/username"}
        }))
        .await;

        let envelope = renderer
            .handle_user_event(UserEvent::TextInput {
                component_id: ComponentId::new("root").unwrap(),
                value: "alice".into(),
            })
            .await
            .unwrap();

        // (a) 规范：被动输入变更不触发网络请求
        assert!(envelope.is_none(), "TextInput 不应产生消息");
        // (b) 绑定路径已更新
        assert_eq!(
            renderer.core.binding("s1").unwrap().get("/form/username"),
            Some(&json!("alice")),
            "输入值应写回 DataModel"
        );
        // (c) surface 被标脏
        assert!(renderer.core.dirty_surfaces().contains("s1"));
    }

    #[tokio::test]
    async fn test_check_toggle_writes_back_without_message() {
        let mut renderer = renderer_with_bound_component(json!({
            "component":"CheckBox","id":"root","checked":{"path":"/agree"}
        }))
        .await;

        let envelope = renderer
            .handle_user_event(UserEvent::CheckToggle {
                component_id: ComponentId::new("root").unwrap(),
                checked: true,
            })
            .await
            .unwrap();

        assert!(envelope.is_none(), "CheckToggle 不应产生消息");
        assert_eq!(
            renderer.core.binding("s1").unwrap().get("/agree"),
            Some(&json!(true))
        );
        assert!(renderer.core.dirty_surfaces().contains("s1"));
    }

    #[tokio::test]
    async fn test_slider_change_writes_back_without_message() {
        let mut renderer = renderer_with_bound_component(json!({
            "component":"Slider","id":"root","value":{"path":"/volume"},"min":0,"max":100
        }))
        .await;

        let envelope = renderer
            .handle_user_event(UserEvent::SliderChange {
                component_id: ComponentId::new("root").unwrap(),
                value: 42.5,
            })
            .await
            .unwrap();

        assert!(envelope.is_none(), "SliderChange 不应产生消息");
        assert_eq!(
            renderer.core.binding("s1").unwrap().get("/volume"),
            Some(&json!(42.5))
        );
        assert!(renderer.core.dirty_surfaces().contains("s1"));
    }

    #[tokio::test]
    async fn test_click_without_declared_action_emits_nothing() {
        let mut renderer = TuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: true,
                components: Some(vec![comp]),
                data_model: Some(json!({"user": {"name": "Alice"}})),
            })
            .await
            .unwrap();

        let envelope = renderer
            .handle_user_event(UserEvent::Click {
                component_id: ComponentId::new("root").unwrap(),
            })
            .await
            .unwrap();
        assert!(envelope.is_none(), "无声明 action 的组件交互不发送消息");
    }

    // --- P2-2: sendDataModel（经信封级 metadata 定向附带本 surface 数据）---

    /// 创建含声明式 action Button 的 surface
    async fn renderer_with_action_button(
        send_data_model: bool,
        data_model: a2ui_core::Value,
    ) -> TuiRenderer {
        let mut renderer = TuiRenderer::new();
        let btn: Component = Component::from_value(json!({
            "id":"btn","component":"Button","child":"lbl",
            "action":{"event":{"name":"submit"}}
        }))
        .unwrap();
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model,
                components: Some(vec![btn]),
                data_model: Some(data_model),
            })
            .await
            .unwrap();
        renderer
    }

    #[tokio::test]
    async fn test_send_data_model_includes_datamodel_in_action() {
        // 旧断言（action.context["dataModel"]）→ 新断言：数据模型经信封级
        // metadata 附带，且组件需声明 action.event 才发消息
        let mut renderer =
            renderer_with_action_button(true, json!({"user": {"name": "Alice"}})).await;

        let envelope = renderer
            .handle_user_event(UserEvent::Click {
                component_id: ComponentId::new("btn").unwrap(),
            })
            .await
            .unwrap()
            .expect("声明式 action 应产生消息");

        let metadata = envelope
            .metadata()
            .expect("sendDataModel 时应附带 metadata");
        assert_eq!(metadata.surface_id, "s1");
        assert_eq!(
            metadata.data_model,
            Some(json!({"user": {"name": "Alice"}}))
        );
        // action 本体不再携带 dataModel context
        let V1_0ClientMessage::Action(action) = envelope.message() else {
            panic!("envelope should carry an action message");
        };
        assert_eq!(action.name, "submit");
        assert!(!action.context.contains_key("dataModel"));
    }

    #[tokio::test]
    async fn test_send_data_model_false_excludes_datamodel() {
        let mut renderer = renderer_with_action_button(false, json!({"secret": "data"})).await;

        let envelope = renderer
            .handle_user_event(UserEvent::Click {
                component_id: ComponentId::new("btn").unwrap(),
            })
            .await
            .unwrap()
            .expect("声明式 action 应产生消息");

        assert!(
            envelope.metadata().is_none(),
            "sendDataModel=false 时不得附带数据模型"
        );
    }

    #[tokio::test]
    async fn test_send_data_model_includes_latest_state() {
        let mut renderer = renderer_with_action_button(true, json!({"count": 0})).await;

        // 更新 DataModel
        renderer
            .update_data_model(a2ui_core::message::server_to_client::UpdateDataModel {
                surface_id: "s1".into(),
                path: Some("/count".into()),
                value: Some(json!(5)),
            })
            .await
            .unwrap();

        // 触发 action 时 metadata 应包含最新 DataModel
        let envelope = renderer
            .handle_user_event(UserEvent::Click {
                component_id: ComponentId::new("btn").unwrap(),
            })
            .await
            .unwrap()
            .expect("声明式 action 应产生消息");

        let metadata = envelope.metadata().expect("metadata 应存在");
        assert_eq!(
            metadata.data_model.as_ref().and_then(|dm| dm.get("count")),
            Some(&json!(5))
        );
    }

    // --- P3-3: TextField/CheckBox/Slider rendering tests ---

    #[tokio::test]
    async fn test_render_frame_with_text_field() {
        use ratatui::backend::TestBackend;

        let mut renderer = TuiRenderer::new();
        let tf: Component = Component::from_json(
            r#"{"id":"name_input","component":"TextField","value":"Alice","placeholder":"Enter name"}"#
        ).unwrap();
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![tf]),
                data_model: None,
            })
            .await
            .unwrap();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        renderer.render_frame(&mut terminal).await.unwrap();

        let buf = terminal.backend().buffer();
        let content = buf.area();
        assert!(content.width > 0);
        assert!(content.height > 0);
    }

    #[tokio::test]
    async fn test_render_frame_with_checkbox() {
        use ratatui::backend::TestBackend;

        let mut renderer = TuiRenderer::new();
        let cb: Component = Component::from_json(
            r#"{"id":"agree","component":"CheckBox","checked":true,"label":"I agree"}"#,
        )
        .unwrap();
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![cb]),
                data_model: None,
            })
            .await
            .unwrap();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        renderer.render_frame(&mut terminal).await.unwrap();

        let buf = terminal.backend().buffer();
        let content = buf.area();
        assert!(content.width > 0);
        assert!(content.height > 0);
    }

    #[tokio::test]
    async fn test_render_frame_with_slider() {
        use ratatui::backend::TestBackend;

        let mut renderer = TuiRenderer::new();
        let sl: Component = Component::from_json(
            r#"{"id":"volume","component":"Slider","value":50,"min":0,"max":100}"#,
        )
        .unwrap();
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![sl]),
                data_model: None,
            })
            .await
            .unwrap();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        renderer.render_frame(&mut terminal).await.unwrap();

        let buf = terminal.backend().buffer();
        let content = buf.area();
        assert!(content.width > 0);
        assert!(content.height > 0);
    }

    // --- CustomComponentRegistry tests ---

    #[test]
    fn test_custom_component_registry() {
        let mut renderer = TuiRenderer::new();
        renderer
            .register_custom_component(a2ui_renderer::CustomComponentDef::new("MyChart"))
            .unwrap();
        assert!(renderer.core.custom_registry().is_registered("MyChart"));
    }

    #[test]
    fn test_custom_component_registry_duplicate_fails() {
        let mut renderer = TuiRenderer::new();
        renderer
            .register_custom_component(a2ui_renderer::CustomComponentDef::new("MyChart"))
            .unwrap();
        let result =
            renderer.register_custom_component(a2ui_renderer::CustomComponentDef::new("MyChart"));
        assert!(result.is_err());
    }

    // --- P4-1: Incremental rendering with DependencyGraph ---

    #[tokio::test]
    async fn test_incremental_render_marks_dirty_on_update_data_model() {
        let mut renderer = TuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("name_label").unwrap(),
            DynamicValue::Path {
                path: "/user/name".into(),
            },
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: Some(json!({"user": {"name": "Alice"}})),
            })
            .await
            .unwrap();

        // createSurface 本身标脏（核心语义），先清空以聚焦本测试
        renderer.core.clear_dirty();
        assert!(renderer.core.dirty_surfaces().is_empty());

        // 更新有依赖的路径 → 应标记 s1 为脏
        renderer
            .update_data_model(UpdateDataModel {
                surface_id: "s1".into(),
                path: Some("/user/name".into()),
                value: Some(json!("Bob")),
            })
            .await
            .unwrap();

        assert!(renderer.core.dirty_surfaces().contains("s1"));
    }

    #[tokio::test]
    async fn test_incremental_render_no_dirty_on_unbound_path() {
        let mut renderer = TuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("name_label").unwrap(),
            DynamicValue::Path {
                path: "/user/name".into(),
            },
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: Some(json!({"user": {"name": "Alice"}})),
            })
            .await
            .unwrap();
        renderer.core.clear_dirty();

        // 更新没有组件依赖的路径 → 不应标记为脏
        renderer
            .update_data_model(UpdateDataModel {
                surface_id: "s1".into(),
                path: Some("/other/path".into()),
                value: Some(json!("value")),
            })
            .await
            .unwrap();

        assert!(renderer.core.dirty_surfaces().is_empty());
    }

    #[tokio::test]
    async fn test_incremental_render_marks_dirty_on_action_response() {
        let mut renderer = TuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("result_label").unwrap(),
            DynamicValue::Path {
                path: "/result".into(),
            },
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: Some(json!({"result": "pending"})),
            })
            .await
            .unwrap();
        renderer.core.clear_dirty();

        // 注册 pending response
        renderer.register_pending_response("action-1", "s1", "/result");

        // 模拟服务器响应
        renderer
            .action_response(ActionResponse {
                action_id: "action-1".into(),
                response: a2ui_core::message::server_to_client::ActionResponsePayload::Success(
                    json!("done"),
                ),
            })
            .await
            .unwrap();

        // 应标记 s1 为脏（因为 /result 有组件依赖）
        assert!(renderer.core.dirty_surfaces().contains("s1"));
    }

    #[tokio::test]
    async fn test_incremental_render_clears_dirty_after_render() {
        use ratatui::backend::TestBackend;

        let mut renderer = TuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("name_label").unwrap(),
            DynamicValue::Path {
                path: "/user/name".into(),
            },
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: Some(json!({"user": {"name": "Alice"}})),
            })
            .await
            .unwrap();

        // 先标记为脏
        renderer
            .update_data_model(UpdateDataModel {
                surface_id: "s1".into(),
                path: Some("/user/name".into()),
                value: Some(json!("Bob")),
            })
            .await
            .unwrap();
        assert!(renderer.core.dirty_surfaces().contains("s1"));

        // render_frame 后应清除脏标记
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        renderer.render_frame(&mut terminal).await.unwrap();

        assert!(renderer.core.dirty_surfaces().is_empty());
    }

    #[tokio::test]
    async fn test_incremental_render_full_render_when_no_dirty() {
        use ratatui::backend::TestBackend;

        let mut renderer = TuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("name_label").unwrap(),
            DynamicValue::Path {
                path: "/user/name".into(),
            },
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: Some(json!({"user": {"name": "Alice"}})),
            })
            .await
            .unwrap();
        renderer.core.clear_dirty();

        // 没有脏 surface 时，render_frame 应正常渲染所有 surface
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let result = renderer.render_frame(&mut terminal).await;
        assert!(result.is_ok());
        assert!(renderer.core.dirty_surfaces().is_empty());
    }

    // ---- render() 帧准备测试 ----

    #[tokio::test]
    async fn test_render_builds_widgets_for_valid_surface() {
        let mut renderer = TuiRenderer::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp]),
                data_model: None,
            })
            .await
            .unwrap();

        let result = renderer.render().await;
        assert!(result.is_ok());
        // render() 应构建了至少 1 个 widget（root 组件）
        assert!(renderer.last_frame_widget_count > 0);
    }

    #[tokio::test]
    async fn test_render_handles_empty_state() {
        let mut renderer = TuiRenderer::new();
        let result = renderer.render().await;
        assert!(result.is_ok());
        // 无 surface 时 widget 数为 0
        assert_eq!(renderer.last_frame_widget_count, 0);
    }

    // ---- FocusManager integration tests ----

    #[tokio::test]
    async fn test_focus_manager_collects_focusable_after_render() {
        let mut renderer = TuiRenderer::new();
        let btn: Component = Component::from_json(
            r#"{"id":"btn","component":"Button","child":"lbl","text":"Click"}"#,
        )
        .unwrap();
        let tf: Component = Component::from_json(
            r#"{"id":"tf","component":"TextField","value":"","placeholder":"Enter"}"#,
        )
        .unwrap();
        let root: Component =
            Component::from_json(r#"{"id":"root","component":"Column","children":["btn","tf"]}"#)
                .unwrap();
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![root, btn, tf]),
                data_model: None,
            })
            .await
            .unwrap();

        renderer.render().await.unwrap();
        // 焦点管理器应收集了 2 个可聚焦组件（Button + TextField）
        assert!(renderer.focus_manager.focusable_count() >= 1);
    }

    #[tokio::test]
    async fn test_tab_key_navigates_focus() {
        let mut renderer = TuiRenderer::new();
        let btn: Component =
            Component::from_json(r#"{"id":"btn","component":"Button","child":"lbl","text":"OK"}"#)
                .unwrap();
        let root: Component =
            Component::from_json(r#"{"id":"root","component":"Column","children":["btn"]}"#)
                .unwrap();
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![root, btn]),
                data_model: None,
            })
            .await
            .unwrap();
        renderer.render().await.unwrap();

        // Tab 键切换焦点，本地行为不产生消息
        let result = renderer
            .handle_user_event(UserEvent::KeyPress { key: "Tab".into() })
            .await
            .unwrap();
        assert!(result.is_none(), "焦点导航不应发送消息");
    }

    #[tokio::test]
    async fn test_enter_activates_focused_component_as_click() {
        // KeyPress 本地转译（docs/refactor-step0 D7）：Enter = 焦点组件的
        // Click，交核心按声明式 action 构造消息
        let mut renderer = TuiRenderer::new();
        let btn: Component = Component::from_value(json!({
            "id":"btn","component":"Button","child":"lbl",
            "action":{"event":{"name":"submit"}}
        }))
        .unwrap();
        let root: Component =
            Component::from_json(r#"{"id":"root","component":"Column","children":["btn"]}"#)
                .unwrap();
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![root, btn]),
                data_model: None,
            })
            .await
            .unwrap();
        renderer.render().await.unwrap();

        // Tab 把焦点移到 btn
        renderer
            .handle_user_event(UserEvent::KeyPress { key: "Tab".into() })
            .await
            .unwrap();
        assert_eq!(
            renderer.focus_manager.current().map(|id| id.as_str()),
            Some("btn")
        );

        // Enter 激活：产生声明的 submit action，来源为焦点组件
        let envelope = renderer
            .handle_user_event(UserEvent::KeyPress {
                key: "Enter".into(),
            })
            .await
            .unwrap()
            .expect("Enter 应转译为焦点组件的 Click 并产生声明式 action");
        let value = envelope.to_value().unwrap();
        assert_eq!(value["action"]["name"], "submit");
        assert_eq!(value["action"]["surfaceId"], "s1");
        assert_eq!(value["action"]["sourceComponentId"], "btn");
    }

    #[tokio::test]
    async fn test_enter_without_focus_emits_nothing() {
        let mut renderer = TuiRenderer::new();
        let envelope = renderer
            .handle_user_event(UserEvent::KeyPress {
                key: "Enter".into(),
            })
            .await
            .unwrap();
        assert!(envelope.is_none());
    }

    async fn renderer_with_choice_picker(variant: Option<&str>) -> TuiRenderer {
        let mut renderer = TuiRenderer::new();
        let mut cp = json!({
            "id":"cp","component":"ChoicePicker",
            "options":[{"label":"Email","value":"email"},{"label":"SMS","value":"sms"}],
            "value":{"path":"/pref"}
        });
        if let Some(v) = variant {
            cp["variant"] = json!(v);
        }
        let root: Component =
            Component::from_json(r#"{"id":"root","component":"Column","children":["cp"]}"#)
                .unwrap();
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![root, Component::from_value(cp).unwrap()]),
                data_model: Some(json!({"pref": []})),
            })
            .await
            .unwrap();
        renderer.render().await.unwrap();
        // Tab 聚焦到 cp（唯一可聚焦组件）
        renderer
            .handle_user_event(UserEvent::KeyPress { key: "Tab".into() })
            .await
            .unwrap();
        assert_eq!(
            renderer.focus_manager.current().map(|id| id.as_str()),
            Some("cp")
        );
        renderer
    }

    #[tokio::test]
    async fn test_choice_picker_keys_move_cursor_and_toggle_selection() {
        // 多选：Right 移动选项游标，Enter 经 toggle_choice 写回完整选中集
        let mut renderer = renderer_with_choice_picker(Some("multipleSelection")).await;

        renderer
            .handle_user_event(UserEvent::KeyPress {
                key: "Right".into(),
            })
            .await
            .unwrap();
        let envelope = renderer
            .handle_user_event(UserEvent::KeyPress {
                key: "Enter".into(),
            })
            .await
            .unwrap();
        assert!(envelope.is_none(), "ChoiceSelect 不应产生消息");
        assert_eq!(
            renderer.core.binding("s1").unwrap().get("/pref"),
            Some(&json!(["sms"]))
        );

        // 多选再次 Enter 反选回空集
        renderer
            .handle_user_event(UserEvent::KeyPress {
                key: "Enter".into(),
            })
            .await
            .unwrap();
        assert_eq!(
            renderer.core.binding("s1").unwrap().get("/pref"),
            Some(&json!([]))
        );
    }

    #[tokio::test]
    async fn test_render_frame_marks_choice_cursor_when_focused() {
        use ratatui::backend::TestBackend;

        // 键盘交互需要可见的游标反馈：焦点 ChoicePicker 的游标选项前缀 ▸
        let mut renderer = renderer_with_choice_picker(None).await;
        renderer
            .handle_user_event(UserEvent::KeyPress {
                key: "Right".into(),
            })
            .await
            .unwrap();

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        renderer.render_frame(&mut terminal).await.unwrap();

        let buf = terminal.backend().buffer();
        let text: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            text.contains("▸( ) SMS"),
            "游标应标记在 SMS 选项前，got: {text}"
        );
    }

    #[tokio::test]
    async fn test_choice_picker_enter_defaults_to_first_option_single_select() {
        // 无 variant（规范默认单选）：游标默认第 0 项，Enter 整体替换
        let mut renderer = renderer_with_choice_picker(None).await;
        renderer
            .handle_user_event(UserEvent::KeyPress {
                key: "Enter".into(),
            })
            .await
            .unwrap();
        assert_eq!(
            renderer.core.binding("s1").unwrap().get("/pref"),
            Some(&json!(["email"]))
        );
        // 单选重复激活保持选中（不反选）
        renderer
            .handle_user_event(UserEvent::KeyPress {
                key: "Enter".into(),
            })
            .await
            .unwrap();
        assert_eq!(
            renderer.core.binding("s1").unwrap().get("/pref"),
            Some(&json!(["email"]))
        );
    }
}
