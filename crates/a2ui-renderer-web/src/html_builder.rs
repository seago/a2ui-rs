use a2ui_core::prelude::*;
use a2ui_renderer::{ChoiceOption, ComponentStyle, StyleColor, StyleSpacing};

/// HTML 渲染 widget
#[derive(Debug, Clone)]
pub enum RenderableHtmlWidget {
    /// 带共享样式的组件包装
    Styled {
        widget: Box<RenderableHtmlWidget>,
        style: ComponentStyle,
    },
    /// 文本组件
    Text {
        id: ComponentId,
        text: String,
        variant: String,
    },
    /// 按钮组件
    Button {
        id: ComponentId,
        label: String,
        variant: String,
    },
    /// 列布局组件
    Column {
        id: ComponentId,
        children: Vec<RenderableHtmlWidget>,
    },
    /// 行布局组件
    Row {
        id: ComponentId,
        children: Vec<RenderableHtmlWidget>,
    },
    /// 图片组件
    Image { id: ComponentId, url: String },
    /// 卡片组件
    Card {
        id: ComponentId,
        child: Box<RenderableHtmlWidget>,
    },
    /// 复选框组件
    CheckBox {
        id: ComponentId,
        checked: bool,
        label: String,
    },
    /// 分割线组件
    Divider { id: ComponentId },
    /// 图标组件
    Icon { id: ComponentId, name: String },
    /// 列表组件
    List {
        id: ComponentId,
        children: Vec<RenderableHtmlWidget>,
    },
    /// 标签页组件
    Tabs {
        id: ComponentId,
        tabs: Vec<(String, Vec<RenderableHtmlWidget>)>,
    },
    /// 模态框组件
    Modal {
        id: ComponentId,
        title: String,
        content: Box<RenderableHtmlWidget>,
    },
    /// 滑块组件
    Slider {
        id: ComponentId,
        value: f64,
        min: f64,
        max: f64,
    },
    /// 文本输入组件
    TextField {
        id: ComponentId,
        value: String,
        placeholder: String,
    },
    /// 选择器组件
    ChoicePicker {
        id: ComponentId,
        options: Vec<ChoiceOption>,
        selected: Vec<String>,
        /// 规范 variant（multipleSelection / mutuallyExclusive，默认单选），
        /// 决定渲染为 checkbox 组还是 radio 组
        variant: Option<String>,
    },
    /// 日期时间输入组件
    DateTimeInput { id: ComponentId, label: String },
    /// 视频组件
    Video { id: ComponentId, url: String },
    /// 音频播放器组件
    AudioPlayer { id: ComponentId, url: String },
    /// 占位符组件（未知类型兜底）
    Placeholder { id: ComponentId, reason: String },
}

/// HTML 构建器
///
/// 将 `RenderableHtmlWidget` 渲染为 HTML 字符串。
/// 所有输出都经过 HTML 转义以防范 XSS 攻击。
#[derive(Debug)]
pub struct HtmlBuilder;

impl HtmlBuilder {
    /// 创建新的 HTML 构建器
    pub fn new() -> Self {
        Self
    }

