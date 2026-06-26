use serde::{Deserialize, Serialize};

/// 子组件列表
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChildList {
    pub children: Vec<String>,
}

impl ChildList {
    pub fn new(children: Vec<String>) -> Self {
        Self { children }
    }
}
