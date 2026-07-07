use crate::html_builder::{HtmlBuilder, RenderableHtmlWidget};
use a2ui_core::component::prop_keys;
use a2ui_core::message::{
    client_to_server::FunctionResponse,
    server_to_client::{
        ActionResponse, CallFunction, CreateSurface, DeleteSurface, UpdateComponents,
        UpdateDataModel,
    },
};
use a2ui_core::prelude::*;
use a2ui_core::Value;
use a2ui_renderer::{
    choice_options, choice_selected, resolve_bool, resolve_f64, resolve_str, ComponentStyle,
    CoreEffects, CustomComponentRegistry, DataBinding, RenderResult, Renderer, RendererCore,
    SurfaceHandle, UserEvent,
};
use std::collections::HashMap;

/// Web 渲染器实现
///
/// 协议状态与消息处理全部委托 [`RendererCore`]，本类型只保留平台特有部分：
/// HTML 构建与 surface 级 HTML 缓存（`last_html`，按核心返回的
/// [`CoreEffects`] 失效）。支持服务端渲染模式，通过 `render_surface_html()`
/// 和 `render_all_html()` 方法输出 HTML。
///
/// # 示例
///
/// ```rust
/// use a2ui_renderer_web::WebRenderer;
///
/// let renderer = WebRenderer::new();
/// assert!(renderer.core.surfaces().is_empty());
/// ```
#[derive(Debug)]
pub struct WebRenderer {
    /// 渲染器公共核心（协议状态 + 消息流水线；pub 供同 crate 测试访问）
    pub core: RendererCore,
    /// 当前聚焦的组件（现为死代码：无写入路径，保留待焦点专项）
    focused_component: Option<ComponentId>,
    /// 最近一次渲染的 HTML 输出（surface_id → HTML body）
    last_html: HashMap<String, String>,
    /// HTML 构建器
    html_builder: HtmlBuilder,
}

impl WebRenderer {
    /// 创建新的 Web 渲染器
    pub fn new() -> Self {
        Self {
            core: RendererCore::new(),
            focused_component: None,
            last_html: HashMap::new(),
            html_builder: HtmlBuilder::new(),
        }
    }

