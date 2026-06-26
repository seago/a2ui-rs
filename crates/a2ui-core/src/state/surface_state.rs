use crate::error::{A2uiError, Result};

/// Surface 生命周期状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SurfaceState {
    /// 已收到 createSurface 但尚未完全初始化
    Pending,
    /// 正常状态，可接收更新
    Active,
    /// 已销毁，不可再操作
    Deleted,
}

impl SurfaceState {
    /// 获取状态的字符串表示
    pub fn as_str(&self) -> &'static str {
        match self {
            SurfaceState::Pending => "Pending",
            SurfaceState::Active => "Active",
            SurfaceState::Deleted => "Deleted",
        }
    }
}

impl std::fmt::Display for SurfaceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 状态机操作类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateOperation {
    CreateSurface,
    UpdateComponents,
    UpdateDataModel,
    DeleteSurface,
}

impl StateOperation {
    pub fn as_str(&self) -> &'static str {
        match self {
            StateOperation::CreateSurface => "CreateSurface",
            StateOperation::UpdateComponents => "UpdateComponents",
            StateOperation::UpdateDataModel => "UpdateDataModel",
            StateOperation::DeleteSurface => "DeleteSurface",
        }
    }
}

/// Surface 状态机，跟踪单个 Surface 的生命周期
#[derive(Debug, Clone)]
pub struct StateMachine {
    surface_id: String,
    state: SurfaceState,
}

impl StateMachine {
    /// 创建新的状态机（初始为 Pending）
    pub fn new(surface_id: String) -> Self {
        Self {
            surface_id,
            state: SurfaceState::Pending,
        }
    }

    /// 获取当前状态
    pub fn state(&self) -> SurfaceState {
        self.state
    }

    /// 获取 Surface ID
    pub fn surface_id(&self) -> &str {
        &self.surface_id
    }

    /// 尝试执行 createSurface（Pending → Active）
    pub fn create_surface(&mut self) -> Result<()> {
        match self.state {
            SurfaceState::Pending => {
                self.state = SurfaceState::Active;
                Ok(())
            }
            SurfaceState::Active => Err(A2uiError::InvalidStateTransition {
                current: SurfaceState::Active.as_str().to_string(),
                attempted: StateOperation::CreateSurface.as_str().to_string(),
            }),
            SurfaceState::Deleted => Err(A2uiError::InvalidStateTransition {
                current: SurfaceState::Deleted.as_str().to_string(),
                attempted: StateOperation::CreateSurface.as_str().to_string(),
            }),
        }
    }

    /// 尝试执行 updateComponents（需 Active）
    pub fn update_components(&self) -> Result<()> {
        match self.state {
            SurfaceState::Active => Ok(()),
            _ => Err(A2uiError::InvalidStateTransition {
                current: self.state.as_str().to_string(),
                attempted: StateOperation::UpdateComponents.as_str().to_string(),
            }),
        }
    }

    /// 尝试执行 updateDataModel（需 Active）
    pub fn update_data_model(&self) -> Result<()> {
        match self.state {
            SurfaceState::Active => Ok(()),
            _ => Err(A2uiError::InvalidStateTransition {
                current: self.state.as_str().to_string(),
                attempted: StateOperation::UpdateDataModel.as_str().to_string(),
            }),
        }
    }

    /// 尝试执行 deleteSurface（Active → Deleted）
    pub fn delete_surface(&mut self) -> Result<()> {
        match self.state {
            SurfaceState::Active => {
                self.state = SurfaceState::Deleted;
                Ok(())
            }
            SurfaceState::Pending => Err(A2uiError::InvalidStateTransition {
                current: SurfaceState::Pending.as_str().to_string(),
                attempted: StateOperation::DeleteSurface.as_str().to_string(),
            }),
            SurfaceState::Deleted => Err(A2uiError::InvalidStateTransition {
                current: SurfaceState::Deleted.as_str().to_string(),
                attempted: StateOperation::DeleteSurface.as_str().to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state_is_pending() {
        let sm = StateMachine::new("s1".to_string());
        assert_eq!(sm.state(), SurfaceState::Pending);
    }

    #[test]
    fn test_pending_to_active() {
        let mut sm = StateMachine::new("s1".to_string());
        sm.create_surface().unwrap();
        assert_eq!(sm.state(), SurfaceState::Active);
    }

    #[test]
    fn test_active_can_update() {
        let mut sm = StateMachine::new("s1".to_string());
        sm.create_surface().unwrap();
        assert!(sm.update_components().is_ok());
        assert!(sm.update_data_model().is_ok());
    }

    #[test]
    fn test_active_to_deleted() {
        let mut sm = StateMachine::new("s1".to_string());
        sm.create_surface().unwrap();
        sm.delete_surface().unwrap();
        assert_eq!(sm.state(), SurfaceState::Deleted);
    }

    #[test]
    fn test_deleted_cannot_update() {
        let mut sm = StateMachine::new("s1".to_string());
        sm.create_surface().unwrap();
        sm.delete_surface().unwrap();
        assert!(sm.update_components().is_err());
    }

    #[test]
    fn test_deleted_cannot_recreate() {
        let mut sm = StateMachine::new("s1".to_string());
        sm.create_surface().unwrap();
        sm.delete_surface().unwrap();
        assert!(sm.create_surface().is_err());
    }

    #[test]
    fn test_pending_cannot_delete() {
        let mut sm = StateMachine::new("s1".to_string());
        assert!(sm.delete_surface().is_err());
    }

    #[test]
    fn test_display_variants() {
        assert_eq!(SurfaceState::Pending.as_str(), "Pending");
        assert_eq!(SurfaceState::Active.as_str(), "Active");
        assert_eq!(SurfaceState::Deleted.as_str(), "Deleted");
    }

    #[test]
    fn test_state_operation_str() {
        assert_eq!(StateOperation::CreateSurface.as_str(), "CreateSurface");
        assert_eq!(
            StateOperation::UpdateComponents.as_str(),
            "UpdateComponents"
        );
        assert_eq!(StateOperation::UpdateDataModel.as_str(), "UpdateDataModel");
        assert_eq!(StateOperation::DeleteSurface.as_str(), "DeleteSurface");
    }
}
