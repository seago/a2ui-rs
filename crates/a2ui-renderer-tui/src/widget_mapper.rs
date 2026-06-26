use a2ui_core::prelude::*;
use ratatui::{
    layout::Constraint,
    style::{Color, Style},
    widgets::Paragraph,
};

/// Widget Mapper：将 A2UI 组件映射为 ratatui widget
pub struct WidgetMapper;

impl WidgetMapper {
    /// 将 A2UI 组件映射为 ratatui Paragraph（简化实现）
    pub fn map_to_paragraph(&self, component: &Component) -> Paragraph<'static> {
        let text = self.extract_text(component);
        let style = Style::default();

        Paragraph::new(text).style(style)
    }

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

    /// 获取组件类型对应的样式
    pub fn component_style(&self, component: &Component) -> Style {
        match component.component_type() {
            "Button" => Style::default().fg(Color::Cyan),
            "Text" => Style::default(),
            "TextField" => Style::default(),
            "CheckBox" => Style::default(),
            "Divider" => Style::default().fg(Color::Gray),
            _ => Style::default(),
        }
    }

    /// 判断组件是否可聚焦
    pub fn is_focusable(&self, component: &Component) -> bool {
        matches!(
            component.component_type(),
            "Button" | "TextField" | "CheckBox" | "ChoicePicker" | "Slider"
        )
    }

    /// 获取组件的默认尺寸约束
    pub fn size_constraints(&self, component: &Component) -> (Constraint, Constraint) {
        match component.component_type() {
            "Column" | "Row" | "List" => (Constraint::Min(1), Constraint::Percentage(100)),
            _ => (Constraint::Length(1), Constraint::Length(1)),
        }
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
    fn test_extract_text_from_properties() {
        let mapper = WidgetMapper;
        let comp = Component::text(
            ComponentId::new("t").unwrap(),
            DynamicValue::Literal("World".to_string()),
        );
        assert_eq!(mapper.extract_text(&comp), "World");
    }

    #[test]
    fn test_button_is_focusable() {
        let mapper = WidgetMapper;
        let comp = Component::button(
            ComponentId::new("btn").unwrap(),
            ComponentId::new("label").unwrap(),
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

    #[test]
    fn test_column_size_constraints() {
        let mapper = WidgetMapper;
        let comp = Component::column(ComponentId::new("col").unwrap(), vec![]);
        let (w, _h) = mapper.size_constraints(&comp);
        assert!(matches!(w, Constraint::Min(_)));
    }

    #[test]
    fn test_component_style_button() {
        let mapper = WidgetMapper;
        let comp = Component::button(
            ComponentId::new("btn").unwrap(),
            ComponentId::new("lbl").unwrap(),
        );
        let style = mapper.component_style(&comp);
        assert_eq!(style.fg, Some(Color::Cyan));
    }

    #[test]
    fn test_component_style_text() {
        let mapper = WidgetMapper;
        let comp = Component::text(
            ComponentId::new("t").unwrap(),
            DynamicValue::Literal("hi".to_string()),
        );
        let style = mapper.component_style(&comp);
        assert_eq!(style, Style::default());
    }

    #[test]
    fn test_map_to_paragraph() {
        let mapper = WidgetMapper;
        let comp = Component::text(
            ComponentId::new("t").unwrap(),
            DynamicValue::Literal("Hello".to_string()),
        );
        let _para = mapper.map_to_paragraph(&comp);
    }
}
