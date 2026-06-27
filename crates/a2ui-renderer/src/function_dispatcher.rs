use crate::error::RenderResult;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// 函数执行边界
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallableFrom {
    /// 只能在客户端执行
    ClientOnly,
    /// 只能在服务端执行
    RemoteOnly,
    /// 两端均可执行
    ClientOrRemote,
}

/// 函数定义
#[derive(Debug, Clone)]
pub struct FunctionDef {
    pub name: String,
    pub callable_from: CallableFrom,
}

/// 函数处理器类型别名
pub type FunctionHandler = Arc<dyn Fn(Value) -> RenderResult<Value> + Send + Sync>;

/// 函数调度器
#[derive(Default)]
pub struct FunctionDispatcher {
    /// 已注册的函数元数据
    functions: HashMap<String, FunctionDef>,
    /// 函数处理器（闭包）
    handlers: HashMap<String, FunctionHandler>,
}

impl Clone for FunctionDispatcher {
    fn clone(&self) -> Self {
        Self {
            functions: self.functions.clone(),
            handlers: self.handlers.clone(),
        }
    }
}

impl std::fmt::Debug for FunctionDispatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FunctionDispatcher")
            .field("functions", &self.functions)
            .field("handlers_count", &self.handlers.len())
            .finish()
    }
}

impl FunctionDispatcher {
    /// 创建新的调度器，注册内置 Basic Catalog 函数
    pub fn new() -> Self {
        let mut dispatcher = Self::default();

        // 注册 formatNumber
        dispatcher.register_with_handler(
            "formatNumber",
            CallableFrom::ClientOrRemote,
            Arc::new(|args| {
                let value = args.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let decimals = args.get("decimals").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let grouping = args
                    .get("grouping")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                Ok(Value::String(format_number(value, decimals, grouping)))
            }),
        );

        // 注册 formatCurrency
        dispatcher.register_with_handler(
            "formatCurrency",
            CallableFrom::ClientOrRemote,
            Arc::new(|args| {
                let value = args.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let currency = args
                    .get("currency")
                    .and_then(|v| v.as_str())
                    .unwrap_or("USD")
                    .to_string();
                let decimals = args.get("decimals").and_then(|v| v.as_u64()).unwrap_or(2) as usize;
                let grouping = args
                    .get("grouping")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                Ok(Value::String(format_currency(
                    value, &currency, decimals, grouping,
                )))
            }),
        );

        // 注册 formatDate
        dispatcher.register_with_handler(
            "formatDate",
            CallableFrom::ClientOrRemote,
            Arc::new(|args| {
                let value = args
                    .get("value")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let format_str = args
                    .get("format")
                    .and_then(|v| v.as_str())
                    .unwrap_or("YYYY-MM-DD")
                    .to_string();
                Ok(Value::String(format_date(&value, &format_str)))
            }),
        );

        // 注册 pluralize
        dispatcher.register_with_handler(
            "pluralize",
            CallableFrom::ClientOrRemote,
            Arc::new(|args| {
                let value = args.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0) as i64;
                Ok(Value::String(pluralize(value, &args)))
            }),
        );

