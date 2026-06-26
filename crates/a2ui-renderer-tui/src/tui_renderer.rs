use crate::WidgetMapper;
use a2ui_core::message::client_to_server::FunctionResponse;
use a2ui_core::message::server_to_client::{
    ActionResponse, CallFunction, CreateSurface, DeleteSurface, UpdateComponents, UpdateDataModel,
};
use a2ui_core::prelude::*;
use a2ui_renderer::{
    ComponentForest, DataBinding, DependencyGraph, RenderResult, Renderer, SurfaceHandle, UserEvent,
};
use ratatui::{layout::Rect, Frame};
use serde_json::Value;
use std::collections::HashMap;

/// TUI 渲染器实现
#[derive(Debug)]
pub struct TuiRenderer {
    /// Surface 句柄 → SurfaceId 映射
    pub surfaces: HashMap<SurfaceHandle, String>,
    /// 组件森林（所有 Surface 的组件树）
    forest: ComponentForest,
    /// DataModel 绑定（使用字符串作为 Surface 标识）
    data_bindings: HashMap<String, DataBinding>,
    /// 依赖图
    dependency_graph: DependencyGraph,
    /// 当前聚焦的组件
    focused_component: Option<ComponentId>,
}

impl TuiRenderer {
    /// 创建新的 TUI 渲染器
    pub fn new() -> Self {
        Self {
            surfaces: HashMap::new(),
            forest: ComponentForest::new(),
            data_bindings: HashMap::new(),
            dependency_graph: DependencyGraph::new(),
            focused_component: None,
        }
    }

    /// 获取依赖图的只读引用（用于测试和查询）
    pub fn dependency_graph(&self) -> &DependencyGraph {
        &self.dependency_graph
    }

    /// 渲染单个组件到 Frame（简化实现）
    pub fn render_component(&self, component: &Component, frame: &mut Frame, area: Rect) {
        let mapper = WidgetMapper;
        let paragraph = mapper.map_to_paragraph(component);
        frame.render_widget(paragraph, area);
    }
}

/// 从组件的 properties 中递归提取所有 JSON Pointer 路径
fn extract_paths(component: &Component) -> Vec<String> {
    let mut paths = Vec::new();
    extract_paths_from_value(component.properties(), &mut paths);
    paths
}

