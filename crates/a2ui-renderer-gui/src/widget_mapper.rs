use a2ui_core::prelude::*;

/// Widget Mapper：将 A2UI 组件映射为 egui UI 指令
pub struct WidgetMapper;

impl WidgetMapper {
    /// 从组件属性中提取文本内容
    pub fn extract_text(&self, component: &Component) -> String {
        let props = component.properties();
        if let Some(text_val) = props.get("text") {
            if let Some(s) = text_val.as_str() {
                return s.to_string();
            }
            if let Some(obj) = text_val.as_object() {
                if let Some(path_val) = obj.get("path") {
                    if let Some(p) = path_val.as_str() {
                        return format!("{{path:{}}}", p);
                    }
                }
                if let Some(call_val) = obj.get("call") {
                    if let Some(c) = call_val.as_str() {
                        return format!("{{call:{}}}", c);
                    }
                }
            }
        }
        format!("[{}]", component.component_type())
    }

    /// 判断组件是否可聚焦
    pub fn is_focusable(&self, component: &Component) -> bool {
        matches!(
            component.component_type(),
            "Button" | "TextField" | "CheckBox" | "ChoicePicker" | "Slider"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_text_literal() {
        let mapper = WidgetMapper;
        let comp = Component::text(
            ComponentId::new("t").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        assert_eq!(mapper.extract_text(&comp), "Hello");
    }

    #[test]
    fn test_button_is_focusable() {
        let mapper = WidgetMapper;
        let comp = Component::button(
            ComponentId::new("btn").unwrap(),
            ComponentId::new("lbl").unwrap(),
        );
        assert!(mapper.is_focusable(&comp));
    }

    #[test]
    fn test_text_is_not_focusable() {
        let mapper = WidgetMapper;
        let comp = Component::text(
            ComponentId::new("t").unwrap(),
            DynamicValue::Literal("hi".to_string()),
        );
        assert!(!mapper.is_focusable(&comp));
    }
}