        dispatcher
    }

    /// 注册函数元数据（不提供执行逻辑）
    pub fn register(&mut self, name: impl Into<String>, callable_from: CallableFrom) {
        let name = name.into();
        self.functions.insert(
            name.clone(),
            FunctionDef {
                name: name.clone(),
                callable_from,
            },
        );
    }

    /// 注册函数并附带执行处理器
    pub fn register_with_handler(
        &mut self,
        name: impl Into<String>,
        callable_from: CallableFrom,
        handler: FunctionHandler,
    ) {
        let name = name.into();
        self.functions.insert(
            name.clone(),
            FunctionDef {
                name: name.clone(),
                callable_from,
            },
        );
        self.handlers.insert(name, handler);
    }

    /// 执行函数调用，强制执行 `callableFrom` 边界检查
    ///
    /// - `caller` 参数指定调用来源（`ClientOnly` / `RemoteOnly` / `ClientOrRemote`）
    /// - `ClientOnly` 函数只能被 `ClientOnly` 调用者执行
    /// - `RemoteOnly` 函数只能被 `RemoteOnly` 调用者执行
    /// - `ClientOrRemote` 函数可被任意调用者执行
    pub fn dispatch(&self, name: &str, args: Value, caller: CallableFrom) -> RenderResult<Value> {
        // 检查函数是否存在
        if !self.functions.contains_key(name) {
            return Err(crate::error::RendererError::FunctionNotAvailable(
                name.to_string(),
            ));
        }

        // 强制执行 callableFrom 边界
        if !self.can_call_from(name, caller) {
            return Err(crate::error::RendererError::InvalidFunctionCall(
                name.to_string(),
            ));
        }

        // 优先使用注册的 handler
        if let Some(handler) = self.handlers.get(name) {
            return handler(args);
        }

        // 没有 handler 时返回空值
        Ok(Value::Null)
    }

    /// 检查函数是否可以从指定端调用
    pub fn can_call_from(&self, name: &str, from: CallableFrom) -> bool {
        self.functions.get(name).is_some_and(|f| {
            f.callable_from == from || f.callable_from == CallableFrom::ClientOrRemote
        })
    }

    /// 获取函数定义
    pub fn get(&self, name: &str) -> Option<&FunctionDef> {
        self.functions.get(name)
    }

    /// 获取所有已注册函数名
    pub fn registered_names(&self) -> Vec<&String> {
        self.functions.keys().collect()
    }
}

// ============================================================================
// 格式化辅助函数
// ============================================================================

/// 为数字字符串的整数部分添加千位分隔符
fn add_thousands_separator(s: &str) -> String {
    let mut result = String::new();
    let is_negative = s.starts_with('-');
    let abs_str = if is_negative { &s[1..] } else { s };

    // 按小数点分割整数和小数部分
    if let Some(dot_pos) = abs_str.find('.') {
        let int_part = &abs_str[..dot_pos];
        let dec_part = &abs_str[dot_pos..];
        for (i, c) in int_part.chars().enumerate() {
            if i > 0 && (int_part.len() - i) % 3 == 0 {
                result.push(',');
            }
            result.push(c);
        }
        result.push_str(dec_part);
    } else {
        for (i, c) in abs_str.chars().enumerate() {
            if i > 0 && (abs_str.len() - i) % 3 == 0 {
                result.push(',');
            }
            result.push(c);
        }
    }

    if is_negative {
        result.insert(0, '-');
    }
    result
}

/// 格式化数字
fn format_number(value: f64, decimals: usize, grouping: bool) -> String {
    let rounded = format!("{:.1$}", value, decimals);
    if grouping {
        add_thousands_separator(&rounded)
    } else {
        rounded
    }
}

/// 获取货币符号
fn get_currency_symbol(currency: &str) -> &str {
    match currency {
        "CNY" | "JPY" => "\u{00a5}", // ¥
        "USD" => "$",
        "EUR" => "\u{20ac}", // €
        "GBP" => "\u{00a3}", // £
        _ => currency,
    }
}

/// 格式化货币
fn format_currency(value: f64, currency: &str, decimals: usize, grouping: bool) -> String {
    let symbol = get_currency_symbol(currency);
    let number_str = format_number(value, decimals, grouping);
    format!("{}{}", symbol, number_str)
}

/// 将 Unix 时间戳（秒）转换为 (年, 月, 日)
fn timestamp_to_date(ts: i64) -> Option<(i32, u32, u32)> {
    let days = if ts >= 0 {
        ts / 86400
    } else {
        (ts + 1) / 86400 - 1
    };
    days_to_date(days)
}

/// 将 Epoch 天数转换为 (年, 月, 日)
/// 使用 Howard Hinnant 的日期算法
fn days_to_date(days: i64) -> Option<(i32, u32, u32)> {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    Some((y as i32, m as u32, d as u32))
}

/// 计算从 Unix 纪元（1970-01-01）以来的天数
fn days_since_epoch(year: i32, month: u32, day: u32) -> i64 {
    let y = year as i64;
    let m = month as i64;
    let d = day as i64;

    let (adj_y, adj_m) = if m <= 2 { (y - 1, m + 12) } else { (y, m) };

    // 使用 Fliegel-Van Flandern 算法的变体
    let jdn = (1461 * (adj_y + 4800 + (adj_m - 14) / 12)) / 4
        + (367 * (adj_m - 2 - 12 * ((adj_m - 14) / 12))) / 12
        - (3 * ((adj_y + 4900 + (adj_m - 14) / 12) / 100)) / 4
        + d
        - 32075;

    // JDN(1970-01-01) = 2440588
    jdn - 2440588
}