    /// 将 `RenderableHtmlWidget` 渲染为 HTML 字符串
    ///
    /// 每个组件类型映射为语义化的 HTML 元素，带有 `a2ui-*` CSS class 前缀。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use a2ui_renderer_web::HtmlBuilder;
    /// use a2ui_renderer_web::html_builder::RenderableHtmlWidget;
    /// use a2ui_core::ComponentId;
    ///
    /// let html = HtmlBuilder.render(&RenderableHtmlWidget::Text {
    ///     id: ComponentId::new("t1").unwrap(),
    ///     text: "Hello".to_string(),
    ///     variant: "body".to_string(),
    /// });
    /// assert!(html.contains("Hello"));
    /// ```
    pub fn render(&self, widget: &RenderableHtmlWidget) -> String {
        match widget {
            RenderableHtmlWidget::Styled { widget, style } => self.render_styled(widget, style),
            RenderableHtmlWidget::Text { text, variant, .. } => match variant.as_str() {
                "caption" => {
                    format!(
                        "<span class=\"a2ui-text a2ui-caption\">{}</span>",
                        html_escape(text)
                    )
                }
                _ => {
                    format!("<p class=\"a2ui-text a2ui-body\">{}</p>", html_escape(text))
                }
            },
            RenderableHtmlWidget::Button { label, variant, .. } => {
                let class = match variant.as_str() {
                    "primary" => "a2ui-button a2ui-primary",
                    "borderless" => "a2ui-button a2ui-borderless",
                    _ => "a2ui-button a2ui-default",
                };
                format!(
                    "<button class=\"{}\">{}</button>",
                    class,
                    html_escape(label)
                )
            }
            RenderableHtmlWidget::Column { children, .. } => {
                let inner: String = children.iter().map(|c| self.render(c)).collect();
                format!("<div class=\"a2ui-column\">{}</div>", inner)
            }
            RenderableHtmlWidget::Row { children, .. } => {
                let inner: String = children.iter().map(|c| self.render(c)).collect();
                format!("<div class=\"a2ui-row\">{}</div>", inner)
            }
            RenderableHtmlWidget::Image { url, .. } => {
                format!(
                    "<img class=\"a2ui-image\" src=\"{}\" alt=\"image\" />",
                    html_attr(url)
                )
            }
            RenderableHtmlWidget::Card { child, .. } => {
                let inner = self.render(child);
                format!("<div class=\"a2ui-card\">{}</div>", inner)
            }
            RenderableHtmlWidget::CheckBox { checked, label, .. } => {
                let checked_attr = if *checked { " checked" } else { "" };
                format!(
                    "<label class=\"a2ui-checkbox\"><input type=\"checkbox\"{} /> {}</label>",
                    checked_attr,
                    html_escape(label)
                )
            }
            RenderableHtmlWidget::Divider { .. } => "<hr class=\"a2ui-divider\" />".to_string(),
            RenderableHtmlWidget::Icon { name, .. } => {
                format!(
                    "<span class=\"a2ui-icon a2ui-icon-{}\" aria-label=\"{}\"></span>",
                    html_attr(name),
                    html_attr(name)
                )
            }
            RenderableHtmlWidget::List { children, .. } => {
                let items: String = children
                    .iter()
                    .map(|c| format!("<li>{}</li>", self.render(c)))
                    .collect();
                format!("<ul class=\"a2ui-list\">{}</ul>", items)
            }
            RenderableHtmlWidget::Tabs { tabs, .. } => {
                let tab_headers: String = tabs
                    .iter()
                    .enumerate()
                    .map(|(i, (title, _))| {
                        let active = if i == 0 { " active" } else { "" };
                        format!(
                            "<button class=\"a2ui-tab{}\">{}</button>",
                            active,
                            html_escape(title)
                        )
                    })
                    .collect();
                let tab_contents: String = tabs
                    .iter()
                    .enumerate()
                    .map(|(i, (_, content))| {
                        let active = if i == 0 { " active" } else { "" };
                        let inner: String = content.iter().map(|c| self.render(c)).collect();
                        format!("<div class=\"a2ui-tab-content{}\">{}</div>", active, inner)
                    })
                    .collect();
                format!(
                    "<div class=\"a2ui-tabs\"><div class=\"a2ui-tab-headers\">{}</div><div class=\"a2ui-tab-contents\">{}</div></div>",
                    tab_headers, tab_contents
                )
            }
            RenderableHtmlWidget::Modal { title, content, .. } => {
                let inner = self.render(content);
                format!(
                    "<div class=\"a2ui-modal\"><div class=\"a2ui-modal-header\">{}</div><div class=\"a2ui-modal-body\">{}</div></div>",
                    html_escape(title), inner
                )
            }
            RenderableHtmlWidget::Slider {
                value, min, max, ..
            } => {
                format!(
                    "<input type=\"range\" class=\"a2ui-slider\" value=\"{}\" min=\"{}\" max=\"{}\" />",
                    value, min, max
                )
            }
            RenderableHtmlWidget::TextField {
                value, placeholder, ..
            } => {
                format!(
                    "<input type=\"text\" class=\"a2ui-textfield\" value=\"{}\" placeholder=\"{}\" />",
                    html_attr(value),
                    html_attr(placeholder)
                )
            }
            RenderableHtmlWidget::ChoicePicker {
                id,
                options,
                selected,
                variant,
            } => {
                // 规范 variant 映射 input type：多选 checkbox / 单选（默认）
                // radio；宿主桥接层依据 data-a2ui-component-id 与 input 的
                // name/value 把 DOM 变更转为 UserEvent::ChoiceSelect
                let input_type = if variant.as_deref() == Some("multipleSelection") {
                    "checkbox"
                } else {
                    "radio"
                };
                let items: String = options
                    .iter()
                    .map(|o| {
                        // 选中匹配按选项稳定值，value 属性用稳定值、文本用 label
                        let checked = if selected.contains(&o.value) {
                            " checked"
                        } else {
                            ""
                        };
                        format!(
                            "<label class=\"a2ui-choice\"><input type=\"{}\" name=\"{}\" value=\"{}\"{} /> {}</label>",
                            input_type,
                            html_attr(id.as_str()),
                            html_attr(&o.value),
                            checked,
                            html_escape(&o.label)
                        )
                    })
                    .collect();
                format!(
                    "<fieldset class=\"a2ui-choicepicker\" data-a2ui-component-id=\"{}\">{}</fieldset>",
                    html_attr(id.as_str()),
                    items
                )
            }
            RenderableHtmlWidget::DateTimeInput { label, .. } => {
                format!(
                    "<div class=\"a2ui-datetime\"><label>{}</label><input type=\"datetime-local\" /></div>",
                    html_escape(label)
                )
            }
            RenderableHtmlWidget::Video { url, .. } => {
                format!(
                    "<video class=\"a2ui-video\" src=\"{}\" controls></video>",
                    html_attr(url)
                )
            }
            RenderableHtmlWidget::AudioPlayer { url, .. } => {
                format!(
                    "<audio class=\"a2ui-audio\" src=\"{}\" controls></audio>",
                    html_attr(url)
                )
            }
            RenderableHtmlWidget::Placeholder { reason, .. } => {
                format!(
                    "<div class=\"a2ui-placeholder\">[{}]</div>",
                    html_escape(reason)
                )
            }
        }
    }

