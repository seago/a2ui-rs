use crate::RenderResult;
use a2ui_core::message::client_to_server::FunctionResponse;
use a2ui_core::message::server_to_client::{
    ActionResponse, CallFunction, CreateSurface, DeleteSurface, UpdateComponents, UpdateDataModel,
};
use a2ui_core::prelude::*;
use uuid::Uuid;

/// Surface 句柄（全局唯一标识符）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceHandle(Uuid);

impl SurfaceHandle {
    /// 创建新的随机 SurfaceHandle
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// 获取内部 UUID 引用
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for SurfaceHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// 用户事件（平台无关）
#[derive(Debug, Clone)]
pub enum UserEvent {
    /// 点击事件
    Click { component_id: ComponentId },
    /// 键盘按下事件
    KeyPress { key: String },
    /// 文本输入事件
    TextInput {
        component_id: ComponentId,
        value: String,
    },
    /// 复选框切换事件
    CheckToggle {
        component_id: ComponentId,
        checked: bool,
    },
    /// 滑块变化事件
    SliderChange {
        component_id: ComponentId,
        value: f64,
    },
}

/// 渲染器 trait — 各平台 crate 必须实现此 trait
#[async_trait::async_trait]
pub trait Renderer: Send {
    /// 创建新的 Surface，返回句柄
    async fn create_surface(&mut self, msg: CreateSurface) -> RenderResult<SurfaceHandle>;

    /// 向指定 Surface 添加或更新组件
    async fn update_components(&mut self, msg: UpdateComponents) -> RenderResult<()>;

    /// 更新指定 Surface 的 Data Model
    async fn update_data_model(&mut self, msg: UpdateDataModel) -> RenderResult<()>;

    /// 销毁指定 Surface
    async fn delete_surface(&mut self, msg: DeleteSurface) -> RenderResult<()>;

    /// 处理服务端对 action 的响应
    async fn action_response(&mut self, msg: ActionResponse) -> RenderResult<()>;

    /// 执行服务端请求的客户端函数
    async fn call_function(&mut self, msg: CallFunction) -> RenderResult<FunctionResponse>;

    /// 渲染当前帧（各平台自行实现）
    async fn render(&mut self) -> RenderResult<()>;

    /// 处理用户交互，生成 action 消息
    async fn handle_user_event(&mut self, event: UserEvent) -> RenderResult<Option<ActionMessage>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_surface_handle_new() {
        let h1 = SurfaceHandle::new();
        let h2 = SurfaceHandle::new();
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_user_event_variants() {
        let _e = UserEvent::Click {
            component_id: ComponentId::new("btn").unwrap(),
        };
        let _e = UserEvent::KeyPress {
            key: "Enter".into(),
        };
        let _e = UserEvent::TextInput {
            component_id: ComponentId::new("input").unwrap(),
            value: "hi".into(),
        };
        let _e = UserEvent::CheckToggle {
            component_id: ComponentId::new("cb").unwrap(),
            checked: true,
        };
        let _e = UserEvent::SliderChange {
            component_id: ComponentId::new("slider").unwrap(),
            value: 0.5,
        };
    }

    #[test]
    fn test_renderer_trait_compiles() {
        // 确保 trait 可以被实现（仅编译检查）
        fn assert_impl<R: Renderer + Send + 'static>() {}
    }
}