/// 解析日期字符串，支持 ISO 8601 和 Unix 时间戳
fn parse_date(value: &str) -> Option<(i32, u32, u32)> {
    let trimmed = value.trim();

    // 尝试解析为 Unix 时间戳（秒）
    if let Ok(ts) = trimmed.parse::<i64>() {
        return timestamp_to_date(ts);
    }

    // 尝试 ISO 8601: "2024-01-15" 或 "2024-01-15T..." 或 "2024-01-15 ..."
    let date_part = trimmed
        .split('T')
        .next()
        .and_then(|s| s.split_whitespace().next())
        .unwrap_or(trimmed);

    let parts: Vec<&str> = date_part.split('-').collect();
    if parts.len() == 3 {
        let year = parts[0].parse::<i32>().ok()?;
        let month = parts[1].parse::<u32>().ok()?;
        let day = parts[2].parse::<u32>().ok()?;
        if (1..=12).contains(&month) && (1..=31).contains(&day) {
            Some((year, month, day))
        } else {
            None
        }
    } else {
        None
    }
}

const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// 格式化日期字符串
fn format_date(value: &str, format: &str) -> String {
    let parsed = parse_date(value);

    match parsed {
        Some((year, month, day)) => match format {
            "YYYY-MM-DD" => format!("{:04}-{:02}-{:02}", year, month, day),
            "full" => {
                let month_name = MONTH_NAMES[month as usize - 1];
                format!("{} {}, {}", month_name, day, year)
            }
            "short" => format!("{:02}/{:02}/{:04}", month, day, year),
            "relative" => format_relative_date(year, month, day),
            _ => format!("{:04}-{:02}-{:02}", year, month, day),
        },
        None => value.to_string(),
    }
}

/// 计算相对日期字符串
fn format_relative_date(year: i32, month: u32, day: u32) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let (now_year, now_month, now_day) = timestamp_to_date(now).unwrap_or((1970, 1, 1));

    let target_days = days_since_epoch(year, month, day);
    let now_days = days_since_epoch(now_year, now_month, now_day);

    let diff = target_days - now_days;

    match diff {
        0 => "today".to_string(),
        1 => "tomorrow".to_string(),
        -1 => "yesterday".to_string(),
        d if d > 1 => format!("in {} days", d),
        d => format!("{} days ago", -d),
    }
}