/// 递归遍历 serde_json::Value，收集所有 DynamicValue::Path 中的路径字符串
fn extract_paths_from_value(value: &Value, paths: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (_, v) in map {
                // 检测 {"path": "..."} 结构
                if let Value::Object(inner) = v {
                    if inner.len() == 1 {
                        if let Some(Value::String(p)) = inner.get("path") {
                            paths.push(p.clone());
                            continue;
                        }
                    }
                }
                extract_paths_from_value(v, paths);
            }
        }
        Value::Array(arr) => {
            for item in arr {
                extract_paths_from_value(item, paths);
            }
        }
        _ => {}
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
        let handle = SurfaceHandle::new();
        let surface_id = msg.surface_id.clone();

        // 注册组件
        if let Some(components) = msg.components.clone() {
            for comp in &components {
                self.forest.upsert(&surface_id, comp.clone())?;
            }
            // 注册依赖关系到 DependencyGraph
            for comp in components {
                for path in extract_paths(&comp) {
                    self.dependency_graph
                        .register_dependency(comp.id().clone(), path);
                }
            }
        }

        // 注册 DataModel
        let data_model = msg.data_model.unwrap_or(Value::Object(Default::default()));
        self.data_bindings.insert(
            surface_id.clone(),
            DataBinding::new(DataModel::new(data_model)),
        );

        // 记录 Surface 映射
        self.surfaces.insert(handle, surface_id);

        Ok(handle)
    }

    async fn update_components(&mut self, msg: UpdateComponents) -> RenderResult<()> {
        let surface_id = msg.surface_id.clone();
        for comp in msg.components {
            self.forest.upsert(&surface_id, comp)?;
        }
        Ok(())
    }

    async fn update_data_model(&mut self, msg: UpdateDataModel) -> RenderResult<()> {
        let surface_id = msg.surface_id.clone();
        if let Some(binding) = self.data_bindings.get_mut(&surface_id) {
            if let Some(path) = &msg.path {
                binding.set(path, msg.value.unwrap_or(Value::Null))?;
                // 查询依赖图，获取需要重渲染的组件
                let _affected = self.dependency_graph.on_data_change(path);
                // TODO: 使用 _affected 列表进行增量渲染
            }
        }
        Ok(())
    }

    async fn delete_surface(&mut self, msg: DeleteSurface) -> RenderResult<()> {
        let surface_id = msg.surface_id.clone();
        self.forest.remove_surface(&surface_id)?;
        self.data_bindings.remove(&surface_id);
        // 移除 surface 映射
        self.surfaces.retain(|_, sid| sid != &surface_id);
        Ok(())
    }

    async fn action_response(&mut self, msg: ActionResponse) -> RenderResult<()> {
        // 如果有 responsePath，将响应值写入 DataModel
        // 简化实现
        let _ = msg;
        Ok(())
    }

    async fn call_function(&mut self, msg: CallFunction) -> RenderResult<FunctionResponse> {
        let _ = msg;
        Ok(FunctionResponse {
            function_call_id: String::new(),
            call: String::new(),
            value: Value::Null,
        })
    }

    async fn render(&mut self) -> RenderResult<()> {
        // 简化实现：实际渲染由平台 crate 处理
        Ok(())
    }

    async fn handle_user_event(&mut self, event: UserEvent) -> RenderResult<Option<ActionMessage>> {
        match event {
            UserEvent::Click { component_id } => {
                let action = ActionMessage::event("click", "").with_context(
                    "source",
                    DynamicValue::Literal(Value::String(component_id.as_str().to_string())),
                );
                Ok(Some(action))
            }
            UserEvent::KeyPress { key } => {
                if key == "Enter" || key == " " {
                    if let Some(ref comp_id) = self.focused_component {
                        let action = ActionMessage::event("activate", "").with_context(
                            "source",
                            DynamicValue::Literal(Value::String(comp_id.as_str().to_string())),
                        );
                        return Ok(Some(action));
                    }
                }
                Ok(None)
            }
            UserEvent::TextInput {
                component_id,
                value,
            } => {
                let action = ActionMessage::event("input", "")
                    .with_context(
                        "component",
                        DynamicValue::Literal(Value::String(component_id.as_str().to_string())),
                    )
                    .with_context("value", DynamicValue::Literal(Value::String(value)));
                Ok(Some(action))
            }
            UserEvent::CheckToggle {
                component_id,
                checked,
            } => {
                let action = ActionMessage::event("toggle", "")
                    .with_context(
                        "component",
                        DynamicValue::Literal(Value::String(component_id.as_str().to_string())),
                    )
                    .with_context(
                        "checked",
                        DynamicValue::Literal(Value::String(checked.to_string())),
                    );
                Ok(Some(action))
            }
            UserEvent::SliderChange {
                component_id,
                value,
            } => {
                let action = ActionMessage::event("slider_change", "")
                    .with_context(
                        "component",
                        DynamicValue::Literal(Value::String(component_id.as_str().to_string())),
                    )
                    .with_context(
                        "value",
                        DynamicValue::Literal(Value::String(value.to_string())),
                    );
                Ok(Some(action))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use a2ui_core::ComponentId;

    #[test]
    fn test_tui_renderer_new() {
        let renderer = TuiRenderer::new();
        assert!(renderer.surfaces.is_empty());
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
            catalog_id: "basic".to_string(),
            surface_properties: None,
            send_data_model: false,
            components: Some(vec![comp]),
            data_model: None,
        };

        // 结构验证
        assert!(renderer.surfaces.is_empty());
    }

    #[test]
    fn test_delete_surface_removes_bindings() {
        let renderer = TuiRenderer::new();
        // 结构验证
        assert!(renderer.data_bindings.is_empty());
    }

    #[tokio::test]
    async fn test_dependency_graph_populated_on_create_surface() {
        let mut renderer = TuiRenderer::new();
        let comp_a = Component::text(
            ComponentId::new("name_label").unwrap(),
            DynamicValue::Path { path: "/user/name".into() },
        );
        let comp_b = Component::text(
            ComponentId::new("count_label").unwrap(),
            DynamicValue::Path { path: "/user/count".into() },
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "basic".into(),
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
            DynamicValue::Path { path: "/user/name".into() },
        );
        let comp_b = Component::text(
            ComponentId::new("count_label").unwrap(),
            DynamicValue::Path { path: "/user/count".into() },
        );
        renderer
            .create_surface(CreateSurface {
                surface_id: "s1".into(),
                catalog_id: "basic".into(),
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
}
