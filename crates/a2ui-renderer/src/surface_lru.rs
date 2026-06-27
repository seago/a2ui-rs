/// Surface LRU 驱逐管理器
///
/// 追踪每个 surface 的最后访问时间，支持：
/// - 基于数量的 LRU 驱逐（超出上限时驱逐最久未用）
/// - 基于空闲超时的自动驱逐（可选）
use std::collections::HashMap;
use std::time::Instant;

/// Surface LRU 驱逐管理器
#[derive(Debug)]
pub struct SurfaceLru {
    /// 最大 Surface 数量
    max_surfaces: usize,
    /// 空闲超时（None 表示不禁用基于时间的驱逐）
    idle_timeout: Option<std::time::Duration>,
    /// surface_id → 最后访问时间
    last_access: HashMap<String, std::time::Instant>,
}

impl SurfaceLru {
    /// 创建新的 LRU 管理器
    ///
    /// `max_surfaces`: 最大并发 Surface 数
    /// `idle_timeout`: 可选空闲超时
    ///
    /// # 示例
    ///
    /// ```rust
    /// use a2ui_renderer::SurfaceLru;
    /// use std::time::Duration;
    ///
    /// let lru = SurfaceLru::new(100, Some(Duration::from_secs(600)));
    /// assert!(lru.is_empty());
    /// ```
    pub fn new(max_surfaces: usize, idle_timeout: Option<std::time::Duration>) -> Self {
        Self {
            max_surfaces,
            idle_timeout,
            last_access: HashMap::new(),
        }
    }

    /// 记录对某个 surface 的访问（更新其最后访问时间）
    ///
    /// # 示例
    ///
    /// ```rust
    /// use a2ui_renderer::SurfaceLru;
    ///
    /// let mut lru = SurfaceLru::new(100, None);
    /// lru.touch("s1");
    /// assert_eq!(lru.len(), 1);
    /// ```
    pub fn touch(&mut self, surface_id: &str) {
        self.last_access
            .insert(surface_id.to_string(), Instant::now());
    }

    /// 检查是否需要驱逐。
    ///
    /// `current_count`: 当前 surface 数量
    /// 返回应被驱逐的 surface_id（如果存在）
    ///
    /// 驱逐策略：
    /// 1. 先检查是否有 surface 的空闲时间超过 `idle_timeout`
    /// 2. 如果数量仍超限则驱逐 LRU（最久未访问）
    ///
    /// # 示例
    ///
    /// ```rust
    /// use a2ui_renderer::SurfaceLru;
    ///
    /// let mut lru = SurfaceLru::new(2, None);
    /// lru.touch("s1");
    /// lru.touch("s2");
    /// // 未超出上限
    /// assert_eq!(lru.find_victim(2), None);
    /// // 超出上限
    /// assert_eq!(lru.find_victim(3), Some("s1".to_string()));
    /// ```
    pub fn find_victim(&mut self, current_count: usize) -> Option<String> {
        // 1. 先检查空闲超时
        if let Some(timeout) = self.idle_timeout {
            let mut oldest_idle: Option<(String, Instant)> = None;
            for (sid, last) in &self.last_access {
                if last.elapsed() >= timeout {
                    match &oldest_idle {
                        Some((_, oldest_time)) if last > oldest_time => {}
                        _ => {
                            oldest_idle = Some((sid.clone(), *last));
                        }
                    }
                }
            }
            if let Some((victim, _)) = oldest_idle {
                return Some(victim);
            }
        }

        // 2. 如果数量仍超限，驱逐 LRU（最久未访问）
        if current_count > self.max_surfaces {
            let victim = self
                .last_access
                .iter()
                .min_by_key(|(_, &time)| time)
                .map(|(sid, _)| sid.clone());
            return victim;
        }

        None
    }

    /// 移除 surface 的跟踪记录
    ///
    /// 当 surface 被 `delete_surface` 正常销毁时调用。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use a2ui_renderer::SurfaceLru;
    ///
    /// let mut lru = SurfaceLru::new(10, None);
    /// lru.touch("s1");
    /// lru.remove("s1");
    /// assert!(lru.is_empty());
    /// ```
    pub fn remove(&mut self, surface_id: &str) {
        self.last_access.remove(surface_id);
    }

    /// 当前追踪的 surface 数量
    ///
    /// # 示例
    ///
    /// ```rust
    /// use a2ui_renderer::SurfaceLru;
    ///
    /// let mut lru = SurfaceLru::new(10, None);
    /// assert_eq!(lru.len(), 0);
    /// lru.touch("s1");
    /// assert_eq!(lru.len(), 1);
    /// ```
    pub fn len(&self) -> usize {
        self.last_access.len()
    }

    /// 是否为空
    ///
    /// # 示例
    ///
    /// ```rust
    /// use a2ui_renderer::SurfaceLru;
    ///
    /// let lru = SurfaceLru::new(10, None);
    /// assert!(lru.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.last_access.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_surface_lru_new() {
        let lru = SurfaceLru::new(100, None);
        assert!(lru.is_empty());
        assert_eq!(lru.len(), 0);
    }

    #[test]
    fn test_surface_lru_touch_and_find() {
        let mut lru = SurfaceLru::new(3, None);
        lru.touch("s1");
        lru.touch("s2");
        lru.touch("s3");
        assert_eq!(lru.len(), 3);
        // 未超出上限，无需驱逐
        assert_eq!(lru.find_victim(3), None);
        // 超出上限，返回 LRU
        assert_eq!(lru.find_victim(4), Some("s1".to_string()));
    }

    #[test]
    fn test_surface_lru_evicts_oldest() {
        let mut lru = SurfaceLru::new(2, None);
        lru.touch("oldest");
        std::thread::sleep(std::time::Duration::from_millis(10));
        lru.touch("newest");
        // 当前 2 个，上限 2，但要创建第 3 个 → 驱逐最旧的
        let victim = lru.find_victim(3);
        assert_eq!(victim, Some("oldest".to_string()));
    }

    #[test]
    fn test_surface_lru_touch_updates_order() {
        let mut lru = SurfaceLru::new(2, None);
        lru.touch("s1");
        std::thread::sleep(std::time::Duration::from_millis(10));
        lru.touch("s2");
        // 重新访问 s1，s1 变成最新
        std::thread::sleep(std::time::Duration::from_millis(10));
        lru.touch("s1");
        // s2 现在是最旧的
        let victim = lru.find_victim(3);
        assert_eq!(victim, Some("s2".to_string()));
    }

    #[test]
    fn test_surface_lru_idle_timeout() {
        let mut lru = SurfaceLru::new(
            10,
            Some(std::time::Duration::from_millis(50)),
        );
        lru.touch("s1");
        // 立即检查不会超时
        assert_eq!(lru.find_victim(1), None);
        // 等待超时后检查
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert_eq!(lru.find_victim(1), Some("s1".to_string()));
    }

    #[test]
    fn test_surface_lru_remove() {
        let mut lru = SurfaceLru::new(10, None);
        lru.touch("s1");
        lru.touch("s2");
        assert_eq!(lru.len(), 2);
        lru.remove("s1");
        assert_eq!(lru.len(), 1);
        assert!(!lru.is_empty());
        lru.remove("s2");
        assert!(lru.is_empty());
    }

    #[test]
    fn test_surface_lru_no_timeout_returns_none_when_under_limit() {
        let mut lru = SurfaceLru::new(5, None);
        lru.touch("s1");
        lru.touch("s2");
        assert_eq!(lru.find_victim(2), None);
    }
}
