use a2ui_core::prelude::*;
use a2ui_renderer::UserEvent;

/// 输入处理器：将平台事件转换为 A2UI UserEvent
pub struct InputHandler;

impl InputHandler {
    /// 处理键盘事件
    pub fn handle_key(&self, key: &str) -> Option<UserEvent> {
        match key {
            "Enter" => Some(UserEvent::KeyPress {
                key: "Enter".into(),
            }),
            "Escape" => Some(UserEvent::KeyPress {
                key: "Escape".into(),
            }),
            "Tab" => Some(UserEvent::KeyPress { key: "Tab".into() }),
            "Backspace" => Some(UserEvent::KeyPress {
                key: "Backspace".into(),
            }),
            "Delete" => Some(UserEvent::KeyPress {
                key: "Delete".into(),
            }),
            "Up" => Some(UserEvent::KeyPress { key: "Up".into() }),
            "Down" => Some(UserEvent::KeyPress { key: "Down".into() }),
            "Left" => Some(UserEvent::KeyPress { key: "Left".into() }),
            "Right" => Some(UserEvent::KeyPress {
                key: "Right".into(),
            }),
            other if !other.is_empty() && !other.starts_with("Ctrl+") => {
                Some(UserEvent::TextInput {
                    component_id: ComponentId::new("unknown").unwrap(),
                    value: other.to_string(),
                })
            }
            _ => None,
        }
    }

    /// 处理鼠标点击
    pub fn handle_click(&self, component_id: impl Into<String>) -> UserEvent {
        UserEvent::Click {
            component_id: ComponentId::new(component_id.into()).unwrap(),
        }
    }

    /// 创建复选框切换事件
    pub fn handle_check_toggle(&self, component_id: impl Into<String>, checked: bool) -> UserEvent {
        UserEvent::CheckToggle {
            component_id: ComponentId::new(component_id.into()).unwrap(),
            checked,
        }
    }

    /// 创建滑块变化事件
    pub fn handle_slider_change(&self, component_id: impl Into<String>, value: f64) -> UserEvent {
        UserEvent::SliderChange {
            component_id: ComponentId::new(component_id.into()).unwrap(),
            value,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enter_key() {
        let handler = InputHandler;
        assert!(matches!(
            handler.handle_key("Enter"),
            Some(UserEvent::KeyPress { key }) if key == "Enter"
        ));
    }

    #[test]
    fn test_escape_key() {
        let handler = InputHandler;
        assert!(matches!(
            handler.handle_key("Escape"),
            Some(UserEvent::KeyPress { key }) if key == "Escape"
        ));
    }

    #[test]
    fn test_tab_key() {
        let handler = InputHandler;
        assert!(matches!(
            handler.handle_key("Tab"),
            Some(UserEvent::KeyPress { key }) if key == "Tab"
        ));
    }

    #[test]
    fn test_regular_key() {
        let handler = InputHandler;
        if let Some(UserEvent::TextInput { value, .. }) = handler.handle_key("a") {
            assert_eq!(value, "a");
        } else {
            panic!("expected TextInput");
        }
    }

    #[test]
    fn test_ctrl_key() {
        let handler = InputHandler;
        assert!(handler.handle_key("Ctrl+C").is_none());
    }

    #[test]
    fn test_empty_key() {
        let handler = InputHandler;
        assert!(handler.handle_key("").is_none());
    }

    #[test]
    fn test_handle_click() {
        let handler = InputHandler;
        let event = handler.handle_click("btn1");
        match event {
            UserEvent::Click { component_id } => {
                assert_eq!(component_id.as_str(), "btn1");
            }
            _ => panic!("expected Click"),
        }
    }

    #[test]
    fn test_handle_check_toggle() {
        let handler = InputHandler;
        let event = handler.handle_check_toggle("cb1", true);
        match event {
            UserEvent::CheckToggle {
                component_id,
                checked,
            } => {
                assert_eq!(component_id.as_str(), "cb1");
                assert!(checked);
            }
            _ => panic!("expected CheckToggle"),
        }
    }

    #[test]
    fn test_handle_slider_change() {
        let handler = InputHandler;
        let event = handler.handle_slider_change("slider1", 0.75);
        match event {
            UserEvent::SliderChange {
                component_id,
                value,
            } => {
                assert_eq!(component_id.as_str(), "slider1");
                assert_eq!(value, 0.75);
            }
            _ => panic!("expected SliderChange"),
        }
    }
}
