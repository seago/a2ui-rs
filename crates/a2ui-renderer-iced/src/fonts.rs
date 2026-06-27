use std::borrow::Cow;

/// Font bytes and family name suitable for CJK text in iced examples.
#[derive(Debug, Clone)]
pub struct LoadedFont {
    pub family: &'static str,
    pub bytes: Cow<'static, [u8]>,
    pub path: &'static str,
}

/// Try common CJK-capable system fonts without requiring a bundled asset.
pub fn load_cjk_font() -> Option<LoadedFont> {
    const FONT_CANDIDATES: &[(&str, &str)] = &[
        (
            "Arial Unicode MS",
            "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        ),
        ("PingFang SC", "/System/Library/Fonts/PingFang.ttc"),
        (
            "Hiragino Sans GB",
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
        ),
        (
            "Noto Sans CJK SC",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        ),
        (
            "Noto Sans CJK SC",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        ),
        (
            "Droid Sans Fallback",
            "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
        ),
        ("Microsoft YaHei", "C:\\Windows\\Fonts\\msyh.ttf"),
        ("Microsoft YaHei", "C:\\Windows\\Fonts\\msyh.ttc"),
        ("SimSun", "C:\\Windows\\Fonts\\simsun.ttc"),
    ];

    FONT_CANDIDATES.iter().find_map(|(family, path)| {
        std::fs::read(path).ok().map(|bytes| LoadedFont {
            family,
            bytes: Cow::Owned(bytes),
            path,
        })
    })
}