/// 根据 CLDR 复数规则选择合适的复数形式
///
/// 优先级：zero > one > two > few > many > other（兜底）
fn pluralize(value: i64, args: &Value) -> String {
    let forms: [(&str, bool); 5] = [
        ("zero", value == 0),
        ("one", value == 1),
        ("two", value == 2),
        ("few", (3..=10).contains(&value)),
        ("many", value > 10),
    ];

    for (key, condition) in &forms {
        if *condition {
            if let Some(v) = args.get(*key).and_then(|v| v.as_str()) {
                return v.to_string();
            }
        }
    }

    // 兜底到 "other"
    args.get("other")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_register_and_dispatch() {
        let mut dispatcher = FunctionDispatcher::new();
        dispatcher.register("add", CallableFrom::ClientOrRemote);
        assert!(dispatcher.can_call_from("add", CallableFrom::ClientOnly));
        assert!(dispatcher.can_call_from("add", CallableFrom::RemoteOnly));
        assert!(dispatcher.can_call_from("add", CallableFrom::ClientOrRemote));
    }

    #[test]
    fn test_client_only_cannot_be_called_remote() {
        let mut dispatcher = FunctionDispatcher::new();
        dispatcher.register("validate", CallableFrom::ClientOnly);
        assert!(dispatcher.can_call_from("validate", CallableFrom::ClientOnly));
        assert!(!dispatcher.can_call_from("validate", CallableFrom::RemoteOnly));
    }

    #[test]
    fn test_remote_only_cannot_be_called_client() {
        let mut dispatcher = FunctionDispatcher::new();
        dispatcher.register("fetch", CallableFrom::RemoteOnly);
        assert!(!dispatcher.can_call_from("fetch", CallableFrom::ClientOnly));
        assert!(dispatcher.can_call_from("fetch", CallableFrom::RemoteOnly));
    }

    #[test]
    fn test_dispatch_unknown_function() {
        let dispatcher = FunctionDispatcher::new();
        let result: RenderResult<Value> =
            dispatcher.dispatch("unknown", json!({}), CallableFrom::ClientOnly);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_undefined() {
        let dispatcher = FunctionDispatcher::new();
        assert!(dispatcher.get("unknown").is_none());
    }

    #[test]
    fn test_registered_names() {
        let mut dispatcher = FunctionDispatcher::new();
        dispatcher.register("f1", CallableFrom::ClientOnly);
        dispatcher.register("f2", CallableFrom::RemoteOnly);
        let names = dispatcher.registered_names();
        // new() 预注册 formatNumber/formatCurrency/formatDate/pluralize = 4 个
        assert_eq!(names.len(), 6);
    }

    #[test]
    fn test_dispatch_with_handler() {
        let mut dispatcher = FunctionDispatcher::new();
        dispatcher.register_with_handler(
            "upper",
            CallableFrom::ClientOrRemote,
            Arc::new(|args| {
                let s = args.get("value").and_then(|v| v.as_str()).unwrap_or("");
                Ok(json!(s.to_uppercase()))
            }),
        );
        let result = dispatcher
            .dispatch(
                "upper",
                json!({"value": "hello"}),
                CallableFrom::ClientOrRemote,
            )
            .unwrap();
        assert_eq!(result, json!("HELLO"));
    }

    #[test]
    fn test_dispatch_without_handler_returns_null() {
        let mut dispatcher = FunctionDispatcher::new();
        dispatcher.register("noop", CallableFrom::ClientOrRemote);
        let result = dispatcher
            .dispatch("noop", json!({}), CallableFrom::ClientOrRemote)
            .unwrap();
        assert_eq!(result, Value::Null);
    }

    // --- Basic Catalog 格式化函数 ---

    #[test]
    fn test_format_number_with_grouping() {
        let dispatcher = FunctionDispatcher::new();
        let result = dispatcher
            .dispatch(
                "formatNumber",
                json!({"value": 1234567.89, "decimals": 2, "grouping": true}),
                CallableFrom::ClientOnly,
            )
            .unwrap();
        assert_eq!(result, json!("1,234,567.89"));
    }

    #[test]
    fn test_format_number_no_grouping() {
        let dispatcher = FunctionDispatcher::new();
        let result = dispatcher
            .dispatch(
                "formatNumber",
                json!({"value": 1000.5, "decimals": 1, "grouping": false}),
                CallableFrom::ClientOnly,
            )
            .unwrap();
        assert_eq!(result, json!("1000.5"));
    }

    #[test]
    fn test_format_currency_cny() {
        let dispatcher = FunctionDispatcher::new();
        let result = dispatcher
            .dispatch(
                "formatCurrency",
                json!({"value": 1234.5, "currency": "CNY", "decimals": 2}),
                CallableFrom::ClientOnly,
            )
            .unwrap();
        let s = result.as_str().unwrap();
        assert!(s.contains("\u{00a5}") || s.contains("CNY"));
        assert!(s.contains("1,234.50") || s.contains("1234.50"));
    }

    #[test]
    fn test_format_currency_usd() {
        let dispatcher = FunctionDispatcher::new();
        let result = dispatcher
            .dispatch(
                "formatCurrency",
                json!({"value": 99.99, "currency": "USD", "decimals": 2}),
                CallableFrom::ClientOnly,
            )
            .unwrap();
        assert!(result.as_str().unwrap().contains("99.99"));
    }

    #[test]
    fn test_format_date_iso() {
        let dispatcher = FunctionDispatcher::new();
        let result = dispatcher
            .dispatch(
                "formatDate",
                json!({"value": "2024-01-15", "format": "YYYY-MM-DD"}),
                CallableFrom::ClientOnly,
            )
            .unwrap();
        assert!(result.as_str().unwrap().contains("2024"));
    }

    #[test]
    fn test_format_date_full() {
        let dispatcher = FunctionDispatcher::new();
        let result = dispatcher
            .dispatch(
                "formatDate",
                json!({"value": "2024-01-15", "format": "full"}),
                CallableFrom::ClientOnly,
            )
            .unwrap();
        let s = result.as_str().unwrap();
        assert!(s.contains("January") || s.contains("1\u{6708}") || s.contains("2024"));
    }

    #[test]
    fn test_pluralize_one() {
        let dispatcher = FunctionDispatcher::new();
        let result = dispatcher
            .dispatch(
                "pluralize",
                json!({"value": 1, "one": "item", "other": "items"}),
                CallableFrom::ClientOnly,
            )
            .unwrap();
        assert_eq!(result, json!("item"));
    }

    #[test]
    fn test_pluralize_many() {
        let dispatcher = FunctionDispatcher::new();
        let result = dispatcher
            .dispatch(
                "pluralize",
                json!({"value": 5, "one": "item", "other": "items"}),
                CallableFrom::ClientOnly,
            )
            .unwrap();
        assert_eq!(result, json!("items"));
    }

    #[test]
    fn test_pluralize_zero() {
        let dispatcher = FunctionDispatcher::new();
        let result = dispatcher
            .dispatch(
                "pluralize",
                json!({"value": 0, "zero": "no items", "one": "item", "other": "items"}),
                CallableFrom::ClientOnly,
            )
            .unwrap();
        assert_eq!(result, json!("no items"));
    }

    // --- dispatch 内部 callableFrom 强制执行测试 ---

    #[test]
    fn test_dispatch_enforces_client_only_boundary() {
        let mut dispatcher = FunctionDispatcher::new();
        dispatcher.register("client_func", CallableFrom::ClientOnly);
        // ClientOnly 调用者可以执行
        assert!(dispatcher
            .dispatch("client_func", json!({}), CallableFrom::ClientOnly)
            .is_ok());
        // RemoteOnly 调用者被拒绝
        assert!(matches!(
            dispatcher.dispatch("client_func", json!({}), CallableFrom::RemoteOnly),
            Err(crate::error::RendererError::InvalidFunctionCall(_))
        ));
    }

    #[test]
    fn test_dispatch_enforces_remote_only_boundary() {
        let mut dispatcher = FunctionDispatcher::new();
        dispatcher.register("remote_func", CallableFrom::RemoteOnly);
        // ClientOnly 调用者被拒绝
        assert!(matches!(
            dispatcher.dispatch("remote_func", json!({}), CallableFrom::ClientOnly),
            Err(crate::error::RendererError::InvalidFunctionCall(_))
        ));
        // RemoteOnly 调用者可以执行
        assert!(dispatcher
            .dispatch("remote_func", json!({}), CallableFrom::RemoteOnly)
            .is_ok());
    }

    #[test]
    fn test_dispatch_client_or_remote_works_from_both_sides() {
        let mut dispatcher = FunctionDispatcher::new();
        dispatcher.register("both", CallableFrom::ClientOrRemote);
        assert!(dispatcher
            .dispatch("both", json!({}), CallableFrom::ClientOnly)
            .is_ok());
        assert!(dispatcher
            .dispatch("both", json!({}), CallableFrom::RemoteOnly)
            .is_ok());
        assert!(dispatcher
            .dispatch("both", json!({}), CallableFrom::ClientOrRemote)
            .is_ok());
    }

    #[test]
    fn test_client_or_remote_caller_cannot_bypass_restrictions() {
        // 安全测试：ClientOrRemote 调用者不能调用 ClientOnly 或 RemoteOnly 函数
        let mut dispatcher = FunctionDispatcher::new();
        dispatcher.register("client_func", CallableFrom::ClientOnly);
        dispatcher.register("remote_func", CallableFrom::RemoteOnly);
        dispatcher.register("both_func", CallableFrom::ClientOrRemote);

        // ClientOrRemote 调用者不能调用 ClientOnly 函数
        assert!(!dispatcher.can_call_from("client_func", CallableFrom::ClientOrRemote));
        // ClientOrRemote 调用者不能调用 RemoteOnly 函数
        assert!(!dispatcher.can_call_from("remote_func", CallableFrom::ClientOrRemote));
        // ClientOrRemote 调用者只能调用 ClientOrRemote 函数
        assert!(dispatcher.can_call_from("both_func", CallableFrom::ClientOrRemote));
    }
}
