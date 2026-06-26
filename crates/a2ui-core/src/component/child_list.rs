use crate::component::ComponentId;
use serde::Deserialize;

/// ChildList: 两种模式
/// - Array: 固定子组件列表
/// - Object: 动态模板（从 Data Model 数组生成）
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum ChildList {
    Array {
        #[serde(rename = "children")]
        list: Vec<ComponentId>,
    },
    Object {
        template: ComponentId,
        path: String,
    },
}

impl ChildList {
    /// 从 Vec<ComponentId> 创建 Array 模式
    pub fn array(children: Vec<ComponentId>) -> Self {
        Self::Array { list: children }
    }

    /// 创建 Object 模板模式
    pub fn object(template: ComponentId, path: impl Into<String>) -> Self {
        Self::Object {
            template,
            path: path.into(),
        }
    }

    /// 获取所有子组件 ID（Array 模式）
    pub fn component_ids(&self) -> Box<dyn Iterator<Item = &ComponentId> + '_> {
        match self {
            ChildList::Array { list } => Box::new(list.iter()),
            ChildList::Object { .. } => Box::new(std::iter::empty()),
        }
    }

    /// 获取模板组件 ID（Object 模式）
    pub fn template_id(&self) -> Option<&str> {
        match self {
            ChildList::Object { template, .. } => Some(template.as_str()),
            ChildList::Array { .. } => None,
        }
    }

    /// 获取数据路径（Object 模式）
    pub fn data_path(&self) -> Option<&str> {
        match self {
            ChildList::Object { path, .. } => Some(path.as_str()),
            ChildList::Array { .. } => None,
        }
    }
}

// Custom Serialize to ensure Array serializes as {"children": [...]}
impl serde::Serialize for ChildList {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            ChildList::Array { list } => {
                let ids: Vec<&str> = list.iter().map(|c| c.as_str()).collect();
                let map = serde_json::json!({"children": ids});
                map.serialize(serializer)
            }
            ChildList::Object { template, path } => {
                let map = serde_json::json!({
                    "template": template.as_str(),
                    "path": path
                });
                map.serialize(serializer)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_child_list_array() {
        let cl = ChildList::Array { list: vec![
            ComponentId::new("child1").unwrap(),
            ComponentId::new("child2").unwrap(),
        ]};
        let ids: Vec<_> = cl.component_ids().collect();
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn test_child_list_object() {
        let cl = ChildList::Object {
            template: ComponentId::new("item_template").unwrap(),
            path: "/items".to_string(),
        };
        assert_eq!(cl.template_id(), Some("item_template"));
        assert_eq!(cl.data_path(), Some("/items"));
    }

    #[test]
    fn test_child_list_array_serialize() {
        let cl = ChildList::Array { list: vec![
            ComponentId::new("a").unwrap(),
            ComponentId::new("b").unwrap(),
        ]};
        let json = serde_json::to_value(&cl).unwrap();
        assert_eq!(json["children"][0], "a");
        assert_eq!(json["children"][1], "b");
    }

    #[test]
    fn test_child_list_object_serialize() {
        let cl = ChildList::Object {
            template: ComponentId::new("template").unwrap(),
            path: "/items".to_string(),
        };
        let json = serde_json::to_value(&cl).unwrap();
        assert_eq!(json["template"], "template");
        assert_eq!(json["path"], "/items");
    }
}
