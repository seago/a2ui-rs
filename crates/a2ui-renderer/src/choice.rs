//! ChoicePicker 选择语义：点击某选项后计算新选中集的纯函数。
//!
//! 规范 basic catalog 的 `variant` 决定行为：`mutuallyExclusive`（默认）
//! 单选整体替换；`multipleSelection` 多选切换成员。逻辑收编在公共层，
//! 平台渲染器只负责把点击事件映射到 [`toggle_choice`] 调用。

/// ChoicePicker `variant` 的规范取值：多选。
pub const VARIANT_MULTIPLE_SELECTION: &str = "multipleSelection";
/// ChoicePicker `variant` 的规范取值：单选（规范默认值）。
pub const VARIANT_MUTUALLY_EXCLUSIVE: &str = "mutuallyExclusive";

/// 计算点击某选项后的新选中集。
///
/// - `mutuallyExclusive`（`variant` 缺失或取值非法时的规范默认）：整体
///   替换为 `[clicked]`。
/// - `multipleSelection`：`clicked` 已在集合中则移除、否则追加，保持
///   既有顺序。
///
/// # 示例
///
/// ```rust
/// use a2ui_renderer::choice::toggle_choice;
///
/// // 单选（默认）：整体替换
/// assert_eq!(toggle_choice(&["email".into()], "sms", None), vec!["sms"]);
/// // 多选：切换成员
/// assert_eq!(
///     toggle_choice(&["email".into()], "sms", Some("multipleSelection")),
///     vec!["email", "sms"]
/// );
/// ```
pub fn toggle_choice(current: &[String], clicked: &str, variant: Option<&str>) -> Vec<String> {
    if variant == Some(VARIANT_MULTIPLE_SELECTION) {
        if current.iter().any(|v| v == clicked) {
            current.iter().filter(|v| *v != clicked).cloned().collect()
        } else {
            let mut next = current.to_vec();
            next.push(clicked.to_string());
            next
        }
    } else {
        vec![clicked.to_string()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selected(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn mutually_exclusive_replaces_selection() {
        // 显式声明与规范默认（None）行为一致
        for variant in [Some(VARIANT_MUTUALLY_EXCLUSIVE), None] {
            assert_eq!(
                toggle_choice(&selected(&["email"]), "sms", variant),
                selected(&["sms"])
            );
            // 点击已选中项保持选中（单选不反选）
            assert_eq!(
                toggle_choice(&selected(&["email"]), "email", variant),
                selected(&["email"])
            );
            // 空集起步
            assert_eq!(toggle_choice(&[], "email", variant), selected(&["email"]));
        }
    }

    #[test]
    fn multiple_selection_toggles_membership_preserving_order() {
        let variant = Some(VARIANT_MULTIPLE_SELECTION);
        // 未选中 → 追加到尾部
        assert_eq!(
            toggle_choice(&selected(&["a", "b"]), "c", variant),
            selected(&["a", "b", "c"])
        );
        // 已选中 → 移除，其余保持顺序
        assert_eq!(
            toggle_choice(&selected(&["a", "b", "c"]), "b", variant),
            selected(&["a", "c"])
        );
        // 最后一项也可移除为空集
        assert_eq!(
            toggle_choice(&selected(&["a"]), "a", variant),
            selected(&[])
        );
    }

    #[test]
    fn unknown_variant_falls_back_to_mutually_exclusive() {
        // 规范：variant 枚举外取值按默认 mutuallyExclusive 处理
        assert_eq!(
            toggle_choice(&selected(&["a", "b"]), "c", Some("chips")),
            selected(&["c"])
        );
    }
}
