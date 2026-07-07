use a2ui_core::component::component::Component;
use a2ui_core::component::{SpacingDecl, StyleDecl};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StyleColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StyleSpacing {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ComponentStyle {
    pub font_size: Option<f32>,
    pub strong: bool,
    pub color: Option<StyleColor>,
    pub fill: Option<StyleColor>,
    pub padding: Option<f32>,
    pub spacing: Option<StyleSpacing>,
    pub radius: Option<f32>,
}

impl ComponentStyle {
    pub fn from_component_props(props: &Value) -> Self {
        let Some(style) = props.get("style").and_then(|v| v.as_object()) else {
            return Self::default();
        };

        Self {
            font_size: extract_f32(style, "fontSize"),
            strong: style
                .get("strong")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            color: style.get("color").and_then(parse_color),
            fill: style.get("fill").and_then(parse_color),
            padding: extract_f32(style, "padding"),
            spacing: extract_spacing(style),
            radius: extract_f32(style, "radius"),
        }
    }

    /// 从组件的 [`StyleDecl`] 结构化视图构造样式（新 API）。
    ///
    /// 与 [`ComponentStyle::from_component_props`] 逐字段等价：core 视图
    /// 给出原始声明（f64 数值 / 颜色字符串 / spacing 双形态），本方法
    /// 补上渲染语义（f32 换算、十六进制颜色解析、Uniform → 纵向间距）。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use a2ui_core::component::component::Component;
    /// use a2ui_renderer::ComponentStyle;
    /// use serde::Deserialize;
    /// use serde_json::json;
    ///
    /// let c = Component::deserialize(json!({
    ///     "component": "Text", "id": "t", "text": "hi",
    ///     "style": {"fontSize": 22, "strong": true}
    /// })).unwrap();
    /// let style = ComponentStyle::from_component(&c);
    /// assert_eq!(style.font_size, Some(22.0));
    /// assert!(style.strong);
    /// ```
    pub fn from_component(component: &Component) -> Self {
        let Some(decl) = component.style_decl() else {
            return Self::default();
        };
        Self::from_style_decl(&decl)
    }

    /// 从已解析的 [`StyleDecl`] 构造样式（[`ComponentStyle::from_component`]
    /// 的组成部分，供已持有视图的调用方复用）。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use a2ui_core::component::StyleDecl;
    /// use a2ui_renderer::ComponentStyle;
    ///
    /// let decl = StyleDecl { padding: Some(12.0), ..Default::default() };
    /// assert_eq!(ComponentStyle::from_style_decl(&decl).padding, Some(12.0));
    /// ```
    pub fn from_style_decl(decl: &StyleDecl) -> Self {
        Self {
            font_size: decl.font_size.map(|v| v as f32),
            strong: decl.strong.unwrap_or(false),
            color: decl.color.as_deref().and_then(parse_color_str),
            fill: decl.fill.as_deref().and_then(parse_color_str),
            padding: decl.padding.map(|v| v as f32),
            spacing: decl.spacing.map(|spacing| match spacing {
                SpacingDecl::Uniform(y) => StyleSpacing {
                    x: 0.0,
                    y: y as f32,
                },
                SpacingDecl::Xy { x, y } => StyleSpacing {
                    x: x as f32,
                    y: y as f32,
                },
            }),
            radius: decl.radius.map(|v| v as f32),
        }
    }
}

fn extract_f32(style: &Map<String, Value>, key: &str) -> Option<f32> {
    style.get(key).and_then(|v| v.as_f64()).map(|v| v as f32)
}

fn extract_spacing(style: &Map<String, Value>) -> Option<StyleSpacing> {
    let value = style.get("spacing")?;
    if let Some(number) = value.as_f64() {
        return Some(StyleSpacing {
            x: 0.0,
            y: number as f32,
        });
    }

    let object = value.as_object()?;
    Some(StyleSpacing {
        x: object.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
        y: object.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
    })
}

fn parse_color(value: &Value) -> Option<StyleColor> {
    parse_color_str(value.as_str()?)
}

fn parse_color_str(value: &str) -> Option<StyleColor> {
    let value = value.trim();
    let hex = value.strip_prefix('#').unwrap_or(value);
    // len() 是字节长度、下面按字节范围切片：非 ASCII 输入会落在非字符
    // 边界导致 panic，且合法十六进制颜色只含 ASCII，直接拒绝
    if !hex.is_ascii() {
        return None;
    }
    let parse = |range: std::ops::Range<usize>| u8::from_str_radix(&hex[range], 16).ok();

    match hex.len() {
        6 => Some(StyleColor {
            r: parse(0..2)?,
            g: parse(2..4)?,
            b: parse(4..6)?,
            a: 255,
        }),
        8 => Some(StyleColor {
            r: parse(0..2)?,
            g: parse(2..4)?,
            b: parse(4..6)?,
            a: parse(6..8)?,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use serde_json::json;

    #[test]
    fn from_component_matches_legacy_props_parser() {
        // 新旧两条路径必须产出完全一致的结果（含 spacing 三形态与非法输入）
        let samples = [
            json!({
                "component": "Text", "id": "t1",
                "style": {
                    "fontSize": 22, "strong": true, "color": "#1976d2",
                    "fill": "#44556680", "padding": 12,
                    "spacing": {"x": 6, "y": 0}, "radius": 10
                }
            }),
            json!({"component": "Text", "id": "t2", "style": {"spacing": 8}}),
            json!({"component": "Text", "id": "t3", "style": {"spacing": {"x": 6}}}),
            json!({"component": "Text", "id": "t4", "style": {"spacing": "tight"}}),
            json!({"component": "Text", "id": "t5", "style": {"color": "red", "fill": "€€"}}),
            json!({"component": "Text", "id": "t6", "style": {"fontSize": "big", "padding": 3}}),
            json!({"component": "Text", "id": "t7", "style": "bold"}),
            json!({"component": "Text", "id": "t8"}),
        ];
        for sample in samples {
            let component =
                a2ui_core::component::component::Component::deserialize(sample.clone()).unwrap();
            assert_eq!(
                ComponentStyle::from_component(&component),
                ComponentStyle::from_component_props(component.properties()),
                "style parse mismatch for {sample}"
            );
        }
    }

    #[test]
    fn parses_style_fields() {
        let style = ComponentStyle::from_component_props(&json!({
            "style": {
                "fontSize": 22,
                "strong": true,
                "color": "#1976d2",
                "fill": "#fafafa",
                "padding": 12,
                "spacing": {"x": 6, "y": 0},
                "radius": 10
            }
        }));

        assert_eq!(style.font_size, Some(22.0));
        assert!(style.strong);
        assert_eq!(
            style.color,
            Some(StyleColor {
                r: 25,
                g: 118,
                b: 210,
                a: 255,
            })
        );
        assert_eq!(style.padding, Some(12.0));
        assert_eq!(
            style.fill,
            Some(StyleColor {
                r: 250,
                g: 250,
                b: 250,
                a: 255,
            })
        );
        assert_eq!(style.spacing, Some(StyleSpacing { x: 6.0, y: 0.0 }));
        assert_eq!(style.radius, Some(10.0));
    }

    #[test]
    fn parses_same_contract_fields_for_supported_components() {
        let supported_components = ["Text", "Icon", "Row", "Column", "List", "Card", "Image"];

        for component in supported_components {
            let style = ComponentStyle::from_component_props(&json!({
                "component": component,
                "id": "styled",
                "style": {
                    "fontSize": 18,
                    "strong": true,
                    "color": "#112233",
                    "fill": "#44556680",
                    "padding": 9,
                    "spacing": {"x": 7, "y": 11},
                    "radius": 5
                }
            }));

            assert_eq!(
                style,
                ComponentStyle {
                    font_size: Some(18.0),
                    strong: true,
                    color: Some(StyleColor {
                        r: 17,
                        g: 34,
                        b: 51,
                        a: 255,
                    }),
                    fill: Some(StyleColor {
                        r: 68,
                        g: 85,
                        b: 102,
                        a: 128,
                    }),
                    padding: Some(9.0),
                    spacing: Some(StyleSpacing { x: 7.0, y: 11.0 }),
                    radius: Some(5.0),
                },
                "style contract should parse consistently for {component}"
            );
        }
    }

    #[test]
    fn returns_default_for_missing_or_non_object_style() {
        assert_eq!(
            ComponentStyle::from_component_props(&json!({})),
            ComponentStyle::default()
        );
        assert_eq!(
            ComponentStyle::from_component_props(&json!({"style": null})),
            ComponentStyle::default()
        );
        assert_eq!(
            ComponentStyle::from_component_props(&json!({"style": "bold"})),
            ComponentStyle::default()
        );
    }

    #[test]
    fn parses_rgba_hex_color() {
        let style = ComponentStyle::from_component_props(&json!({
            "style": {"color": "#1976d280"}
        }));

        assert_eq!(
            style.color,
            Some(StyleColor {
                r: 25,
                g: 118,
                b: 210,
                a: 128,
            })
        );
    }

    #[test]
    fn parses_single_number_spacing_as_vertical_spacing() {
        let style = ComponentStyle::from_component_props(&json!({
            "style": {"spacing": 8}
        }));

        assert_eq!(style.spacing, Some(StyleSpacing { x: 0.0, y: 8.0 }));
    }

    #[test]
    fn parses_partial_spacing_object_with_zero_defaults() {
        let style = ComponentStyle::from_component_props(&json!({
            "style": {"spacing": {"x": 6}}
        }));

        assert_eq!(style.spacing, Some(StyleSpacing { x: 6.0, y: 0.0 }));
    }

    #[test]
    fn ignores_invalid_color() {
        let style = ComponentStyle::from_component_props(&json!({
            "style": {"color": "red"}
        }));

        assert_eq!(style.color, None);
    }

    #[test]
    fn ignores_non_ascii_color_without_panicking() {
        // "€€" 为 6 字节 2 字符：按字节长度进入 6 位分支后，
        // 字节切片落在非字符边界会 panic——必须安全返回 None
        for bad in ["€€", "#€€", "ééé", "#ffff€€", "\u{1F600}\u{1F600}"] {
            let style = ComponentStyle::from_component_props(&json!({
                "style": {"color": bad, "fill": bad}
            }));
            assert_eq!(style.color, None, "input {bad:?} must not parse");
            assert_eq!(style.fill, None, "input {bad:?} must not parse");
        }
    }

    #[test]
    fn ignores_invalid_spacing() {
        let style = ComponentStyle::from_component_props(&json!({
            "style": {"spacing": "tight"}
        }));

        assert_eq!(style.spacing, None);
    }
}
