use thiserror::Error;

/// A2UI 统一错误类型
#[derive(Debug, Error)]
pub enum A2uiError {
    /// Surface 不存在（ID 无效或已销毁）
    #[error("Surface not found: {0}")]
    SurfaceNotFound(String),

    /// Surface ID 冲突（createSurface 时 ID 已存在）
    #[error("Surface ID already exists: {0}")]
    SurfaceIdConflict(String),

    /// 组件 ID 无效（不符合 UAX #31）
    #[error("Invalid component ID: {0}")]
    InvalidComponentId(String),

    /// 组件引用不存在（children 引用了未定义的组件）
    #[error("Component reference not found: {0}")]
    ComponentNotFound(String),

    /// Catalog 未加载或 ID 不匹配
    #[error("Catalog not found: {0}")]
    CatalogNotFound(String),

    /// 函数未注册或执行边界不允许
    #[error("Function not available: {0}")]
    FunctionNotAvailable(String),

    /// 状态机违规（在错误状态执行了非法操作）
    #[error("Invalid state transition: current={current:?}, attempted={attempted:?}")]
    InvalidStateTransition { current: String, attempted: String },

    /// JSON 反序列化失败
    #[error("Deserialization error: {0}")]
    Deserialization(#[from] serde_json::Error),

    /// 校验错误
    #[error("{message} (component: {component_id}, check: {check_index})")]
    ValidationError {
        message: String,
        component_id: String,
        check_index: usize,
    },

    /// JSON Pointer 路径无效
    #[error("invalid JSON Pointer: {0}")]
    InvalidPointer(String),

    /// 路径遍历攻击检测
    #[error("path traversal detected: {0}")]
    PathTraversal(String),
}

/// 便捷类型别名
pub type Result<T, E = A2uiError> = std::result::Result<T, E>;

impl A2uiError {
    /// 生产模式错误消息：移除敏感信息
    pub fn sanitize_core(&self) -> String {
        match self {
            A2uiError::SurfaceNotFound(_) => "Surface not found".to_string(),
            A2uiError::SurfaceIdConflict(_) => "Surface ID conflict".to_string(),
            A2uiError::InvalidComponentId(_) => "Invalid component ID".to_string(),
            A2uiError::ComponentNotFound(_) => "Component reference not found".to_string(),
            A2uiError::CatalogNotFound(_) => "Catalog not found".to_string(),
            A2uiError::FunctionNotAvailable(_) => "Function not available".to_string(),
            A2uiError::InvalidStateTransition { .. } => "Invalid state transition".to_string(),
            A2uiError::Deserialization(_) => "Deserialization error".to_string(),
            A2uiError::ValidationError { .. } => "Validation error".to_string(),
            A2uiError::InvalidPointer(_) => "Invalid path".to_string(),
            A2uiError::PathTraversal(_) => "Path traversal detected".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_surface_not_found_display() {
        let err = A2uiError::SurfaceNotFound("surface-1".into());
        assert_eq!(err.to_string(), "Surface not found: surface-1");
    }

    #[test]
    fn test_surface_id_conflict_display() {
        let err = A2uiError::SurfaceIdConflict("s1".into());
        assert_eq!(err.to_string(), "Surface ID already exists: s1");
    }

    #[test]
    fn test_invalid_component_id_display() {
        let err = A2uiError::InvalidComponentId("bad id!".into());
        assert_eq!(err.to_string(), "Invalid component ID: bad id!");
    }

    #[test]
    fn test_component_not_found_display() {
        let err = A2uiError::ComponentNotFound("btn".into());
        assert_eq!(err.to_string(), "Component reference not found: btn");
    }

    #[test]
    fn test_catalog_not_found_display() {
        let err = A2uiError::CatalogNotFound("https://example.com/catalog".into());
        assert_eq!(
            err.to_string(),
            "Catalog not found: https://example.com/catalog"
        );
    }

    #[test]
    fn test_function_not_available_display() {
        let err = A2uiError::FunctionNotAvailable("myFunc".into());
        assert_eq!(err.to_string(), "Function not available: myFunc");
    }

    #[test]
    fn test_deserialization_error_wraps_serde() {
        let serde_err: serde_json::Error =
            serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let err = A2uiError::Deserialization(serde_err);
        assert!(err.to_string().contains("Deserialization error"));
    }

    #[test]
    fn test_validation_error_display() {
        let err = A2uiError::ValidationError {
            message: "field required".into(),
            component_id: "email".into(),
            check_index: 0,
        };
        assert!(err.to_string().contains("field required"));
        assert!(err.to_string().contains("email"));
    }
}
