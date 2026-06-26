use a2ui_core::A2uiError;
use a2ui_core::state::{StateOperation, SurfaceState};
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
}

pub type RenderResult<T> = Result<T, RendererError>;

#[cfg(test)]
mod tests {
    use super::*;
    use a2ui_core::{ComponentId, state::{SurfaceState, StateOperation}};

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
}