    fn render_styled(&self, widget: &RenderableHtmlWidget, style: &ComponentStyle) -> String {
        match widget {
            RenderableHtmlWidget::Text { text, variant, .. } => {
                let style_attr = style_attr(style, StyleTarget::Text);
                match variant.as_str() {
                    "caption" => format!(
                        "<span class=\"a2ui-text a2ui-caption\"{}>{}</span>",
                        style_attr,
                        html_escape(text)
                    ),
                    _ => format!(
                        "<p class=\"a2ui-text a2ui-body\"{}>{}</p>",
                        style_attr,
                        html_escape(text)
                    ),
                }
            }
            RenderableHtmlWidget::Column { children, .. } => {
                let inner: String = children.iter().map(|c| self.render(c)).collect();
                format!(
                    "<div class=\"a2ui-column\"{}>{}</div>",
                    style_attr(style, StyleTarget::Column),
                    inner
                )
            }
            RenderableHtmlWidget::Row { children, .. } => {
                let inner: String = children.iter().map(|c| self.render(c)).collect();
                format!(
                    "<div class=\"a2ui-row\"{}>{}</div>",
                    style_attr(style, StyleTarget::Row),
                    inner
                )
            }
            RenderableHtmlWidget::Image { url, .. } => {
                format!(
                    "<img class=\"a2ui-image\" src=\"{}\" alt=\"image\"{} />",
                    html_attr(url),
                    style_attr(style, StyleTarget::Image)
                )
            }
            RenderableHtmlWidget::Card { child, .. } => {
                let inner = self.render(child);
                format!(
                    "<div class=\"a2ui-card\"{}>{}</div>",
                    style_attr(style, StyleTarget::Card),
                    inner
                )
            }
            RenderableHtmlWidget::Icon { name, .. } => {
                format!(
                    "<span class=\"a2ui-icon a2ui-icon-{}\" aria-label=\"{}\"{}></span>",
                    html_attr(name),
                    html_attr(name),
                    style_attr(style, StyleTarget::Icon)
                )
            }
            RenderableHtmlWidget::List { children, .. } => {
                let items: String = children
                    .iter()
                    .map(|c| format!("<li>{}</li>", self.render(c)))
                    .collect();
                format!(
                    "<ul class=\"a2ui-list\"{}>{}</ul>",
                    style_attr(style, StyleTarget::List),
                    items
                )
            }
            _ => {
                let inner = self.render(widget);
                let style_attr = style_attr(style, StyleTarget::Generic);
                if style_attr.is_empty() {
                    inner
                } else {
                    format!("<div class=\"a2ui-styled\"{}>{}</div>", style_attr, inner)
                }
            }
        }
    }

