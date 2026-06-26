use a2ui_core::prelude::*;

/// 焦点管理器：管理可聚焦组件的焦点状态
#[derive(Debug, Clone, Default)]
pub struct FocusManager {
    /// 可聚焦组件列表（按渲染顺序）
    focusable: Vec<ComponentId>,
    /// 当前焦点索引（-1 表示无焦点）
    current_index: isize,
}

impl FocusManager {
    /// 创建新的焦点管理器
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置可聚焦组件列表
    pub fn set_focusable(&mut self, components: Vec<ComponentId>) {
        self.focusable = components;
        self.current_index = if self.focusable.is_empty() { -1 } else { 0 };
    }

    /// 获取当前聚焦的组件
    pub fn current(&self) -> Option<&ComponentId> {
        if self.current_index >= 0 && (self.current_index as usize) < self.focusable.len() {
            self.focusable.get(self.current_index as usize)
        } else {
            None
        }
    }

    /// 循环移动到下一个可聚焦组件
    pub fn next(&mut self) {
        if self.focusable.is_empty() {
            return;
        }
        self.current_index = (self.current_index + 1) % self.focusable.len() as isize;
    }

    /// 循环移动到上一个可聚焦组件
    pub fn previous(&mut self) {
        if self.focusable.is_empty() {
            return;
        }
        self.current_index = if self.current_index <= 0 {
            (self.focusable.len() - 1) as isize
        } else {
            self.current_index - 1
        };
    }

    /// 检查是否有可聚焦组件
    pub fn has_focusable(&self) -> bool {
        !self.focusable.is_empty()
    }

    /// 获取可聚焦组件数量
    pub fn focusable_count(&self) -> usize {
        self.focusable.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_focus_manager() {
        let fm = FocusManager::new();
        assert!(fm.current().is_none());
    }

    #[test]
    fn test_set_focusable() {
        let mut fm = FocusManager::new();
        fm.set_focusable(vec![
            ComponentId::new("btn1").unwrap(),
            ComponentId::new("btn2").unwrap(),
        ]);
        assert_eq!(fm.focusable_count(), 2);
        assert!(fm.current().is_some());
    }

    #[test]
    fn test_next_focus() {
        let mut fm = FocusManager::new();
        fm.set_focusable(vec![
            ComponentId::new("a").unwrap(),
            ComponentId::new("b").unwrap(),
            ComponentId::new("c").unwrap(),
        ]);
        assert_eq!(fm.current().unwrap().as_str(), "a");
        fm.next();
        assert_eq!(fm.current().unwrap().as_str(), "b");
        fm.next();
        assert_eq!(fm.current().unwrap().as_str(), "c");
        fm.next();
        assert_eq!(fm.current().unwrap().as_str(), "a"); // wrap
    }

    #[test]
    fn test_previous_focus() {
        let mut fm = FocusManager::new();
        fm.set_focusable(vec![
            ComponentId::new("a").unwrap(),
            ComponentId::new("b").unwrap(),
        ]);
        fm.next();
        assert_eq!(fm.current().unwrap().as_str(), "b");
        fm.previous();
        assert_eq!(fm.current().unwrap().as_str(), "a");
        fm.previous();
        assert_eq!(fm.current().unwrap().as_str(), "b"); // wrap
    }

    #[test]
    fn test_empty_focusable() {
        let mut fm = FocusManager::new();
        fm.set_focusable(vec![]);
        assert!(fm.current().is_none());
        fm.next();
        fm.previous();
        assert!(fm.current().is_none());
    }
}
