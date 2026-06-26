use crate::WidgetMapper;
use a2ui_core::prelude::*;
use a2ui_renderer::component_forest::ComponentTreeNode;
use a2ui_renderer::{ComponentForest, DataBinding};
use ratatui::layout::Rect;

/// 渲染目标 widget（类型抹平，用于渲染管线）
#[derive(Debug, Clone)]
pub enum RenderableWidget {
    Paragraph {
        id: ComponentId,
        area: Rect,
        text: String,
    },
    Block {
        id: ComponentId,
        area: Rect,
        title: String,
    },
    Placeholder {
        id: ComponentId,
        area: Rect,
        reason: String,
    },
    TextField {
        id: ComponentId,
        area: Rect,
        value: String,
        placeholder: String,
    },
    CheckBox {
        id: ComponentId,
        area: Rect,
        label: String,
        checked: bool,
    },
    Slider {
        id: ComponentId,
        area: Rect,
        value: f64,
        min: f64,
        max: f64,
    },
}

impl RenderableWidget {
    pub fn id(&self) -> &ComponentId {
        match self {
            Self::Paragraph { id, .. } => id,
            Self::Block { id, .. } => id,
            Self::Placeholder { id, .. } => id,
            Self::TextField { id, .. } => id,
            Self::CheckBox { id, .. } => id,
            Self::Slider { id, .. } => id,
        }
    }

    pub fn area(&self) -> Rect {
        match self {
            Self::Paragraph { area, .. } => *area,
            Self::Block { area, .. } => *area,
            Self::Placeholder { area, .. } => *area,
            Self::TextField { area, .. } => *area,
            Self::CheckBox { area, .. } => *area,
            Self::Slider { area, .. } => *area,
        }
    }
}

/// 将组件森林构建为渲染目标列表
pub struct WidgetBuilder<'a> {
    mapper: &'a WidgetMapper,
    binding: &'a DataBinding,
    forest: &'a ComponentForest,
}

impl<'a> WidgetBuilder<'a> {
    pub fn new(
        mapper: &'a WidgetMapper,
        binding: &'a DataBinding,
        forest: &'a ComponentForest,
    ) -> Self {
        Self {
            mapper,
            binding,
            forest,
        }
    }

    /// 从指定 Surface 的根组件开始构建 widget 树
    pub fn build_tree(&self, surface_id: &str, area: Rect) -> Vec<RenderableWidget> {
        let tree = match self.forest.build_tree(surface_id) {
            Ok(t) => t,
            Err(_) => return vec![],
        };

        let mut widgets = Vec::new();
        self.flatten_node(&tree, area, &mut widgets);
        widgets
    }

    /// 递归展平组件树为 RenderableWidget 列表（前序遍历：容器先于子元素）
    fn flatten_node(
        &self,
        node: &ComponentTreeNode,
        area: Rect,
        widgets: &mut Vec<RenderableWidget>,
    ) {
        let comp = &node.component;

        // 生成当前组件的 widget
        widgets.push(self.component_to_widget(comp, area));

        // 获取子组件 ID 列表（从组件属性解析）
        let child_ids = self.get_child_ids(comp);
        let children = &node.children;

        // 为缺失的组件创建占位符
        for missing_id in &child_ids {
            if !children
                .iter()
                .any(|c| c.component.id().as_str() == missing_id.as_str())
            {
                widgets.push(RenderableWidget::Placeholder {
                    id: missing_id.clone(),
                    area,
                    reason: format!("component not found: {}", missing_id),
                });
            }
        }

        if children.is_empty() {
            return;
        }

        // 计算子组件区域
        let child_areas = self.layout_children(comp, children, area);
        for (child_node, child_area) in children.iter().zip(child_areas) {
            self.flatten_node(child_node, child_area, widgets);
        }
    }

    /// 从组件属性中提取子组件 ID 列表
    fn get_child_ids(&self, component: &Component) -> Vec<ComponentId> {
        let props = component.properties();
        let mut ids = Vec::new();

        // 检查 children 属性
        if let Some(children_val) = props.get("children") {
            if let Some(children_obj) = children_val.as_object() {
                if let Some(ids_val) = children_obj.get("children") {
                    if let Some(ids_arr) = ids_val.as_array() {
                        for id_val in ids_arr {
                            if let Some(id_str) = id_val.as_str() {
                                if let Ok(id) = ComponentId::new(id_str) {
                                    ids.push(id);
                                }
                            }
                        }
                    }
                }
            }
        }

        // 检查单个 child 属性
        if let Some(child_str) = props.get("child").and_then(|v| v.as_str()) {
            if let Ok(id) = ComponentId::new(child_str) {
                ids.push(id);
            }
        }

        ids
    }

