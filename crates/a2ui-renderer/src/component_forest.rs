use crate::data_binding::DataBinding;
use crate::error::{RenderResult, RendererError};
use crate::function_dispatcher::FunctionDispatcher;
use crate::path_resolver::PathResolver;
use a2ui_core::prelude::*;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// 组件树节点
#[derive(Debug, Clone)]
pub struct ComponentTreeNode {
    pub component: Component,
    pub children: Vec<ComponentTreeNode>,
}

impl ComponentTreeNode {
    pub fn new(component: Component) -> Self {
        Self {
            component,
            children: Vec::new(),
        }
    }

    pub fn with_children(mut self, children: Vec<ComponentTreeNode>) -> Self {
        self.children = children;
        self
    }
}

/// 组件森林：管理所有 Surface 的组件树
/// 使用邻接表模型：flat list + ID map，延迟构建树
#[derive(Debug, Clone, Default)]
pub struct ComponentForest {
    surfaces: HashMap<String, ComponentSurface>,
}

/// 单个 Surface 的组件存储
#[derive(Debug, Clone)]
struct ComponentSurface {
    /// flat list → tree 的构建缓存
    tree: Option<ComponentTreeNode>,
    /// 所有组件的 flat map
    components: HashMap<ComponentId, Component>,
    /// root 组件 ID
    root: ComponentId,
}

impl ComponentForest {
    /// 创建新的空组件森林
    pub fn new() -> Self {
        Self::default()
    }

    /// 向指定 Surface 添加或更新组件
    pub fn upsert(&mut self, surface_id: &str, component: Component) -> RenderResult<()> {
        let surface = self
            .surfaces
            .entry(surface_id.to_string())
            .or_insert_with(|| ComponentSurface {
                tree: None,
                components: HashMap::new(),
                root: ComponentId::new("root").expect("'root' is a valid ComponentId"),
            });

        let comp_id = component.id().clone();
        let is_root = comp_id.as_str() == "root";

        if is_root {
            surface.root = comp_id.clone();
        }

        surface.components.insert(comp_id, component);
        surface.tree = None; // 失效缓存
        Ok(())
    }

    /// 获取指定 Surface 的组件
    pub fn get(&self, surface_id: &str, component_id: &ComponentId) -> Option<&Component> {
        self.surfaces.get(surface_id)?.components.get(component_id)
    }

    /// 获取指定 Surface 的 root 组件
    pub fn get_root(&self, surface_id: &str) -> Option<&Component> {
        let surface = self.surfaces.get(surface_id)?;
        surface.components.get(&surface.root)
    }

    /// 移除整个 Surface
    pub fn remove_surface(&mut self, surface_id: &str) -> RenderResult<()> {
        self.surfaces.remove(surface_id);
        Ok(())
    }

    /// 获取指定 Surface 的组件数量
    pub fn component_count(&self, surface_id: &str) -> usize {
        self.surfaces
            .get(surface_id)
            .map(|s| s.components.len())
            .unwrap_or(0)
    }

    /// 展开 ChildList::Object 模板：从 Data Model 读取数组，为每个项实例化模板组件
    ///
    /// 返回新创建的组件 ID 列表。父组件的 `children` 属性会从
    /// `{"template": "...", "path": "..."}` 更新为 `{"children": [id0, id1, ...]}`。
    pub fn expand_templates(
        &mut self,
        surface_id: &str,
        data_binding: &DataBinding,
        resolver: &PathResolver,
        dispatcher: &FunctionDispatcher,
    ) -> RenderResult<Vec<ComponentId>> {
        // 1. 收集需要展开的模板（避免持有借用同时修改 HashMap）
        let templates_to_expand: Vec<(ComponentId, String, String)> = {
            let surface = self.surfaces.get(surface_id).ok_or_else(|| {
                crate::error::RendererError::SurfaceNotFound(surface_id.to_string())
            })?;
            let mut result = Vec::new();
            for comp in surface.components.values() {
                if let Some(obj) = comp.properties().as_object() {
                    if let Some(children_val) = obj.get("children") {
                        if let Some(children_obj) = children_val.as_object() {
                            if let (Some(template_id), Some(path)) = (
                                children_obj.get("template").and_then(|v| v.as_str()),
                                children_obj.get("path").and_then(|v| v.as_str()),
                            ) {
                                result.push((
                                    comp.id().clone(),
                                    template_id.to_string(),
                                    path.to_string(),
                                ));
                            }
                        }
                    }
                }
            }
            result
        };

        let mut all_new_ids = Vec::new();

        // 2. 逐个展开模板
        for (parent_id, template_id_str, data_path) in &templates_to_expand {
            let new_ids = self.expand_one_template(
                surface_id,
                parent_id,
                template_id_str,
                data_path,
                data_binding,
                resolver,
                dispatcher,
            )?;
            all_new_ids.extend(new_ids);
        }

        Ok(all_new_ids)
    }