    /// 渲染完整的 HTML 页面包装
    ///
    /// 将 HTML body 内容嵌入到完整的 HTML5 文档中，包含 A2UI 基础样式。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use a2ui_renderer_web::HtmlBuilder;
    ///
    /// let html = HtmlBuilder.render_page("<p>Hello</p>", "My App");
    /// assert!(html.contains("<!DOCTYPE html>"));
    /// assert!(html.contains("Hello"));
    /// assert!(html.contains("My App"));
    /// ```
    pub fn render_page(&self, body: &str, title: &str) -> String {
        format!(
            r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{title}</title>
    <style>
        /* A2UI CSS Variables (override to customize theme) */
        :root {{
            --a2ui-primary-color: #1976D2;
            --a2ui-primary-text-color: #FFFFFF;
            --a2ui-bg-color: #FFFFFF;
            --a2ui-text-color: #212121;
            --a2ui-border-color: #CCCCCC;
            --a2ui-error-color: #FF0000;
            --a2ui-font-family: system-ui, sans-serif;
            --a2ui-font-size: 14px;
            --a2ui-border-radius: 4px;
            --a2ui-card-border-radius: 8px;
            --a2ui-item-spacing: 8px;
            --a2ui-icon-size: 24px;
            --a2ui-content-max-width: 720px;
        }}
        /* A2UI Basic Styles */
        .a2ui-body {{ margin: 0; padding: 16px; font-family: system-ui, sans-serif; }}
        .a2ui-column {{ display: flex; flex-direction: column; gap: 8px; }}
        .a2ui-row {{ display: flex; flex-direction: row; gap: 8px; align-items: center; }}
        .a2ui-card {{ border: 1px solid #ddd; border-radius: 8px; padding: 16px; margin: 8px 0; }}
        .a2ui-button {{ padding: 8px 16px; border-radius: 4px; border: 1px solid #ccc; cursor: pointer; background: #f5f5f5; }}
        .a2ui-button.a2ui-primary {{ background: #007aff; color: white; border-color: #007aff; }}
        .a2ui-text {{ margin: 4px 0; }}
        .a2ui-text.a2ui-caption {{ font-size: 0.85em; color: #666; }}
        .a2ui-image {{ max-width: 100%; height: auto; }}
        .a2ui-divider {{ border: none; border-top: 1px solid #eee; margin: 16px 0; }}
        .a2ui-checkbox {{ display: flex; align-items: center; gap: 8px; }}
        .a2ui-textfield {{ padding: 8px; border: 1px solid #ccc; border-radius: 4px; }}
        .a2ui-slider {{ width: 100%; }}
        .a2ui-list {{ padding-left: 24px; }}
        .a2ui-tab-headers {{ display: flex; gap: 4px; border-bottom: 1px solid #ddd; }}
        .a2ui-tab {{ padding: 8px 16px; border: none; background: none; cursor: pointer; }}
        .a2ui-tab.active {{ border-bottom: 2px solid #007aff; }}
        .a2ui-tab-content {{ display: none; padding: 16px 0; }}
        .a2ui-tab-content.active {{ display: block; }}
        .a2ui-modal {{ border: 1px solid #ddd; border-radius: 8px; overflow: hidden; }}
        .a2ui-modal-header {{ background: #f5f5f5; padding: 12px 16px; font-weight: bold; }}
        .a2ui-modal-body {{ padding: 16px; }}
        .a2ui-placeholder {{ padding: 8px; background: #fff3cd; border: 1px dashed #ffc107; border-radius: 4px; color: #856404; }}
        .a2ui-video, .a2ui-audio {{ max-width: 100%; }}
    </style>
</head>
<body class="a2ui-body">
    {body}
</body>
</html>"#,
            title = html_escape(title),
            body = body
        )
    }
}

impl Default for HtmlBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// HTML 文本内容转义
///
/// 将特殊字符转换为 HTML 实体，防止 XSS 攻击。
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// HTML 属性值转义
///
/// 对 HTML 属性值进行转义，防止通过属性注入的 XSS 攻击。
fn html_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[derive(Debug, Clone, Copy)]
enum StyleTarget {
    Text,
    Icon,
    Row,
    Column,
    List,
    Card,
    Image,
    Generic,
}

fn style_attr(style: &ComponentStyle, target: StyleTarget) -> String {
    let mut declarations = Vec::new();

    match target {
        StyleTarget::Text | StyleTarget::Icon => {
            push_text_style(&mut declarations, style);
        }
        StyleTarget::Row | StyleTarget::Column | StyleTarget::List => {
            if let Some(spacing) = style.spacing {
                push_gap_style(&mut declarations, spacing);
                if matches!(target, StyleTarget::List) {
                    declarations.push("display:flex".to_string());
                    declarations.push("flex-direction:column".to_string());
                }
            }
        }
        StyleTarget::Card | StyleTarget::Image => {
            push_box_style(&mut declarations, style);
        }
        StyleTarget::Generic => {
            push_text_style(&mut declarations, style);
            push_box_style(&mut declarations, style);
            if let Some(spacing) = style.spacing {
                push_gap_style(&mut declarations, spacing);
            }
        }
    }

    if declarations.is_empty() {
        String::new()
    } else {
        format!(" style=\"{}\"", html_attr(&declarations.join(";")))
    }
}

fn push_text_style(declarations: &mut Vec<String>, style: &ComponentStyle) {
    if let Some(font_size) = style.font_size {
        declarations.push(format!("font-size:{}", format_px(font_size)));
    }
    if style.strong {
        declarations.push("font-weight:700".to_string());
    }
    if let Some(color) = style.color {
        declarations.push(format!("color:{}", format_css_color(color)));
    }
}

fn push_box_style(declarations: &mut Vec<String>, style: &ComponentStyle) {
    if let Some(fill) = style.fill {
        declarations.push(format!("background-color:{}", format_css_color(fill)));
    }
    if let Some(padding) = style.padding {
        declarations.push(format!("padding:{}", format_px(padding)));
    }
    if let Some(radius) = style.radius {
        declarations.push(format!("border-radius:{}", format_px(radius)));
    }
}

fn push_gap_style(declarations: &mut Vec<String>, spacing: StyleSpacing) {
    declarations.push(format!(
        "gap:{} {}",
        format_px(spacing.y),
        format_px(spacing.x)
    ));
}

fn format_css_color(color: StyleColor) -> String {
    if color.a == 255 {
        format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
    } else {
        format!(
            "rgba({},{},{},{:.3})",
            color.r,
            color.g,
            color.b,
            color.a as f32 / 255.0
        )
    }
}

fn format_px(value: f32) -> String {
    if value.fract() == 0.0 {
        format!("{}px", value as i32)
    } else {
        format!("{value}px")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use a2ui_core::ComponentId;

    fn contract_style() -> ComponentStyle {
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
        }
    }

    #[test]
    fn test_render_text_body() {
        let html = HtmlBuilder.render(&RenderableHtmlWidget::Text {
            id: ComponentId::new("t1").unwrap(),
            text: "Hello Web".to_string(),
            variant: "body".to_string(),
        });
        assert!(html.contains("Hello Web"));
        assert!(html.contains("<p"));
    }

    #[test]
    fn test_render_text_caption() {
        let html = HtmlBuilder.render(&RenderableHtmlWidget::Text {
            id: ComponentId::new("t1").unwrap(),
            text: "Caption text".to_string(),
            variant: "caption".to_string(),
        });
        assert!(html.contains("Caption text"));
        assert!(html.contains("<span"));
        assert!(html.contains("a2ui-caption"));
    }

    #[test]
    fn test_render_styled_text_css() {
        let html = HtmlBuilder.render(&RenderableHtmlWidget::Styled {
            widget: Box::new(RenderableHtmlWidget::Text {
                id: ComponentId::new("t1").unwrap(),
                text: "Styled text".to_string(),
                variant: "body".to_string(),
            }),
            style: contract_style(),
        });

        assert!(html.contains("style=\""));
        assert!(html.contains("font-size:18px"));
        assert!(html.contains("font-weight:700"));
        assert!(html.contains("color:#112233"));
    }

    #[test]
    fn test_render_button_primary() {
        let html = HtmlBuilder.render(&RenderableHtmlWidget::Button {
            id: ComponentId::new("btn1").unwrap(),
            label: "Click".to_string(),
            variant: "primary".to_string(),
        });
        assert!(html.contains("Click"));
        assert!(html.contains("<button"));
        assert!(html.contains("a2ui-primary"));
    }

    #[test]
    fn test_render_button_default() {
        let html = HtmlBuilder.render(&RenderableHtmlWidget::Button {
            id: ComponentId::new("btn2").unwrap(),
            label: "Default".to_string(),
            variant: "default".to_string(),
        });
        assert!(html.contains("Default"));
        assert!(html.contains("a2ui-default"));
    }

    #[test]
    fn test_render_button_borderless() {
        let html = HtmlBuilder.render(&RenderableHtmlWidget::Button {
            id: ComponentId::new("btn3").unwrap(),
            label: "Borderless".to_string(),
            variant: "borderless".to_string(),
        });
        assert!(html.contains("Borderless"));
        assert!(html.contains("a2ui-borderless"));
    }

    #[test]
    fn test_render_column() {
        let html = HtmlBuilder.render(&RenderableHtmlWidget::Column {
            id: ComponentId::new("col1").unwrap(),
            children: vec![
                RenderableHtmlWidget::Text {
                    id: ComponentId::new("t1").unwrap(),
                    text: "Item 1".to_string(),
                    variant: "body".to_string(),
                },
                RenderableHtmlWidget::Text {
                    id: ComponentId::new("t2").unwrap(),
                    text: "Item 2".to_string(),
                    variant: "body".to_string(),
                },
            ],
        });
        assert!(html.contains("a2ui-column"));
        assert!(html.contains("Item 1"));
        assert!(html.contains("Item 2"));
    }

    #[test]
    fn test_render_styled_layout_css() {
        let html = HtmlBuilder.render(&RenderableHtmlWidget::Styled {
            widget: Box::new(RenderableHtmlWidget::Row {
                id: ComponentId::new("row1").unwrap(),
                children: vec![],
            }),
            style: contract_style(),
        });

        assert!(html.contains("a2ui-row"));
        assert!(html.contains("gap:11px 7px"));
    }

    #[test]
    fn test_render_row() {
        let html = HtmlBuilder.render(&RenderableHtmlWidget::Row {
            id: ComponentId::new("row1").unwrap(),
            children: vec![
                RenderableHtmlWidget::Text {
                    id: ComponentId::new("t1").unwrap(),
                    text: "Left".to_string(),
                    variant: "body".to_string(),
                },
                RenderableHtmlWidget::Text {
                    id: ComponentId::new("t2").unwrap(),
                    text: "Right".to_string(),
                    variant: "body".to_string(),
                },
            ],
        });
        assert!(html.contains("a2ui-row"));
        assert!(html.contains("Left"));
        assert!(html.contains("Right"));
    }

    #[test]
    fn test_render_image() {
        let html = HtmlBuilder.render(&RenderableHtmlWidget::Image {
            id: ComponentId::new("img1").unwrap(),
            url: "https://example.com/img.png".to_string(),
        });
        assert!(html.contains("<img"));
        assert!(html.contains("example.com"));
        assert!(html.contains("a2ui-image"));
    }

    #[test]
    fn test_render_card() {
        let html = HtmlBuilder.render(&RenderableHtmlWidget::Card {
            id: ComponentId::new("card1").unwrap(),
            child: Box::new(RenderableHtmlWidget::Text {
                id: ComponentId::new("inner").unwrap(),
                text: "Card content".to_string(),
                variant: "body".to_string(),
            }),
        });
        assert!(html.contains("a2ui-card"));
        assert!(html.contains("Card content"));
    }

    #[test]
    fn test_render_styled_box_css() {
        let html = HtmlBuilder.render(&RenderableHtmlWidget::Styled {
            widget: Box::new(RenderableHtmlWidget::Card {
                id: ComponentId::new("card1").unwrap(),
                child: Box::new(RenderableHtmlWidget::Text {
                    id: ComponentId::new("inner").unwrap(),
                    text: "Card content".to_string(),
                    variant: "body".to_string(),
                }),
            }),
            style: contract_style(),
        });

        assert!(html.contains("background-color:rgba(68,85,102,0.502)"));
        assert!(html.contains("padding:9px"));
        assert!(html.contains("border-radius:5px"));
    }

    #[test]
    fn test_render_checkbox_checked() {
        let html = HtmlBuilder.render(&RenderableHtmlWidget::CheckBox {
            id: ComponentId::new("cb1").unwrap(),
            checked: true,
            label: "Accept".to_string(),
        });
        assert!(html.contains("checked"));
        assert!(html.contains("Accept"));
        assert!(html.contains("a2ui-checkbox"));
    }

    #[test]
    fn test_render_checkbox_unchecked() {
        let html = HtmlBuilder.render(&RenderableHtmlWidget::CheckBox {
            id: ComponentId::new("cb2").unwrap(),
            checked: false,
            label: "Decline".to_string(),
        });
        assert!(!html.contains("checked"));
        assert!(html.contains("Decline"));
    }

    #[test]
    fn test_render_divider() {
        let html = HtmlBuilder.render(&RenderableHtmlWidget::Divider {
            id: ComponentId::new("div1").unwrap(),
        });
        assert!(html.contains("<hr"));
        assert!(html.contains("a2ui-divider"));
    }

    #[test]
    fn test_render_icon() {
        let html = HtmlBuilder.render(&RenderableHtmlWidget::Icon {
            id: ComponentId::new("icon1").unwrap(),
            name: "star".to_string(),
        });
        assert!(html.contains("a2ui-icon-star"));
        assert!(html.contains("aria-label"));
    }

    #[test]
    fn test_render_list() {
        let html = HtmlBuilder.render(&RenderableHtmlWidget::List {
            id: ComponentId::new("list1").unwrap(),
            children: vec![
                RenderableHtmlWidget::Text {
                    id: ComponentId::new("i1").unwrap(),
                    text: "Item A".to_string(),
                    variant: "body".to_string(),
                },
                RenderableHtmlWidget::Text {
                    id: ComponentId::new("i2").unwrap(),
                    text: "Item B".to_string(),
                    variant: "body".to_string(),
                },
            ],
        });
        assert!(html.contains("<ul"));
        assert!(html.contains("<li>"));
        assert!(html.contains("Item A"));
        assert!(html.contains("Item B"));
    }

    #[test]
    fn test_render_styled_list_css() {
        let html = HtmlBuilder.render(&RenderableHtmlWidget::Styled {
            widget: Box::new(RenderableHtmlWidget::List {
                id: ComponentId::new("list1").unwrap(),
                children: vec![],
            }),
            style: contract_style(),
        });

        assert!(html.contains("display:flex"));
        assert!(html.contains("flex-direction:column"));
        assert!(html.contains("gap:11px 7px"));
    }

    #[test]
    fn test_render_tabs() {
        let html = HtmlBuilder.render(&RenderableHtmlWidget::Tabs {
            id: ComponentId::new("tabs1").unwrap(),
            tabs: vec![
                (
                    "Tab A".to_string(),
                    vec![RenderableHtmlWidget::Text {
                        id: ComponentId::new("c1").unwrap(),
                        text: "Content A".to_string(),
                        variant: "body".to_string(),
                    }],
                ),
                (
                    "Tab B".to_string(),
                    vec![RenderableHtmlWidget::Text {
                        id: ComponentId::new("c2").unwrap(),
                        text: "Content B".to_string(),
                        variant: "body".to_string(),
                    }],
                ),
            ],
        });
        assert!(html.contains("Tab A"));
        assert!(html.contains("Tab B"));
        assert!(html.contains("a2ui-tab-headers"));
        assert!(html.contains("a2ui-tab-content"));
    }

    #[test]
    fn test_render_modal() {
        let html = HtmlBuilder.render(&RenderableHtmlWidget::Modal {
            id: ComponentId::new("modal1").unwrap(),
            title: "Confirm".to_string(),
            content: Box::new(RenderableHtmlWidget::Text {
                id: ComponentId::new("msg").unwrap(),
                text: "Are you sure?".to_string(),
                variant: "body".to_string(),
            }),
        });
        assert!(html.contains("Confirm"));
        assert!(html.contains("Are you sure?"));
        assert!(html.contains("a2ui-modal-header"));
        assert!(html.contains("a2ui-modal-body"));
    }

    #[test]
    fn test_render_slider() {
        let html = HtmlBuilder.render(&RenderableHtmlWidget::Slider {
            id: ComponentId::new("sl1").unwrap(),
            value: 50.0,
            min: 0.0,
            max: 100.0,
        });
        assert!(html.contains("type=\"range\""));
        assert!(html.contains("value=\"50\""));
        assert!(html.contains("min=\"0\""));
        assert!(html.contains("max=\"100\""));
    }

    #[test]
    fn test_render_textfield() {
        let html = HtmlBuilder.render(&RenderableHtmlWidget::TextField {
            id: ComponentId::new("tf1").unwrap(),
            value: "Hello".to_string(),
            placeholder: "Enter text".to_string(),
        });
        assert!(html.contains("type=\"text\""));
        assert!(html.contains("value=\"Hello\""));
        assert!(html.contains("placeholder=\"Enter text\""));
    }

    #[test]
    fn test_render_choicepicker() {
        let option = |label: &str, value: &str| a2ui_renderer::ChoiceOption {
            label: label.to_string(),
            value: value.to_string(),
        };
        let html = HtmlBuilder.render(&RenderableHtmlWidget::ChoicePicker {
            id: ComponentId::new("cp1").unwrap(),
            options: vec![option("A", "A"), option("B", "B"), option("C", "C")],
            selected: vec!["A".to_string()],
            variant: None,
        });
        assert!(html.contains("a2ui-choicepicker"));
        assert!(html.contains("checked"));
    }

    #[test]
    fn test_render_choicepicker_single_select_renders_radio_group() {
        // 规范默认 mutuallyExclusive → radio 组；宿主经 data-a2ui-component-id
        // 与 input 的 name/value 把 DOM 事件桥接为 ChoiceSelect
        let html = HtmlBuilder.render(&RenderableHtmlWidget::ChoicePicker {
            id: ComponentId::new("cp2").unwrap(),
            options: vec![
                a2ui_renderer::ChoiceOption {
                    label: "Email".to_string(),
                    value: "email".to_string(),
                },
                a2ui_renderer::ChoiceOption {
                    label: "SMS".to_string(),
                    value: "sms".to_string(),
                },
            ],
            selected: vec!["email".to_string()],
            variant: None,
        });
        assert!(
            html.contains("data-a2ui-component-id=\"cp2\""),
            "got: {html}"
        );
        assert!(
            html.contains("<input type=\"radio\" name=\"cp2\" value=\"email\" checked />"),
            "got: {html}"
        );
        assert!(
            html.contains("<input type=\"radio\" name=\"cp2\" value=\"sms\" />"),
            "got: {html}"
        );
        assert!(html.contains("Email") && html.contains("SMS"));
    }

    #[test]
    fn test_render_choicepicker_multiple_selection_renders_checkboxes() {
        let html = HtmlBuilder.render(&RenderableHtmlWidget::ChoicePicker {
            id: ComponentId::new("cp3").unwrap(),
            options: vec![a2ui_renderer::ChoiceOption {
                label: "Email".to_string(),
                value: "email".to_string(),
            }],
            selected: vec![],
            variant: Some("multipleSelection".to_string()),
        });
        assert!(
            html.contains("<input type=\"checkbox\" name=\"cp3\" value=\"email\" />"),
            "got: {html}"
        );
    }

    #[test]
    fn test_render_datetimeinput() {
        let html = HtmlBuilder.render(&RenderableHtmlWidget::DateTimeInput {
            id: ComponentId::new("dt1").unwrap(),
            label: "Pick date".to_string(),
        });
        assert!(html.contains("Pick date"));
        assert!(html.contains("datetime-local"));
    }

    #[test]
    fn test_render_video() {
        let html = HtmlBuilder.render(&RenderableHtmlWidget::Video {
            id: ComponentId::new("vid1").unwrap(),
            url: "https://example.com/video.mp4".to_string(),
        });
        assert!(html.contains("<video"));
        assert!(html.contains("controls"));
        assert!(html.contains("example.com"));
    }

    #[test]
    fn test_render_audioplayer() {
        let html = HtmlBuilder.render(&RenderableHtmlWidget::AudioPlayer {
            id: ComponentId::new("aud1").unwrap(),
            url: "https://example.com/audio.mp3".to_string(),
        });
        assert!(html.contains("<audio"));
        assert!(html.contains("controls"));
        assert!(html.contains("example.com"));
    }

    #[test]
    fn test_render_placeholder() {
        let html = HtmlBuilder.render(&RenderableHtmlWidget::Placeholder {
            id: ComponentId::new("unk1").unwrap(),
            reason: "unknown component type: Foo".to_string(),
        });
        assert!(html.contains("a2ui-placeholder"));
        assert!(html.contains("unknown component type"));
    }

    #[test]
    fn test_html_escape_text() {
        let html = HtmlBuilder.render(&RenderableHtmlWidget::Text {
            id: ComponentId::new("t1").unwrap(),
            text: "<script>alert('xss')</script>".to_string(),
            variant: "body".to_string(),
        });
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("&#x27;"));
    }

    #[test]
    fn test_html_attr_escape() {
        let html = HtmlBuilder.render(&RenderableHtmlWidget::Image {
            id: ComponentId::new("img1").unwrap(),
            url: "\" onerror=\"alert('xss')\"".to_string(),
        });
        // The double quotes should be escaped to &quot; to prevent attribute injection
        assert!(html.contains("&quot;"));
        // The src attribute should not be breakable — no unescaped quotes
        assert!(!html.contains("src=\"\""));
    }

    #[test]
    fn test_render_page_full() {
        let html = HtmlBuilder.render_page("<p>Hello</p>", "My App");
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<title>My App</title>"));
        assert!(html.contains("Hello"));
        assert!(html.contains("a2ui-body"));
        assert!(html.contains("</html>"));
    }

    #[test]
    fn test_render_page_escapes_title() {
        let html = HtmlBuilder.render_page("<p>Body</p>", "<script>alert('xss')</script>");
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
    }
}
