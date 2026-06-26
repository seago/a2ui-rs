use a2ui_core::state::{StateOperation, SurfaceState};
pub use a2ui_core::A2uiError;
use a2ui_core::ComponentId;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RendererError {
    #[error("Surface not found: {0}")]
    SurfaceNotFound(String),

    #[error("Surface ID already exists: {0}")]
    SurfaceIdConflict(String),

    #[error("Component reference not found: {0}")]
    ComponentNotFound(ComponentId),

    #[error("Catalog not found: {0}")]
    CatalogNotFound(String),

    #[error("Function not available: {0}")]
    FunctionNotAvailable(String),

    #[error("invalid function call: {0} (callableFrom enforcement)")]
    InvalidFunctionCall(String),

    #[error("Invalid state transition: current={current:?}, attempted={attempted:?}")]
    InvalidStateTransition {
        current: SurfaceState,
        attempted: StateOperation,
    },

    #[error("Core error: {0}")]
    CoreError(#[from] A2uiError),

    #[error("Binding error: {0}")]
    BindingError(String),

    #[error("Path resolution error: {0}")]
    PathError(String),

    #[error("surface limit exceeded: current={current}, max={max}")]
    SurfaceLimitExceeded { current: usize, max: usize },

    #[error("component limit exceeded for surface {surface_id}: current={current}, max={max}")]
    ComponentLimitExceeded {
        surface_id: String,
        current: usize,
        max: usize,
    },
}

pub type RenderResult<T> = Result<T, RendererError>;

impl RendererError {
    /// 生产模式错误消息：移除敏感信息（内部路径、内存地址、原始 payload）
    ///
    /// 开发/调试模式使用 `to_string()` 获取完整错误信息。
    /// 生产模式应使用 `sanitize()` 防止信息泄露。
    pub fn sanitize(&self) -> String {
        match self {
            RendererError::CoreError(e) => e.sanitize_core(),
            RendererError::SurfaceNotFound(_) => "Surface not found".to_string(),
            RendererError::SurfaceIdConflict(_) => "Surface ID conflict".to_string(),
            RendererError::ComponentNotFound(_) => "Component reference not found".to_string(),
            RendererError::CatalogNotFound(_) => "Catalog not found".to_string(),
            RendererError::FunctionNotAvailable(_) => "Function not available".to_string(),
            RendererError::InvalidFunctionCall(_) => "Invalid function call".to_string(),
            RendererError::InvalidStateTransition { .. } => "Invalid state transition".to_string(),
            RendererError::BindingError(_) => "Binding error".to_string(),
            RendererError::PathError(_) => "Path error".to_string(),
            RendererError::SurfaceLimitExceeded { .. } => "Surface limit exceeded".to_string(),
            RendererError::ComponentLimitExceeded { .. } => "Component limit exceeded".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use a2ui_core::{
        state::{StateOperation, SurfaceState},
        ComponentId,
    };

    #[test]
    fn test_surface_not_found() {
        let err = RendererError::SurfaceNotFound("s1".into());
        assert!(err.to_string().contains("s1"));
    }

    #[test]
    fn test_component_not_found() {
        let err = RendererError::ComponentNotFound(ComponentId::new("x").unwrap());
        assert!(err.to_string().contains("x"));
    }

    #[test]
    fn test_catalog_not_found() {
        let err = RendererError::CatalogNotFound("basic".into());
        assert!(err.to_string().contains("basic"));
    }

    #[test]
    fn test_function_not_available() {
        let err = RendererError::FunctionNotAvailable("func".into());
        assert!(err.to_string().contains("func"));
    }

    #[test]
    fn test_invalid_state_transition() {
        let err = RendererError::InvalidStateTransition {
            current: SurfaceState::Deleted,
            attempted: StateOperation::CreateSurface,
        };
        assert!(err.to_string().contains("Deleted"));
    }

    #[test]
    fn test_from_a2ui_error() {
        let a2ui_err = a2ui_core::A2uiError::SurfaceNotFound("s1".into());
        let renderer_err: RendererError = a2ui_err.into();
        assert!(matches!(renderer_err, RendererError::CoreError(_)));
    }

    #[test]
    fn test_binding_error() {
        let err = RendererError::BindingError("path not found".into());
        assert!(err.to_string().contains("path not found"));
    }

    #[test]
    fn test_path_error() {
        let err = RendererError::PathError("invalid pointer".into());
        assert!(err.to_string().contains("invalid pointer"));
    }

    // --- P2-3: 错误信息 sanitization ---

    #[test]
    fn test_sanitize_surface_not_found() {
        let err = RendererError::SurfaceNotFound("secret-surface-id".into());
        assert_eq!(err.sanitize(), "Surface not found");
    }

    #[test]
    fn test_sanitize_component_not_found() {
        let err = RendererError::ComponentNotFound(ComponentId::new("secret_comp").unwrap());
        assert_eq!(err.sanitize(), "Component reference not found");
    }

    #[test]
    fn test_sanitize_function_not_available() {
        let err = RendererError::FunctionNotAvailable("secret-func".into());
        assert_eq!(err.sanitize(), "Function not available");
    }

    #[test]
    fn test_sanitize_binding_error() {
        let err = RendererError::BindingError("internal path: /secret/data".into());
        assert_eq!(err.sanitize(), "Binding error");
    }

    #[test]
    fn test_sanitize_path_error() {
        let err = RendererError::PathError("/secret/path".into());
        assert_eq!(err.sanitize(), "Path error");
    }

    #[test]
    fn test_sanitize_invalid_function_call() {
        let err = RendererError::InvalidFunctionCall("secret-func".into());
        assert_eq!(err.sanitize(), "Invalid function call");
    }

    #[test]
    fn test_sanitize_preserves_limit_errors() {
        let err = RendererError::SurfaceLimitExceeded {
            current: 101,
            max: 100,
        };
        assert!(err.sanitize().contains("limit exceeded"));
    }

    #[test]
    fn test_sanitize_preserves_state_transition() {
        use a2ui_core::state::{StateOperation, SurfaceState};
        let err = RendererError::InvalidStateTransition {
            current: SurfaceState::Deleted,
            attempted: StateOperation::CreateSurface,
        };
        assert!(err.sanitize().contains("Invalid state transition"));
    }

    #[test]
    fn test_original_message_preserved() {
        let err = RendererError::SurfaceNotFound("s1".into());
        assert_eq!(err.to_string(), "Surface not found: s1");
        assert_eq!(err.sanitize(), "Surface not found");
    }
}
