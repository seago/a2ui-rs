use a2ui_core::prelude::*;
use std::collections::{HashMap, HashSet};

/// 组件依赖图
/// 记录每个组件依赖的 Data Model 路径集合，用于响应式重渲染
#[derive(Debug, Clone, Default)]
pub struct DependencyGraph {
    /// component_id → 依赖的路径集合
    dependencies: HashMap<ComponentId, HashSet<String>>,
    /// path → 依赖该路径的组件集合（反向索引）
    dependents: HashMap<String, HashSet<ComponentId>>,
}

impl DependencyGraph {
    /// 创建新的空依赖图
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册组件对路径的依赖
    pub fn register_dependency(&mut self, component_id: ComponentId, path: String) {
        // 正向：component → paths
        self.dependencies
            .entry(component_id.clone())
            .or_default()
            .insert(path.clone());
        // 反向：path → components
        self.dependents
            .entry(path)
            .or_default()
            .insert(component_id);
    }

    /// 获取依赖指定路径的所有组件
    pub fn dependents(&self, path: &str) -> Vec<&ComponentId> {
        self.dependents
            .get(path)
            .map(|set| set.iter().collect())
            .unwrap_or_default()
    }

    /// 当路径变更时，返回需要重渲染的组件列表
    pub fn on_data_change(&mut self, path: &str) -> Vec<ComponentId> {
        self.dependents
            .get(path)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// 获取组件的所有依赖路径
    pub fn get_dependencies(&self, component_id: &ComponentId) -> Option<&HashSet<String>> {
        self.dependencies.get(component_id)
    }

    /// 移除组件（清理依赖关系）
    pub fn remove_component(&mut self, component_id: &ComponentId) {
        if let Some(paths) = self.dependencies.remove(component_id) {
            for path in paths {
                if let Some(set) = self.dependents.get_mut(&path) {
                    set.remove(component_id);
                }
            }
        }
    }

    /// 清除所有依赖
    pub fn clear(&mut self) {
        self.dependencies.clear();
        self.dependents.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_dependency() {
        let mut graph = DependencyGraph::new();
        let comp = ComponentId::new("label").unwrap();
        graph.register_dependency(comp.clone(), "/user/name".to_string());

        assert!(graph
            .get_dependencies(&comp)
            .unwrap()
            .contains("/user/name"));
    }

    #[test]
    fn test_dependents() {
        let mut graph = DependencyGraph::new();
        let comp1 = ComponentId::new("label1").unwrap();
        let comp2 = ComponentId::new("label2").unwrap();
        graph.register_dependency(comp1.clone(), "/user/name".to_string());
        graph.register_dependency(comp2.clone(), "/user/name".to_string());

        let deps = graph.dependents("/user/name");
        assert_eq!(deps.len(), 2);
    }

    #[test]
    fn test_on_data_change() {
        let mut graph = DependencyGraph::new();
        let comp1 = ComponentId::new("label").unwrap();
        let comp2 = ComponentId::new("input").unwrap();
        graph.register_dependency(comp1.clone(), "/name".to_string());
        graph.register_dependency(comp2.clone(), "/email".to_string());

        // 只有 label 需要重渲染
        let affected = graph.on_data_change("/name");
        assert_eq!(affected.len(), 1);
        assert_eq!(affected[0].as_str(), "label");
    }

    #[test]
    fn test_remove_component() {
        let mut graph = DependencyGraph::new();
        let comp = ComponentId::new("label").unwrap();
        graph.register_dependency(comp.clone(), "/name".to_string());

        graph.remove_component(&comp);
        assert!(graph.get_dependencies(&comp).is_none());
        assert!(graph.dependents("/name").is_empty());
    }

    #[test]
    fn test_clear() {
        let mut graph = DependencyGraph::new();
        graph.register_dependency(ComponentId::new("a").unwrap(), "/x".to_string());
        graph.register_dependency(ComponentId::new("b").unwrap(), "/y".to_string());

        graph.clear();
        assert!(graph
            .get_dependencies(&ComponentId::new("a").unwrap())
            .is_none());
    }

    #[test]
    fn test_multiple_paths_per_component() {
        let mut graph = DependencyGraph::new();
        let comp = ComponentId::new("form").unwrap();
        graph.register_dependency(comp.clone(), "/name".to_string());
        graph.register_dependency(comp.clone(), "/email".to_string());

        let deps = graph.get_dependencies(&comp).unwrap();
        assert_eq!(deps.len(), 2);
    }
}
