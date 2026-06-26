use a2ui_core::prelude::*;
use crate::error::RenderResult;
use std::collections::HashMap;

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

impl ComponentSurface {
    fn new() -> Self {
        Self {
            tree: None,
            components: HashMap::new(),
            root: ComponentId::new("_root_").expect("placeholder ComponentId must be valid"),
        }
    }
}

impl ComponentForest {
    /// 创建新的空组件森林
    pub fn new() -> Self {
        Self::default()
    }

    /// 向指定 Surface 添加或更新组件
    pub fn upsert(&mut self, surface_id: &str, component: Component) -> RenderResult<()> {
        let surface = self.surfaces.entry(surface_id.to_string()).or_insert_with(ComponentSurface::new);

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

    /// 构建组件树
    pub fn build_tree(&self, surface_id: &str) -> RenderResult<ComponentTreeNode> {
        let surface = self.surfaces.get(surface_id)
            .ok_or_else(|| A2uiError::SurfaceNotFound(surface_id.to_string()))?;

        let root_id = &surface.root;
        let root_comp = surface.components.get(root_id)
            .ok_or_else(|| A2uiError::ComponentNotFound(root_id.as_str().to_string()))?;

        self.build_node(root_comp, &surface.components)
            .ok_or_else(|| A2uiError::ComponentNotFound(root_id.as_str().to_string()))
            .map_err(Into::into)
    }

    /// 递归构建节点
    fn build_node(
        &self,
        component: &Component,
        all: &HashMap<ComponentId, Component>,
    ) -> Option<ComponentTreeNode> {
        let mut node = ComponentTreeNode::new(component.clone());
        let props = component.properties();

        // 尝试从 children 属性中获取子组件 ID
        if let Some(children_obj) = props.get("children") {
            if let Some(children_arr) = children_obj.get("children") {
                if let Some(ids) = children_arr.as_array() {
                    for id_val in ids {
                        if let Some(id_str) = id_val.as_str() {
                            if let Ok(child_id) = ComponentId::new(id_str) {
                                if let Some(child_comp) = all.get(&child_id) {
                                    if let Some(child_node) = self.build_node(child_comp, all) {
                                        node.children.push(child_node);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 处理 Button 的 child 属性
        if let Some(child_str) = props.get("child").and_then(|v| v.as_str()) {
            if let Ok(child_id) = ComponentId::new(child_str) {
                if let Some(child_comp) = all.get(&child_id) {
                    if let Some(child_node) = self.build_node(child_comp, all) {
                        node.children.push(child_node);
                    }
                }
            }
        }

        Some(node)
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
        assert!(forest.get("s1", &ComponentId::new("root").unwrap()).is_some());

        forest.remove_surface("s1").unwrap();
        assert!(forest.get("s1", &ComponentId::new("root").unwrap()).is_none());
    }
}