    /// 展开单个 ChildList::Object 模板
    fn expand_one_template(
        &mut self,
        surface_id: &str,
        parent_id: &ComponentId,
        template_id_str: &str,
        data_path: &str,
        data_binding: &DataBinding,
        resolver: &PathResolver,
        dispatcher: &FunctionDispatcher,
    ) -> RenderResult<Vec<ComponentId>> {
        // 读取 Data Model 中的数组（使用 DataBinding::get 而非 Value::get，前者走 JSON Pointer）
        let array = data_binding
            .get(data_path)
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                crate::error::RendererError::BindingError(format!(
                    "template data path not found: {}",
                    data_path
                ))
            })?;

        // 获取模板组件（用于验证模板存在）
        let template_id = ComponentId::new(template_id_str)?;

        let mut expanded_ids = Vec::new();

        for (index, _item) in array.iter().enumerate() {
            // 创建带集合作用域的解析器（克隆已有 resolver 以复用 DataModel）
            let mut scope_resolver = resolver.clone();
            scope_resolver.enter_collection(data_path, index);

            let suffix = index.to_string();

            // 递归克隆并解析模板及其整个子树
            let new_id = self.clone_and_resolve_subtree(
                surface_id,
                &template_id,
                &suffix,
                &scope_resolver,
                dispatcher,
            )?;
            expanded_ids.push(new_id);
        }

        // 3. 更新父组件的 children 属性
        if !expanded_ids.is_empty() {
            let parent_comp = {
                let surface = self.surfaces.get(surface_id).ok_or_else(|| {
                    crate::error::RendererError::SurfaceNotFound(surface_id.to_string())
                })?;
                surface.components.get(parent_id).cloned().ok_or_else(|| {
                    crate::error::RendererError::ComponentNotFound(parent_id.clone())
                })?
            };

            // 通过 JSON 序列化/反序列化更新父组件（字段为 private）
            // 将 children 从 template 对象替换为展开后的 ID 数组
            let mut parent_json = serde_json::to_value(&parent_comp).map_err(|e| {
                crate::error::RendererError::CoreError(a2ui_core::A2uiError::Deserialization(e))
            })?;
            if let Some(obj) = parent_json.as_object_mut() {
                let ids: Vec<Value> = expanded_ids
                    .iter()
                    .map(|id| Value::String(id.as_str().to_string()))
                    .collect();
                obj.insert("children".to_string(), Value::Array(ids));
            }
            let new_parent: Component = serde_json::from_value(parent_json).map_err(|e| {
                crate::error::RendererError::CoreError(a2ui_core::A2uiError::Deserialization(e))
            })?;
            self.upsert(surface_id, new_parent)?;
        }

        Ok(expanded_ids)
    }

    /// 递归克隆组件子树，解析所有 DynamicValue 并重命名 ID
    ///
    /// 返回新创建的根组件 ID。
    /// 模板展开时，每个数组项调用一次，对整个子树做深拷贝 + 属性解析。
    fn clone_and_resolve_subtree(
        &mut self,
        surface_id: &str,
        comp_id: &ComponentId,
        suffix: &str,
        scope_resolver: &PathResolver,
        dispatcher: &FunctionDispatcher,
    ) -> RenderResult<ComponentId> {
        let mut visited = HashSet::new();
        self.clone_and_resolve_subtree_inner(
            surface_id, comp_id, suffix, scope_resolver, dispatcher, 0, &mut visited,
        )
    }

    /// clone_and_resolve_subtree 的内部递归实现
    /// 包含循环检测和深度限制
    fn clone_and_resolve_subtree_inner(
        &mut self,
        surface_id: &str,
        comp_id: &ComponentId,
        suffix: &str,
        scope_resolver: &PathResolver,
        dispatcher: &FunctionDispatcher,
        depth: usize,
        visited: &mut HashSet<ComponentId>,
    ) -> RenderResult<ComponentId> {
        // 深度限制
        if depth >= crate::error::MAX_TREE_DEPTH {
            return Err(RendererError::tree_too_deep(depth));
        }

        // 循环检测
        if !visited.insert(comp_id.clone()) {
            return Err(RendererError::BindingError(format!(
                "circular component reference detected: {}",
                comp_id.as_str()
            )));
        }

        // 获取原始组件
        let original = {
            let surface = self.surfaces.get(surface_id).ok_or_else(|| {
                RendererError::SurfaceNotFound(surface_id.to_string())
            })?;
            surface.components.get(comp_id).cloned().ok_or_else(|| {
                RendererError::ComponentNotFound(comp_id.clone())
            })?
        };

        let new_id_str = format!("{}_{}", comp_id.as_str(), suffix);

        // 序列化为 JSON，解析动态值，设置新 ID
        let mut comp_json = serde_json::to_value(&original).map_err(|e| {
            RendererError::CoreError(a2ui_core::A2uiError::Deserialization(e))
        })?;

        if let Some(obj) = comp_json.as_object_mut() {
            obj.remove("id");
            obj.insert("id".to_string(), Value::String(new_id_str.clone()));

            // 递归处理 children 数组中的每个子组件引用
            if let Some(children_val) = obj.get_mut("children") {
                if let Some(children_arr) = children_val.as_array() {
                    let mut new_children = Vec::new();
                    for id_val in children_arr {
                        let id_str = match id_val.as_str() {
                            Some(s) => s,
                            None => {
                                tracing::warn!("non-string child ID in template: {:?}", id_val);
                                continue;
                            }
                        };
                        let child_id = match ComponentId::new(id_str) {
                            Ok(id) => id,
                            Err(e) => {
                                tracing::warn!("invalid child ID '{}' in template: {}", id_str, e);
                                continue;
                            }
                        };
                        let new_child_id = self.clone_and_resolve_subtree_inner(
                            surface_id, &child_id, suffix, scope_resolver, dispatcher,
                            depth + 1, visited,
                        )?;
                        new_children.push(Value::String(new_child_id.as_str().to_string()));
                    }
                    *children_val = Value::Array(new_children);
                }
            }

            // 处理单个 child 引用（Button、Card 等）
            if let Some(child_val) = obj.get("child").and_then(|v| v.as_str()) {
                if let Ok(child_id) = ComponentId::new(child_val) {
                    let new_child_id = self.clone_and_resolve_subtree_inner(
                        surface_id, &child_id, suffix, scope_resolver, dispatcher,
                        depth + 1, visited,
                    )?;
                    obj.insert(
                        "child".to_string(),
                        Value::String(new_child_id.as_str().to_string()),
                    );
                }
            }

            // 解析每个属性中的 DynamicValue
            for (key, val) in obj.iter_mut() {
                if key == "component" || key == "id" || key == "children" || key == "child" {
                    continue;
                }
                *val = Self::resolve_value_json(val, scope_resolver, dispatcher);
            }
        }

        let new_comp: Component = serde_json::from_value(comp_json).map_err(|e| {
            RendererError::CoreError(a2ui_core::A2uiError::Deserialization(e))
        })?;
        let new_id = ComponentId::new(&new_id_str)?;
        self.upsert(surface_id, new_comp)?;
        Ok(new_id)
    }

    /// 递归解析 JSON 中的 DynamicValue 表达式
    fn resolve_value_json(
        value: &Value,
        resolver: &PathResolver,
        dispatcher: &FunctionDispatcher,
    ) -> Value {
        match value {
            Value::Object(map) => {
                // 检测 DynamicValue::Path: {"path": "..."}
                if let Some(Value::String(p)) = map.get("path") {
                    return resolver.resolve(p).unwrap_or(Value::Null);
                }
                // 检测 DynamicValue::FunctionCall: {"call": "...", "args": {...}}
                if let Some(Value::String(call)) = map.get("call") {
                    if call == "@index" {
                        return resolver.resolve("@index").unwrap_or(Value::Null);
                    }
                    if let Some(args) = map.get("args") {
                        // 模板展开发生在客户端，必须以 ClientOnly 身份调用
                        if let Ok(result) = dispatcher.dispatch(
                            call,
                            args.clone(),
                            crate::function_dispatcher::CallableFrom::ClientOnly,
                        ) {
                            return result;
                        }
                    }
                    return Value::Null;
                }
                // 递归处理嵌套对象
                let mut result = serde_json::Map::new();
                for (k, v) in map {
                    result.insert(k.clone(), Self::resolve_value_json(v, resolver, dispatcher));
                }
                Value::Object(result)
            }
            Value::Array(arr) => Value::Array(
                arr.iter()
                    .map(|v| Self::resolve_value_json(v, resolver, dispatcher))
                    .collect(),
            ),
            _ => value.clone(),
        }
    }

    /// 构建组件树（含循环检测和深度限制）
    pub fn build_tree(&self, surface_id: &str) -> RenderResult<ComponentTreeNode> {
        let surface = self
            .surfaces
            .get(surface_id)
            .ok_or_else(|| crate::error::RendererError::SurfaceNotFound(surface_id.to_string()))?;

        let root_id = &surface.root;
        let root_comp = surface
            .components
            .get(root_id)
            .ok_or_else(|| crate::error::RendererError::ComponentNotFound(root_id.clone()))?;

        let mut visited = HashSet::new();
        self.build_node_with_depth(root_comp, &surface.components, 0, &mut visited)
    }

    /// 递归构建节点，包含深度限制和循环检测
    fn build_node_with_depth(
        &self,
        component: &Component,
        all: &HashMap<ComponentId, Component>,
        depth: usize,
        visited: &mut HashSet<ComponentId>,
    ) -> RenderResult<ComponentTreeNode> {
        // 深度限制
        if depth >= crate::error::MAX_TREE_DEPTH {
            return Err(RendererError::tree_too_deep(depth));
        }

        // 循环检测
        if !visited.insert(component.id().clone()) {
            return Err(RendererError::BindingError(format!(
                "circular component reference detected: {}",
                component.id().as_str()
            )));
        }

        let mut node = ComponentTreeNode::new(component.clone());
        let props = component.properties();

        // 尝试从 children 属性中获取子组件 ID（数组格式：["id1", "id2"]）
        if let Some(ids) = props.get("children").and_then(|v| v.as_array()) {
            for id_val in ids {
                if let Some(id_str) = id_val.as_str() {
                    if let Ok(child_id) = ComponentId::new(id_str) {
                        if let Some(child_comp) = all.get(&child_id) {
                            let child_node = self.build_node_with_depth(
                                child_comp, all, depth + 1, visited,
                            )?;
                            node.children.push(child_node);
                        }
                    }
                }
            }
        }

        // 处理 Button/Card 的 child 属性
        if let Some(child_str) = props.get("child").and_then(|v| v.as_str()) {
            if let Ok(child_id) = ComponentId::new(child_str) {
                if let Some(child_comp) = all.get(&child_id) {
                    let child_node = self.build_node_with_depth(
                        child_comp, all, depth + 1, visited,
                    )?;
                    node.children.push(child_node);
                }
            }
        }

        Ok(node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_tree_node_new() {
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        let node = ComponentTreeNode::new(comp);
        assert!(node.children.is_empty());
    }

    #[test]
    fn test_component_forest_new() {
        let forest = ComponentForest::new();
        assert!(forest.surfaces.is_empty());
    }

    #[test]
    fn test_upsert_and_get() {
        let mut forest = ComponentForest::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        forest.upsert("s1", comp).unwrap();

        let retrieved = forest.get("s1", &ComponentId::new("root").unwrap());
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_get_root() {
        let mut forest = ComponentForest::new();
        let root = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        let child = Component::text(
            ComponentId::new("child").unwrap(),
            DynamicValue::Literal("World".to_string()),
        );
        forest.upsert("s1", root).unwrap();
        forest.upsert("s1", child).unwrap();

        let root_comp = forest.get_root("s1");
        assert!(root_comp.is_some());
        assert_eq!(root_comp.unwrap().id().as_str(), "root");
    }

    #[test]
    fn test_build_tree() {
        let mut forest = ComponentForest::new();
        let root = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("child1").unwrap()],
        );
        let child1 = Component::text(
            ComponentId::new("child1").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        forest.upsert("s1", root).unwrap();
        forest.upsert("s1", child1).unwrap();

        let tree = forest.build_tree("s1").unwrap();
        assert_eq!(tree.component.id().as_str(), "root");
        assert_eq!(tree.children.len(), 1);
    }

    #[test]
    fn test_remove_surface() {
        let mut forest = ComponentForest::new();
        let comp = Component::text(
            ComponentId::new("root").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        forest.upsert("s1", comp).unwrap();
        assert!(forest
            .get("s1", &ComponentId::new("root").unwrap())
            .is_some());

        forest.remove_surface("s1").unwrap();
        assert!(forest
            .get("s1", &ComponentId::new("root").unwrap())
            .is_none());
    }

    // --- P1-1: @index scope system — template expansion ---

    #[test]
    fn test_expand_templates_object_mode() {
        let mut forest = ComponentForest::new();
        let dm = DataModel::new(json!({"items": [{"name": "a"}, {"name": "b"}]}));
        let binding = DataBinding::new(dm);
        let resolver = PathResolver::new(DataModel::new(binding.as_value().clone()));

        // Template: Text with relative path "name"
        let template = Component::text(
            ComponentId::new("item_tmpl").unwrap(),
            DynamicValue::Path {
                path: "name".into(),
            },
        );

        // Parent with ChildList::Object: template + data path
        let parent: Component =
            serde_json::from_value(json!({"component": "Column", "id": "list", "children": {"template": "item_tmpl", "path": "/items"}}))
                .unwrap();

        forest.upsert("s1", parent).unwrap();
        forest.upsert("s1", template).unwrap();

        let new_ids = forest
            .expand_templates("s1", &binding, &resolver, &FunctionDispatcher::new())
            .unwrap();

        assert_eq!(new_ids.len(), 2);

        // Verify resolved values
        let comp0 = forest
            .get("s1", &ComponentId::new("item_tmpl_0").unwrap())
            .unwrap();
        assert_eq!(comp0.properties().get("text"), Some(&json!("a")));

        let comp1 = forest
            .get("s1", &ComponentId::new("item_tmpl_1").unwrap())
            .unwrap();
        assert_eq!(comp1.properties().get("text"), Some(&json!("b")));
    }

    #[test]
    fn test_expand_templates_with_at_index() {
        let mut forest = ComponentForest::new();
        let dm = DataModel::new(json!({"items": [{"name": "x"}, {"name": "y"}, {"name": "z"}]}));
        let binding = DataBinding::new(dm);
        let resolver = PathResolver::new(DataModel::new(binding.as_value().clone()));

        // Template uses @index function call
        let template = Component::text(
            ComponentId::new("idx_tmpl").unwrap(),
            DynamicValue::FunctionCall {
                call: "@index".into(),
                args: json!({}),
            },
        );

        let parent: Component =
            serde_json::from_value(json!({"component": "Column", "id": "list", "children": {"template": "idx_tmpl", "path": "/items"}}))
                .unwrap();

        forest.upsert("s1", parent).unwrap();
        forest.upsert("s1", template).unwrap();

        let new_ids = forest
            .expand_templates("s1", &binding, &resolver, &FunctionDispatcher::new())
            .unwrap();

        assert_eq!(new_ids.len(), 3);
        assert_eq!(
            forest
                .get("s1", &ComponentId::new("idx_tmpl_0").unwrap())
                .unwrap()
                .properties()
                .get("text"),
            Some(&json!(0))
        );
        assert_eq!(
            forest
                .get("s1", &ComponentId::new("idx_tmpl_1").unwrap())
                .unwrap()
                .properties()
                .get("text"),
            Some(&json!(1))
        );
        assert_eq!(
            forest
                .get("s1", &ComponentId::new("idx_tmpl_2").unwrap())
                .unwrap()
                .properties()
                .get("text"),
            Some(&json!(2))
        );
    }

    #[test]
    fn test_expand_templates_empty_array() {
        let mut forest = ComponentForest::new();
        let dm = DataModel::new(json!({"items": []}));
        let binding = DataBinding::new(dm);
        let resolver = PathResolver::new(DataModel::new(binding.as_value().clone()));

        let template = Component::text(
            ComponentId::new("item_tmpl").unwrap(),
            DynamicValue::Path {
                path: "name".into(),
            },
        );
        let parent: Component =
            serde_json::from_value(json!({"component": "Column", "id": "list", "children": {"template": "item_tmpl", "path": "/items"}}))
                .unwrap();

        forest.upsert("s1", parent).unwrap();
        forest.upsert("s1", template).unwrap();

        let new_ids = forest
            .expand_templates("s1", &binding, &resolver, &FunctionDispatcher::new())
            .unwrap();

        assert!(new_ids.is_empty());
    }

    #[test]
    fn test_expand_templates_missing_data_path() {
        let mut forest = ComponentForest::new();
        let dm = DataModel::new(json!({"other": [1, 2]}));
        let binding = DataBinding::new(dm);
        let resolver = PathResolver::new(DataModel::new(binding.as_value().clone()));

        let template = Component::text(
            ComponentId::new("item_tmpl").unwrap(),
            DynamicValue::Path {
                path: "name".into(),
            },
        );
        let parent: Component =
            serde_json::from_value(json!({"component": "Column", "id": "list", "children": {"template": "item_tmpl", "path": "/missing"}}))
                .unwrap();

        forest.upsert("s1", parent).unwrap();
        forest.upsert("s1", template).unwrap();

        let result = forest.expand_templates("s1", &binding, &resolver, &FunctionDispatcher::new());
        assert!(result.is_err());
    }
}