    /// 获取依赖图的只读引用（用于测试和查询）
    pub fn dependency_graph(&self) -> &a2ui_renderer::DependencyGraph {
        self.core.dependency_graph()
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

    /// 消费核心返回的缓存失效回执：整 surface 失效时清对应 HTML 缓存
    /// （组件级失效 web 无对应缓存粒度，忽略）
    fn apply_effects(&mut self, effects: &CoreEffects) {
        for surface_id in &effects.invalidated_surfaces {
            self.last_html.remove(surface_id);
        }
    }

    /// 渲染指定 Surface 为 HTML 字符串
    ///
    /// 返回完整的 HTML body 内容，不包含页面包装（`<html>`、`<head>` 等）。
    /// 使用 `render_page()` 可以包装为完整 HTML 页面。
    pub fn render_surface_html(&self, surface_id: &str) -> Option<String> {
        // 如果有缓存的 HTML 且 surface 不在脏集合中，直接返回缓存
        if !self.core.dirty_surfaces().contains(surface_id) {
            if let Some(cached) = self.last_html.get(surface_id) {
                return Some(cached.clone());
            }
        }

        // 构建组件树
        let tree = self.core.forest().build_tree(surface_id).ok()?;

        // 获取 data binding
        let binding = self.core.binding(surface_id)?;

        // 将组件树映射为 RenderableHtmlWidget 树
        let root_widget = Self::build_widget_tree(&tree, binding, self.core.custom_registry())?;

        // 渲染为 HTML
        Some(self.html_builder.render(&root_widget))
    }

    /// 渲染所有 Surface 为完整 HTML 页面
    ///
    /// 将所有 Surface 的 HTML body 拼接后嵌入完整页面模板。
    pub fn render_all_html(&self) -> String {
        let all_surface_ids: Vec<String> = self.core.surfaces().values().cloned().collect();
        let mut bodies = String::new();

        for surface_id in &all_surface_ids {
            if let Some(html) = self.render_surface_html(surface_id) {
                bodies.push_str(&format!(
                    "<div class=\"a2ui-surface\" data-surface-id=\"{}\">",
                    html_attr(surface_id)
                ));
                bodies.push_str(&html);
                bodies.push_str("</div>");
            }
        }

        self.html_builder.render_page(&bodies, "A2UI")
    }

    /// 递归构建 RenderableHtmlWidget 树
    fn build_widget_tree(
        node: &a2ui_renderer::component_forest::ComponentTreeNode,
        binding: &DataBinding,
        registry: &CustomComponentRegistry,
    ) -> Option<RenderableHtmlWidget> {
        let component = &node.component;
        let ctype = component.component_type();

        // 递归构建子 widget，保留源组件 id 以便 Modal/Tabs 按 id 匹配
        // （组件树 children 的顺序是构建细节，不可按位置消费）
        let mut child_pairs: Vec<(String, RenderableHtmlWidget)> = Vec::new();
        for child in &node.children {
            if let Some(widget) = Self::build_widget_tree(child, binding, registry) {
                child_pairs.push((child.component.id().as_str().to_string(), widget));
            }
        }

        let widget = match ctype {
            "Text" => {
                let text = extract_text(component, binding);
                let variant = component
                    .prop_str(prop_keys::VARIANT)
                    .unwrap_or("body")
                    .to_string();
                RenderableHtmlWidget::Text {
                    id: component.id().clone(),
                    text,
                    variant,
                }
            }
            "Button" => {
                let label = extract_text(component, binding);
                let variant = component
                    .prop_str(prop_keys::VARIANT)
                    .unwrap_or("default")
                    .to_string();
                RenderableHtmlWidget::Button {
                    id: component.id().clone(),
                    label,
                    variant,
                }
            }
            "Column" => RenderableHtmlWidget::Column {
                id: component.id().clone(),
                children: child_pairs.into_iter().map(|(_, w)| w).collect(),
            },
            "Row" => RenderableHtmlWidget::Row {
                id: component.id().clone(),
                children: child_pairs.into_iter().map(|(_, w)| w).collect(),
            },
            "Image" => {
                let url =
                    extract_string_value(component, prop_keys::URL, binding).unwrap_or_default();
                RenderableHtmlWidget::Image {
                    id: component.id().clone(),
                    url,
                }
            }
            "Card" => {
                let child = child_pairs.into_iter().next().map(|(_, w)| w).unwrap_or(
                    RenderableHtmlWidget::Placeholder {
                        id: component.id().clone(),
                        reason: "empty card".to_string(),
                    },
                );
                RenderableHtmlWidget::Card {
                    id: component.id().clone(),
                    child: Box::new(child),
                }
            }
            "CheckBox" => {
                let checked = resolve_bool_value(component, prop_keys::VALUE, binding)
                    .or_else(|| resolve_bool_value(component, prop_keys::CHECKED, binding))
                    .unwrap_or(false);
                let label =
                    extract_string_value(component, prop_keys::LABEL, binding).unwrap_or_default();
                RenderableHtmlWidget::CheckBox {
                    id: component.id().clone(),
                    checked,
                    label,
                }
            }
            "Divider" => RenderableHtmlWidget::Divider {
                id: component.id().clone(),
            },
            "Icon" => {
                let name = extract_string_value(component, prop_keys::NAME, binding)
                    .unwrap_or("?".to_string());
                RenderableHtmlWidget::Icon {
                    id: component.id().clone(),
                    name,
                }
            }
            "List" => RenderableHtmlWidget::List {
                id: component.id().clone(),
                children: child_pairs.into_iter().map(|(_, w)| w).collect(),
            },
            "Tabs" => {
                // 按 tabs[].child 的 id 从 child_pairs 中匹配各 tab 内容
                let mut remaining = child_pairs;
                let tabs_data: Vec<(String, Vec<RenderableHtmlWidget>)> = component
                    .tabs_decl()
                    .map(|tabs| {
                        tabs.into_iter()
                            .map(|tab| {
                                let children = remaining
                                    .iter()
                                    .position(|(id, _)| id == tab.child.as_str())
                                    .map(|pos| vec![remaining.remove(pos).1])
                                    .unwrap_or_default();
                                (tab.title, children)
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                // 如果从 props 读取到了 tabs 数据，使用该数据
                if !tabs_data.is_empty() {
                    RenderableHtmlWidget::Tabs {
                        id: component.id().clone(),
                        tabs: tabs_data,
                    }
                } else {
                    // 否则使用全部子 widget 作为单一 tab（无元数据的降级路径）
                    RenderableHtmlWidget::Tabs {
                        id: component.id().clone(),
                        tabs: vec![(
                            "Tab".to_string(),
                            remaining.into_iter().map(|(_, w)| w).collect(),
                        )],
                    }
                }
            }
            "Modal" => {
                let title = extract_string_value(component, prop_keys::TITLE, binding)
                    .or_else(|| extract_string_value(component, prop_keys::LABEL, binding))
                    .unwrap_or_default();
                // 按 props.content 的 id 匹配（children 可能同时含 trigger）；
                // 无 content 元数据时降级取第一个非 trigger 的子 widget
                let trigger_id = component.prop_str(prop_keys::TRIGGER);
                let content_widget = match component.prop_str(prop_keys::CONTENT) {
                    Some(content_id) => child_pairs
                        .into_iter()
                        .find(|(id, _)| id == content_id)
                        .map(|(_, w)| w),
                    None => child_pairs
                        .into_iter()
                        .find(|(id, _)| Some(id.as_str()) != trigger_id)
                        .map(|(_, w)| w),
                };
                let content = content_widget.unwrap_or(RenderableHtmlWidget::Placeholder {
                    id: component.id().clone(),
                    reason: "empty modal".to_string(),
                });
                RenderableHtmlWidget::Modal {
                    id: component.id().clone(),
                    title,
                    content: Box::new(content),
                }
            }
            "Slider" => {
                let value =
                    resolve_number_value(component, prop_keys::VALUE, binding).unwrap_or(0.0);
                let min = resolve_number_value(component, prop_keys::MIN, binding).unwrap_or(0.0);
                let max = resolve_number_value(component, prop_keys::MAX, binding).unwrap_or(100.0);
                RenderableHtmlWidget::Slider {
                    id: component.id().clone(),
                    value,
                    min,
                    max,
                }
            }
            "TextField" => {
                let value =
                    extract_string_value(component, prop_keys::VALUE, binding).unwrap_or_default();
                let placeholder = extract_string_value(component, prop_keys::PLACEHOLDER, binding)
                    .unwrap_or_default();
                RenderableHtmlWidget::TextField {
                    id: component.id().clone(),
                    value,
                    placeholder,
                }
            }
            "ChoicePicker" => RenderableHtmlWidget::ChoicePicker {
                id: component.id().clone(),
                options: choice_options(component, Some(binding)),
                selected: choice_selected(component, Some(binding)),
            },
            "DateTimeInput" => {
                let label = extract_string_value(component, prop_keys::LABEL, binding)
                    .unwrap_or("Select date/time".to_string());
                RenderableHtmlWidget::DateTimeInput {
                    id: component.id().clone(),
                    label,
                }
            }
            "Video" => {
                let url =
                    extract_string_value(component, prop_keys::URL, binding).unwrap_or_default();
                RenderableHtmlWidget::Video {
                    id: component.id().clone(),
                    url,
                }
            }
            "AudioPlayer" => {
                let url =
                    extract_string_value(component, prop_keys::URL, binding).unwrap_or_default();
                RenderableHtmlWidget::AudioPlayer {
                    id: component.id().clone(),
                    url,
                }
            }
            _ => {
                // 先检查自定义组件注册表
                if registry.is_registered(ctype) {
                    RenderableHtmlWidget::Placeholder {
                        id: component.id().clone(),
                        reason: format!("custom component: {}", ctype),
                    }
                } else {
                    RenderableHtmlWidget::Placeholder {
                        id: component.id().clone(),
                        reason: format!("unknown component type: {}", ctype),
                    }
                }
            }
        };

        let style = ComponentStyle::from_component(component);
        let widget = if supports_web_style(ctype) && style != ComponentStyle::default() {
            RenderableHtmlWidget::Styled {
                widget: Box::new(widget),
                style,
            }
        } else {
            widget
        };

        Some(widget)
    }

    /// 执行渲染：将脏 surface 渲染为 HTML 并缓存结果
    pub fn render_to_html(&mut self) -> RenderResult<()> {
        let surfaces_to_render: Vec<_> = if self.core.dirty_surfaces().is_empty() {
            self.core.surfaces().values().cloned().collect()
        } else {
            self.core.dirty_surfaces().iter().cloned().collect()
        };

        for surface_id in &surfaces_to_render {
            if let Some(html) = self.render_surface_html(surface_id) {
                self.last_html.insert(surface_id.clone(), html);
            }
        }

        self.core.clear_dirty();
        Ok(())
    }
}

impl Default for WebRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Renderer for WebRenderer {
    async fn create_surface(&mut self, msg: CreateSurface) -> RenderResult<SurfaceHandle> {
        tracing::trace!(surface_id = %msg.surface_id, "createSurface");
        let (handle, effects) = self.core.create_surface(msg).await?;
        self.apply_effects(&effects);
        Ok(handle)
    }

    async fn update_components(&mut self, msg: UpdateComponents) -> RenderResult<()> {
        let effects = self.core.update_components(msg).await?;
        self.apply_effects(&effects);
        Ok(())
    }

    async fn update_data_model(&mut self, msg: UpdateDataModel) -> RenderResult<()> {
        tracing::debug!(surface_id = %msg.surface_id, path = ?msg.path, "updateDataModel");
        let effects = self.core.update_data_model(msg).await?;
        self.apply_effects(&effects);
        Ok(())
    }

    async fn delete_surface(&mut self, msg: DeleteSurface) -> RenderResult<()> {
        tracing::info!(surface_id = %msg.surface_id, "deleteSurface");
        let effects = self.core.delete_surface(msg).await?;
        self.apply_effects(&effects);
        Ok(())
    }

    async fn action_response(&mut self, msg: ActionResponse) -> RenderResult<()> {
        let effects = self.core.action_response(msg).await?;
        self.apply_effects(&effects);
        Ok(())
    }

    async fn call_function(&mut self, msg: CallFunction) -> RenderResult<FunctionResponse> {
        self.core.call_function(msg).await
    }

    async fn render(&mut self) -> RenderResult<()> {
        self.render_to_html()
    }

    async fn handle_user_event(
        &mut self,
        event: UserEvent,
    ) -> RenderResult<Option<a2ui_core::ClientEnvelope>> {
        // KeyPress 是渲染器本地行为（docs/refactor-step0 D7）：
        // Enter/空格 转译为焦点组件的 Click 再交公共核心（有声明 action
        // 才发消息）。focused_component 现无写入路径（待焦点专项）。
        let event = match event {
            UserEvent::KeyPress { key } => match key.as_str() {
                "Enter" | " " => match self.focused_component.clone() {
                    Some(comp_id) => UserEvent::Click {
                        component_id: comp_id,
                    },
                    None => return Ok(None),
                },
                _ => return Ok(None),
            },
            other => other,
        };
        let (envelope, effects) = self.core.handle_user_event(&event).await?;
        self.apply_effects(&effects);
        Ok(envelope)
    }
}

/// 从组件 properties 中提取文本内容，支持 DynamicValue::Path 解析
fn extract_text(component: &Component, binding: &DataBinding) -> String {
    extract_string_value(component, prop_keys::TEXT, binding).unwrap_or_default()
}

/// 从组件 properties 中提取字符串类型的值，支持 DynamicValue::Path 解析
fn extract_string_value(component: &Component, key: &str, binding: &DataBinding) -> Option<String> {
    component
        .prop_dynamic_value(key)
        .map(|dv| resolve_str(&dv, Some(binding)))
}

fn resolve_bool_value(component: &Component, key: &str, binding: &DataBinding) -> Option<bool> {
    component
        .prop_dynamic_bool(key)
        .and_then(|dv| resolve_bool(&dv, Some(binding)))
}

fn resolve_number_value(component: &Component, key: &str, binding: &DataBinding) -> Option<f64> {
    component
        .prop_dynamic_f64(key)
        .and_then(|dv| resolve_f64(&dv, Some(binding)))
}

/// 从 JSON 值中提取静态字符串（非 DynamicValue）
///
/// 现无生产消费者（ChoicePicker 解析已委托 `prop_str_list`），
/// 按仓库约定保留不删。
#[allow(dead_code)]
fn extract_static_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

fn supports_web_style(component_type: &str) -> bool {
    matches!(
        component_type,
        "Text" | "Icon" | "Row" | "Column" | "List" | "Card" | "Image"
    )
}

/// HTML 属性值转义
fn html_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use a2ui_core::prelude::json;
    use a2ui_core::ComponentId;

    #[test]
    fn test_web_renderer_new() {
        let renderer = WebRenderer::new();
        assert!(renderer.core.surfaces().is_empty());
    }

    #[test]
    fn test_register_function() {
        let mut renderer = WebRenderer::new();
        renderer.register_function("upper", a2ui_renderer::CallableFrom::ClientOrRemote);
        assert!(renderer
            .registered_functions()
            .iter()
            .any(|s| s.as_str() == "upper"));
    }

    #[test]
    fn test_register_catalog() {
        let mut renderer = WebRenderer::new();
        let catalog = a2ui_core::Catalog::new("basic").with_instructions("Basic catalog");
        assert!(renderer.register_catalog(catalog).is_ok());
    }

    #[tokio::test]
    async fn test_create_and_render_surface() {
        let mut renderer = WebRenderer::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello World".into()),
        );
        let handle = renderer
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

        assert!(renderer.core.surfaces().contains_key(&handle));

        let html = renderer.render_surface_html("s1");
        assert!(html.is_some());
        assert!(html.unwrap().contains("Hello World"));
    }

    #[tokio::test]
    async fn test_text_input_writes_back_and_renders_new_value() {
        // 旧断言（合成 input 消息 + context dataModel 快照）→ 新断言：
        // 规范：被动输入变更不触发网络请求，只写回数据模型；
        // 写回后渲染的 HTML 含新值（dirty → 缓存失效链路）
        let mut renderer = WebRenderer::new();
        let field: Component = Component::from_value(json!({
            "component":"TextField","id":"root","value":{"path":"/form/username"}
        }))
        .unwrap();
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: true,
                components: Some(vec![field]),
                data_model: Some(json!({"form": {"username": "old"}})),
            })
            .await
            .unwrap();
        // 先渲染一次，缓存旧值并清脏（使断言穿过缓存路径而非仅初次渲染）
        renderer.render_to_html().unwrap();
        assert!(renderer
            .render_surface_html("s1")
            .expect("html")
            .contains("old"));

        let envelope = renderer
            .handle_user_event(UserEvent::TextInput {
                component_id: ComponentId::new("root").unwrap(),
                value: "alice".into(),
            })
            .await
            .unwrap();

        // 规范：被动输入变更不触发网络请求
        assert!(envelope.is_none(), "TextInput 不应产生消息");
        // 绑定路径已更新
        assert_eq!(
            renderer.core.binding("s1").unwrap().get("/form/username"),
            Some(&json!("alice"))
        );
        // 写回后渲染的 HTML 含新值（脏标记 → 缓存失效链路）
        let html = renderer.render_surface_html("s1").expect("html");
        assert!(html.contains("alice"), "HTML 应含写回后的新值: {html}");
    }

    #[tokio::test]
    async fn test_check_toggle_writes_back_to_data_model() {
        // 旧断言（合成 toggle 消息）→ 新断言：无消息 + 写回 + 标脏
        let mut renderer = WebRenderer::new();
        let checkbox: Component = Component::from_value(json!({
            "component":"CheckBox","id":"root","checked":{"path":"/agree"}
        }))
        .unwrap();
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![checkbox]),
                data_model: Some(json!({"agree": false})),
            })
            .await
            .unwrap();
        // createSurface 本身标脏（核心语义），先清空以聚焦本测试
        renderer.core.clear_dirty();

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
    async fn test_click_without_declared_action_emits_nothing() {
        let mut renderer = WebRenderer::new();
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

    #[tokio::test]
    async fn test_click_with_declared_action_emits_spec_envelope() {
        let mut renderer = WebRenderer::new();
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
                send_data_model: true,
                components: Some(vec![btn]),
                data_model: Some(json!({"user": {"name": "Alice"}})),
            })
            .await
            .unwrap();

        let envelope = renderer
            .handle_user_event(UserEvent::Click {
                component_id: ComponentId::new("btn").unwrap(),
            })
            .await
            .unwrap()
            .expect("声明式 action 应产生消息");
        let value = envelope.to_value().unwrap();
        assert_eq!(value["action"]["name"], "submit");
        assert_eq!(value["action"]["surfaceId"], "s1");
        assert_eq!(value["action"]["sourceComponentId"], "btn");
        // sendDataModel 经信封级 metadata 附带本 surface 数据
        assert_eq!(value["metadata"]["surfaceId"], "s1");
        assert_eq!(value["metadata"]["dataModel"]["user"]["name"], "Alice");
    }

    #[tokio::test]
    async fn test_update_components_invalidates_html_cache() {
        // 原审查缺陷 #24 回归测试（effects → last_html 失效链路的
        // 首个真实消费者）：update 后新组件的渲染输出必须出现，
        // 即使脏标记已被清除也不得返回陈旧缓存
        let mut renderer = WebRenderer::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("OLD_CONTENT".into()),
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
        // 渲染并缓存旧内容，清脏
        renderer.render_to_html().unwrap();
        assert!(renderer
            .render_surface_html("s1")
            .expect("html")
            .contains("OLD_CONTENT"));

        // 更新组件
        let new_comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("NEW_CONTENT".into()),
        );
        renderer
            .update_components(UpdateComponents {
                surface_id: "s1".into(),
                components: vec![new_comp],
            })
            .await
            .unwrap();

        // 人为清除脏标记：验证缓存本身已被 effects 失效，
        // 而不是靠 dirty 检查绕过缓存
        renderer.core.clear_dirty();
        let html = renderer.render_surface_html("s1").expect("html");
        assert!(
            html.contains("NEW_CONTENT"),
            "update_components 后不得返回陈旧缓存: {html}"
        );
    }

    #[tokio::test]
    async fn test_modal_widget_gets_content_by_id() {
        let mut renderer = WebRenderer::new();
        let components: Vec<Component> = vec![
            Component::from_value(json!({
                "id":"root","component":"Modal",
                "title":"Confirm","content":"body","trigger":"btn"
            }))
            .unwrap(),
            Component::from_value(json!({
                "id":"body","component":"Text","text":"HELLO_MODAL"
            }))
            .unwrap(),
            Component::from_value(json!({
                "id":"btn","component":"Button","label":"open"
            }))
            .unwrap(),
        ];
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(components),
                data_model: None,
            })
            .await
            .unwrap();

        let html = renderer.render_surface_html("s1").expect("html");
        assert!(
            html.contains("HELLO_MODAL"),
            "Modal content 组件应被渲染，got: {html}"
        );
    }

    #[tokio::test]
    async fn test_tabs_widget_populates_tab_children() {
        let mut renderer = WebRenderer::new();
        let components: Vec<Component> = vec![
            Component::from_value(json!({
                "id":"root","component":"Tabs",
                "tabs":[{"title":"First","child":"a"},{"title":"Second","child":"b"}]
            }))
            .unwrap(),
            Component::from_value(json!({"id":"a","component":"Text","text":"TAB_A_TEXT"}))
                .unwrap(),
            Component::from_value(json!({"id":"b","component":"Text","text":"TAB_B_TEXT"}))
                .unwrap(),
        ];
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(components),
                data_model: None,
            })
            .await
            .unwrap();

        let html = renderer.render_surface_html("s1").expect("html");
        assert!(
            html.contains("TAB_A_TEXT"),
            "tab 0 内容应被渲染，got: {html}"
        );
        assert!(
            html.contains("TAB_B_TEXT"),
            "tab 1 内容应被渲染，got: {html}"
        );
    }

    #[tokio::test]
    async fn test_choicepicker_renders_spec_object_options_with_bound_selection() {
        // 规范 basic catalog 形态：{label, value} 对象 options + value 绑定
        let mut renderer = WebRenderer::new();
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![Component::from_value(json!({
                    "id":"root","component":"ChoicePicker",
                    "options":[{"label":"Email","value":"email"},{"label":"SMS","value":"sms"}],
                    "value":{"path":"/contact/preference"}
                }))
                .unwrap()]),
                data_model: Some(json!({"contact":{"preference":["email"]}})),
            })
            .await
            .unwrap();

        let html = renderer.render_surface_html("s1").expect("html");
        assert!(
            html.contains("<option value=\"email\" selected>Email</option>"),
            "选中项应按 value 匹配并展示 label，got: {html}"
        );
        assert!(
            html.contains("<option value=\"sms\">SMS</option>"),
            "未选中项应展示 label，got: {html}"
        );
    }

    #[tokio::test]
    async fn test_modal_without_content_does_not_render_trigger_as_body() {
        // Modal 只有 trigger 无 content：按位置取第一个子 widget 会把
        // trigger 误当 content 塞进 modal body——必须按 id 匹配
        let mut renderer = WebRenderer::new();
        let components: Vec<Component> = vec![
            Component::from_value(json!({
                "id":"root","component":"Modal","title":"T","trigger":"btn"
            }))
            .unwrap(),
            Component::from_value(json!({
                "id":"btn","component":"Button","label":"TRIGGER_LABEL"
            }))
            .unwrap(),
        ];
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(components),
                data_model: None,
            })
            .await
            .unwrap();

        let html = renderer.render_surface_html("s1").expect("html");
        // trigger 不应出现在 modal body 中
        let body_start = html.find("a2ui-modal-body").expect("modal body present");
        assert!(
            !html[body_start..].contains("TRIGGER_LABEL"),
            "trigger 不应被当作 modal content 渲染，got: {html}"
        );
    }

    #[tokio::test]
    async fn test_dynamic_string_props_render_from_data_model() {
        let mut renderer = WebRenderer::new();
        let components: Vec<Component> = vec![
            Component::from_value(json!({
                "id": "root",
                "component": "Column",
                "children": ["title", "text_field", "checkbox", "slider", "icon"]
            }))
            .unwrap(),
            Component::from_value(json!({
                "id": "title",
                "component": "Text",
                "text": {"path": "/title"}
            }))
            .unwrap(),
            Component::from_value(json!({
                "id": "text_field",
                "component": "TextField",
                "value": {"path": "/form/username"},
                "placeholder": {"path": "/form/placeholder"}
            }))
            .unwrap(),
            Component::from_value(json!({
                "id": "checkbox",
                "component": "CheckBox",
                "label": {"path": "/remember"},
                "value": {"path": "/rememberChecked"}
            }))
            .unwrap(),
            Component::from_value(json!({
                "id": "slider",
                "component": "Slider",
                "value": {"path": "/volume"},
                "min": 0,
                "max": 100
            }))
            .unwrap(),
            Component::from_value(json!({
                "id": "icon",
                "component": "Icon",
                "name": {"path": "/missing_icon"}
            }))
            .unwrap(),
        ];

        renderer
            .create_surface(CreateSurface {
                surface_id: "dynamic".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(components),
                data_model: Some(json!({
                    "title": "Welcome",
                    "remember": "记住密码",
                    "rememberChecked": true,
                    "volume": 42.0,
                    "form": {
                        "username": "Alice",
                        "placeholder": "请输入用户名"
                    }
                })),
            })
            .await
            .unwrap();

        let html = renderer.render_surface_html("dynamic").unwrap();
        assert!(html.contains("Welcome"));
        assert!(html.contains("Alice"));
        assert!(html.contains("请输入用户名"));
        assert!(html.contains("记住密码"));
        assert!(html.contains("checked"));
        assert!(html.contains("value=\"42\""));
        assert!(html.contains("{path:/missing_icon}"));
    }

    #[tokio::test]
    async fn test_web_renderer_applies_shared_style_contract() {
        let mut renderer = WebRenderer::new();
        let style = json!({
            "fontSize": 18,
            "strong": true,
            "color": "#112233",
            "fill": "#44556680",
            "padding": 9,
            "spacing": {"x": 7, "y": 11},
            "radius": 5
        });
        let components: Vec<Component> = vec![
            Component::from_value(json!({
                "id": "root",
                "component": "Column",
                "children": ["title", "icon", "row", "card", "list"],
                "style": style.clone()
            }))
            .unwrap(),
            Component::from_value(json!({
                "id": "title",
                "component": "Text",
                "text": "Styled",
                "style": style.clone()
            }))
            .unwrap(),
            Component::from_value(json!({
                "id": "icon",
                "component": "Icon",
                "name": "star",
                "style": style.clone()
            }))
            .unwrap(),
            Component::from_value(json!({
                "id": "row",
                "component": "Row",
                "children": ["image"],
                "style": style.clone()
            }))
            .unwrap(),
            Component::from_value(json!({
                "id": "image",
                "component": "Image",
                "url": "https://example.com/image.png",
                "style": style.clone()
            }))
            .unwrap(),
            Component::from_value(json!({
                "id": "card",
                "component": "Card",
                "child": "card_text",
                "style": style.clone()
            }))
            .unwrap(),
            Component::from_value(json!({
                "id": "card_text",
                "component": "Text",
                "text": "Card text"
            }))
            .unwrap(),
            Component::from_value(json!({
                "id": "list",
                "component": "List",
                "children": ["list_text"],
                "style": style
            }))
            .unwrap(),
            Component::from_value(json!({
                "id": "list_text",
                "component": "Text",
                "text": "List text"
            }))
            .unwrap(),
        ];

        renderer
            .create_surface(CreateSurface {
                surface_id: "styled".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(components),
                data_model: None,
            })
            .await
            .unwrap();

        let html = renderer.render_surface_html("styled").unwrap();

        assert!(html.contains("font-size:18px"));
        assert!(html.contains("font-weight:700"));
        assert!(html.contains("color:#112233"));
        assert!(html.contains("background-color:rgba(68,85,102,0.502)"));
        assert!(html.contains("padding:9px"));
        assert!(html.contains("border-radius:5px"));
        assert!(html.contains("gap:11px 7px"));
    }

    #[tokio::test]
    async fn test_render_all_html() {
        let mut renderer = WebRenderer::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
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

        let full_html = renderer.render_all_html();
        assert!(full_html.contains("<!DOCTYPE html>"));
        assert!(full_html.contains("Hello"));
        assert!(full_html.contains("a2ui-surface"));
    }

    #[tokio::test]
    async fn test_dependency_graph_populated() {
        let mut renderer = WebRenderer::new();
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
                data_model: None,
            })
            .await
            .unwrap();

        let graph = renderer.dependency_graph();
        let deps = graph
            .get_dependencies(&ComponentId::new("name_label").unwrap())
            .unwrap();
        assert!(deps.contains("/user/name"));
    }

    #[tokio::test]
    async fn test_incremental_render_marks_dirty() {
        let mut renderer = WebRenderer::new();
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
    async fn test_render_all_html_with_multiple_surfaces() {
        let mut renderer = WebRenderer::new();
        let comp1 = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Surface 1".into()),
        );
        let comp2 = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Surface 2".into()),
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp1]),
                data_model: None,
            })
            .await
            .unwrap();
        renderer
            .create_surface(CreateSurface {
                surface_id: "s2".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![comp2]),
                data_model: None,
            })
            .await
            .unwrap();

        let html = renderer.render_all_html();
        assert!(html.contains("Surface 1"));
        assert!(html.contains("Surface 2"));
    }

    // --- Surface/component limit tests ---

    #[tokio::test]
    async fn test_surface_limit_delegates_to_core_lru() {
        // 旧断言（第 101 个 createSurface 报错）→ 新语义（RendererCore）：
        // 满额时创建新 surface 先经 LRU 驱逐最旧者，创建成功而非报错
        let mut renderer = WebRenderer::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );

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
        let mut renderer = WebRenderer::new();
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
    }

    #[tokio::test]
    async fn test_component_limit_allows_under_max() {
        let mut renderer = WebRenderer::new();
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

    // --- CustomComponentRegistry tests ---

    #[test]
    fn test_custom_component_registry() {
        let mut renderer = WebRenderer::new();
        renderer
            .register_custom_component(a2ui_renderer::CustomComponentDef::new("MyChart"))
            .unwrap();
        assert!(renderer.core.custom_registry().is_registered("MyChart"));
    }

    #[test]
    fn test_custom_component_registry_duplicate_fails() {
        let mut renderer = WebRenderer::new();
        renderer
            .register_custom_component(a2ui_renderer::CustomComponentDef::new("MyChart"))
            .unwrap();
        let result =
            renderer.register_custom_component(a2ui_renderer::CustomComponentDef::new("MyChart"));
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_unknown_vs_custom_placeholder_in_html() {
        let mut renderer = WebRenderer::new();

        // 先注册自定义组件
        renderer
            .register_custom_component(a2ui_renderer::CustomComponentDef::new("MyChart"))
            .unwrap();

        // 未知组件（不是 root，而是 root 的子组件）
        let unknown_comp: Component =
            Component::from_json(r#"{"id":"u1","component":"UnknownType"}"#).unwrap();
        let root_unknown = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("u1").unwrap()],
        );

        // 自定义组件（已注册）
        let custom_comp: Component =
            Component::from_json(r#"{"id":"c1","component":"MyChart"}"#).unwrap();
        let root_custom = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("c1").unwrap()],
        );

        // 创建 surface 测试未知组件
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![root_unknown, unknown_comp]),
                data_model: None,
            })
            .await
            .unwrap();

        // 创建 surface 测试自定义组件
        renderer
            .create_surface(CreateSurface {
                surface_id: "s2".into(),
                catalog_id: "a2ui://catalogs/basic/v1".into(),
                surface_properties: None,
                send_data_model: false,
                components: Some(vec![root_custom, custom_comp]),
                data_model: None,
            })
            .await
            .unwrap();

        // 未知组件 → "unknown component type"
        let html1 = renderer.render_surface_html("s1").unwrap();
        assert!(html1.contains("unknown component type"));

        // 注册过的自定义组件 → "custom component"
        let html2 = renderer.render_surface_html("s2").unwrap();
        assert!(html2.contains("custom component"));
    }

    // --- callableFrom enforcement tests ---

    #[tokio::test]
    async fn test_call_function_client_only_allowed() {
        let mut renderer = WebRenderer::new();
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
        let mut renderer = WebRenderer::new();
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
    }
}
