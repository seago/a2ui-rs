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
    let value = value.as_str()?.trim();
    let hex = value.strip_prefix('#').unwrap_or(value);
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
    use serde_json::json;

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
    fn ignores_invalid_spacing() {
        let style = ComponentStyle::from_component_props(&json!({
            "style": {"spacing": "tight"}
        }));

        assert_eq!(style.spacing, None);
    }
}
