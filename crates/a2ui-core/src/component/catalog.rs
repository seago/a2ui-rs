use serde::{Deserialize, Serialize};
use crate::component::ComponentId;

/// 组件目录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Catalog {
    pub id: String,
    pub url: String,
    pub components: Vec<ComponentDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentDefinition {
    pub id: ComponentId,
    pub component_type: String,
}