    /// 将单个组件转换为 RenderableWidget
    fn component_to_widget(&self, component: &Component, area: Rect) -> RenderableWidget {
        let ctype = component.component_type();

        match ctype {
            "Column" | "Row" | "List" => RenderableWidget::Block {
                id: component.id().clone(),
                area,
                title: format!("[{}]", ctype),
            },
            "TextField" => {
                let props = component.properties();
                let value = props
                    .get("value")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let placeholder = props
                    .get("placeholder")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                RenderableWidget::TextField {
                    id: component.id().clone(),
                    area,
                    value,
                    placeholder,
                }
            }
            "CheckBox" => {
                let props = component.properties();
                let label = props
                    .get("label")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let checked = props
                    .get("checked")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                RenderableWidget::CheckBox {
                    id: component.id().clone(),
                    area,
                    label,
                    checked,
                }
            }
            "Slider" => {
                let props = component.properties();
                let value = props
                    .get("value")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let min = props
                    .get("min")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let max = props
                    .get("max")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(100.0);
                RenderableWidget::Slider {
                    id: component.id().clone(),
                    area,
                    value,
                    min,
                    max,
                }
            }
            _ => {
                // 尝试提取文本
                let text = self.mapper.extract_text(component);
                if text.starts_with('[') && text.ends_with(']') {
                    // 未知组件类型，渲染为占位符
                    RenderableWidget::Placeholder {
                        id: component.id().clone(),
                        area,
                        reason: format!("unknown component type: {}", ctype),
                    }
                } else {
                    RenderableWidget::Paragraph {
                        id: component.id().clone(),
                        area,
                        text,
                    }
                }
            }
        }
    }

    /// 根据组件类型为子组件分配渲染区域
    fn layout_children(
        &self,
        parent: &Component,
        children: &[ComponentTreeNode],
        area: Rect,
    ) -> Vec<Rect> {
        let ctype = parent.component_type();
        let count = children.len();

        if count == 0 {
            return vec![];
        }

        match ctype {
            "Column" => {
                let height = area.height / count as u16;
                let mut areas = Vec::new();
                for i in 0..count {
                    areas.push(Rect::new(
                        area.x,
                        area.y + i as u16 * height,
                        area.width,
                        height,
                    ));
                }
                areas
            }
            "Row" => {
                let width = area.width / count as u16;
                let mut areas = Vec::new();
                for i in 0..count {
                    areas.push(Rect::new(
                        area.x + i as u16 * width,
                        area.y,
                        width,
                        area.height,
                    ));
                }
                areas
            }
            _ => {
                // 默认：均分区域
                let area_per_child = Rect::new(area.x, area.y, area.width, area.height);
                vec![area_per_child; count]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_build_widget_tree_from_components() {
        let mut forest = ComponentForest::new();
        let binding = DataBinding::new(DataModel::new(json!({"title": "Hello"})));
        let mapper = WidgetMapper;

        // Column 作为根组件（ID 必须为 "root" 才能被 forest 识别）
        let root = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("title").unwrap()],
        );
        let title = Component::text(
            ComponentId::new("title").unwrap(),
            DynamicValue::Path {
                path: "/title".into(),
            },
        );

        forest.upsert("s1", root).unwrap();
        forest.upsert("s1", title).unwrap();

        let builder = WidgetBuilder::new(&mapper, &binding, &forest);
        let widgets = builder.build_tree("s1", Rect::new(0, 0, 80, 24));

        // 至少有一个 widget（root Column + title Text）
        assert!(widgets.len() >= 2);
        // root 应为 Block（Column 类型）
        assert_eq!(widgets[0].id().as_str(), "root");
        // title 应能找到
        let title_widget = widgets.iter().find(|w| w.id().as_str() == "title");
        assert!(title_widget.is_some());
    }

    #[test]
    fn test_missing_component_renders_placeholder() {
        let mut forest = ComponentForest::new();
        let binding = DataBinding::new(DataModel::empty());
        let mapper = WidgetMapper;

        let root = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("missing").unwrap()],
        );
        forest.upsert("s1", root).unwrap();

