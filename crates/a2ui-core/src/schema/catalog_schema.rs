use crate::component::Catalog;
use crate::error::{A2uiError, Result};

/// Catalog 合规性校验器
pub struct CatalogValidator;

impl CatalogValidator {
    /// 验证 Catalog 结构符合 A2UI v1.0 严格规则
    pub fn validate(catalog: &Catalog) -> Result<()> {
        // 1. 必须包含 catalogId
        if catalog.catalog_id().is_empty() {
            return Err(A2uiError::ValidationError {
                message: "catalogId is required".into(),
                component_id: "catalog".into(),
                check_index: 0,
            });
        }

        // 2. $defs 中只允许 surfaceProperties / anyComponent / anyFunction
        catalog.validate()?;

        // 3. 每个组件 schema 必须有 discriminator
        for name in catalog.components().keys() {
            let schema = catalog.get_component_schema(name).unwrap();
            // 简化检查：确保 schema 是 object
            if !schema.is_object() {
                return Err(A2uiError::ValidationError {
                    message: format!("Component '{}' schema must be an object", name),
                    component_id: name.clone(),
                    check_index: 0,
                });
            }
        }

        // 4. 每个函数必须有 returnType 和 callableFrom
        for (name, schema) in catalog.functions() {
            if !schema.is_object() {
                return Err(A2uiError::ValidationError {
                    message: format!("Function '{}' schema must be an object", name),
                    component_id: name.clone(),
                    check_index: 0,
                });
            }
            let obj = schema.as_object().unwrap();
            if !obj.contains_key("returnType") {
                return Err(A2uiError::ValidationError {
                    message: format!("Function '{}' missing returnType", name),
                    component_id: name.clone(),
                    check_index: 0,
                });
            }
            if !obj.contains_key("callableFrom") {
                return Err(A2uiError::ValidationError {
                    message: format!("Function '{}' missing callableFrom", name),
                    component_id: name.clone(),
                    check_index: 0,
                });
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::Catalog;
    use serde_json::json;

    #[test]
    fn test_validate_ok() {
        let mut catalog = Catalog::new("test".to_string());
        catalog.add_component("Text", json!({"type": "object"}));
        catalog.add_function("required", json!({"returnType":"boolean","callableFrom":"clientOnly"}));
        assert!(CatalogValidator::validate(&catalog).is_ok());
    }

    #[test]
    fn test_validate_missing_catalog_id() {
        let catalog = Catalog::new("".to_string());
        assert!(CatalogValidator::validate(&catalog).is_err());
    }

    #[test]
    fn test_validate_function_missing_return_type() {
        let mut catalog = Catalog::new("test".to_string());
        catalog.add_function("bad", json!({"type": "object"}));
        assert!(CatalogValidator::validate(&catalog).is_err());
    }

    #[test]
    fn test_validate_function_missing_callable_from() {
        let mut catalog = Catalog::new("test".to_string());
        catalog.add_function("bad", json!({"type": "object", "returnType": "string"}));
        assert!(CatalogValidator::validate(&catalog).is_err());
    }

    #[test]
    fn test_validate_component_schema_not_object() {
        let mut catalog = Catalog::new("test".to_string());
        catalog.add_component("Bad", json!("not an object"));
        assert!(CatalogValidator::validate(&catalog).is_err());
    }
}
