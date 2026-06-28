use crate::error::RenderResult;
use a2ui_core::prelude::*;
use std::collections::HashMap;

/// Catalog 注册表：管理已加载的 Catalog
#[derive(Debug, Clone, Default)]
pub struct CatalogRegistry {
    catalogs: HashMap<String, Catalog>,
    _basic_catalog_id: Option<String>,
}

impl CatalogRegistry {
    /// 创建新的空注册表
    pub fn new() -> Self {
        Self::default()
    }

    /// 创建注册表并自动加载 Basic Catalog
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        if let Ok(catalog) = a2ui_core::load_basic_catalog() {
            let _ = registry.register(catalog);
        }
        registry
    }

    /// 注册 Catalog
    pub fn register(&mut self, catalog: Catalog) -> RenderResult<()> {
        let id = catalog.catalog_id().to_string();
        self.catalogs.insert(id.clone(), catalog);
        Ok(())
    }

    /// 获取 Catalog
    pub fn get(&self, catalog_id: &str) -> Option<&Catalog> {
        self.catalogs.get(catalog_id)
    }

    /// 获取可变 Catalog
    pub fn get_mut(&mut self, catalog_id: &str) -> Option<&mut Catalog> {
        self.catalogs.get_mut(catalog_id)
    }

    /// 获取所有已注册的 Catalog ID
    pub fn registered_ids(&self) -> Vec<&String> {
        self.catalogs.keys().collect()
    }

    /// 检查 Catalog 是否已注册
    pub fn has_catalog(&self, catalog_id: &str) -> bool {
        self.catalogs.contains_key(catalog_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_register_and_get() {
        let mut registry = CatalogRegistry::new();
        let catalog = Catalog::new("basic".to_string());
        registry.register(catalog).unwrap();

        assert!(registry.has_catalog("basic"));
        assert!(registry.get("basic").is_some());
        assert!(registry.get("unknown").is_none());
    }

    #[test]
    fn test_registered_ids() {
        let mut registry = CatalogRegistry::new();
        registry.register(Catalog::new("c1".to_string())).unwrap();
        registry.register(Catalog::new("c2".to_string())).unwrap();

        let ids = registry.registered_ids();
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn test_get_mut() {
        let mut registry = CatalogRegistry::new();
        let mut catalog = Catalog::new("basic".to_string());
        catalog.add_component("Text", json!({"type": "object"}));
        registry.register(catalog).unwrap();

        if let Some(c) = registry.get_mut("basic") {
            c.add_component("Button", json!({"type": "object"}));
        }

        assert!(registry.get("basic").unwrap().has_component("Button"));
    }

    #[test]
    fn test_empty_registry() {
        let registry = CatalogRegistry::new();
        assert!(registry.get("any").is_none());
        assert!(registry.registered_ids().is_empty());
    }

    #[test]
    fn test_with_defaults_loads_basic_catalog() {
        let registry = CatalogRegistry::with_defaults();
        assert!(registry.has_catalog("a2ui://catalogs/basic/v1"));
        // Verify it has the 18 standard components
        let catalog = registry.get("a2ui://catalogs/basic/v1").unwrap();
        assert!(catalog.has_component("Text"));
        assert!(catalog.has_component("Button"));
        assert!(catalog.has_component("Image"));
        assert!(catalog.has_function("formatString"));
    }
}