        let builder = WidgetBuilder::new(&mapper, &binding, &forest);
        let widgets = builder.build_tree("s1", Rect::new(0, 0, 80, 24));

        // missing 组件应渲染为占位符
        let placeholder = widgets.iter().find(|w| w.id().as_str() == "missing");
        assert!(placeholder.is_some());
        if let Some(RenderableWidget::Placeholder { reason, .. }) = placeholder {
            assert!(reason.contains("not found"));
        }
    }

    #[test]
    fn test_widget_area_assignment() {
        let mut forest = ComponentForest::new();
        let binding = DataBinding::new(DataModel::empty());
        let mapper = WidgetMapper;

        let root = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("child").unwrap()],
        );
        forest.upsert("s1", root).unwrap();

        let builder = WidgetBuilder::new(&mapper, &binding, &forest);
        let widgets = builder.build_tree("s1", Rect::new(10, 20, 80, 24));

        // root widget 应获得完整区域
        let root_widget = widgets.iter().find(|w| w.id().as_str() == "root");
        assert!(root_widget.is_some());
        assert_eq!(root_widget.unwrap().area(), Rect::new(10, 20, 80, 24));
    }

    // --- P3-3: TextField/CheckBox/Slider widget mapping tests ---

    #[test]
    fn test_textfield_component_maps_to_textfield_widget() {
        let mut forest = ComponentForest::new();
        let binding = DataBinding::new(DataModel::empty());
        let mapper = WidgetMapper;

        let root = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("name_input").unwrap()],
        );
        let tf: Component = serde_json::from_str(
            r#"{"id":"name_input","component":"TextField","value":"Alice","placeholder":"Enter name"}"#
        ).unwrap();
        forest.upsert("s1", root).unwrap();
        forest.upsert("s1", tf).unwrap();

        let builder = WidgetBuilder::new(&mapper, &binding, &forest);
        let widgets = builder.build_tree("s1", Rect::new(0, 0, 80, 24));

        let tf_widget = widgets.iter().find(|w| w.id().as_str() == "name_input");
        assert!(tf_widget.is_some(), "TextField widget should exist in tree");
        assert!(matches!(tf_widget.unwrap(), RenderableWidget::TextField { .. }),
            "TextField component should produce RenderableWidget::TextField");
    }

    #[test]
    fn test_checkbox_component_maps_to_checkbox_widget() {
        let mut forest = ComponentForest::new();
        let binding = DataBinding::new(DataModel::empty());
        let mapper = WidgetMapper;

        let root = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("agree").unwrap()],
        );
        let cb: Component = serde_json::from_str(
            r#"{"id":"agree","component":"CheckBox","checked":true,"label":"I agree"}"#
        ).unwrap();
        forest.upsert("s1", root).unwrap();
        forest.upsert("s1", cb).unwrap();

        let builder = WidgetBuilder::new(&mapper, &binding, &forest);
        let widgets = builder.build_tree("s1", Rect::new(0, 0, 80, 24));

        let cb_widget = widgets.iter().find(|w| w.id().as_str() == "agree");
        assert!(cb_widget.is_some(), "CheckBox widget should exist in tree");
        assert!(matches!(cb_widget.unwrap(), RenderableWidget::CheckBox { .. }),
            "CheckBox component should produce RenderableWidget::CheckBox");
    }

    #[test]
    fn test_slider_component_maps_to_slider_widget() {
        let mut forest = ComponentForest::new();
        let binding = DataBinding::new(DataModel::empty());
        let mapper = WidgetMapper;

        let root = Component::column(
            ComponentId::new("root").unwrap(),
            vec![ComponentId::new("volume").unwrap()],
        );
        let sl: Component = serde_json::from_str(
            r#"{"id":"volume","component":"Slider","value":50,"min":0,"max":100}"#
        ).unwrap();
        forest.upsert("s1", root).unwrap();
        forest.upsert("s1", sl).unwrap();

        let builder = WidgetBuilder::new(&mapper, &binding, &forest);
        let widgets = builder.build_tree("s1", Rect::new(0, 0, 80, 24));

        let sl_widget = widgets.iter().find(|w| w.id().as_str() == "volume");
        assert!(sl_widget.is_some(), "Slider widget should exist in tree");
        assert!(matches!(sl_widget.unwrap(), RenderableWidget::Slider { .. }),
            "Slider component should produce RenderableWidget::Slider");
    }
}
