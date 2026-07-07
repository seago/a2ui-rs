use crate::widget_builder::component_style_to_tui;
use a2ui_core::component::prop_keys;
use a2ui_core::prelude::*;
use a2ui_renderer::{resolve_str, ComponentStyle};
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
        let style = self.component_style(component);

        Paragraph::new(text).style(style)
    }

    /// 从组件属性中提取文本内容
    pub fn extract_text(&self, component: &Component) -> String {
        match component.prop_dynamic_value(prop_keys::TEXT) {
            Some(dv) => resolve_str(&dv, None),
            None => format!("[{}]", component.component_type()),
        }
    }

    /// 获取组件类型对应的样式
    pub fn component_style(&self, component: &Component) -> Style {
        match component.component_type() {
            "Button" => Style::default().fg(Color::Cyan),
            "Text" | "Icon" => component_style_to_tui(&ComponentStyle::from_component(component)),
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
    use ratatui::style::Modifier;

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
    fn test_component_style_text_uses_shared_style_contract() {
        let mapper = WidgetMapper;
        let comp: Component = Component::from_value(json!({
            "id": "t",
            "component": "Text",
            "text": "hi",
            "style": {
                "strong": true,
                "color": "#112233",
                "fill": "#445566",
                "padding": 9,
                "radius": 5
            }
        }))
        .unwrap();

        let style = mapper.component_style(&comp);
        assert_eq!(style.fg, Some(Color::Rgb(17, 34, 51)));
        assert_eq!(style.bg, None);
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_component_style_icon_uses_shared_style_contract() {
        let mapper = WidgetMapper;
        let comp: Component = Component::from_value(json!({
            "id": "i",
            "component": "Icon",
            "name": "star",
            "style": {
                "strong": true,
                "color": "#112233"
            }
        }))
        .unwrap();

        let style = mapper.component_style(&comp);
        assert_eq!(style.fg, Some(Color::Rgb(17, 34, 51)));
        assert!(style.add_modifier.contains(Modifier::BOLD));
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
