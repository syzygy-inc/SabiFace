//! 字形の供給源（SabiFace）。字形 ID または字形名から装置非依存の輪郭を取り出す。
//!
//! - `outline`: 輪郭の表現（直線と三次ベジエ）と境界箱
//! - `type1`: Type1 フォント（`.pfb` / `.pfa`）の解析と charstring 解釈器
//! - `opentype`: CFF / TrueType（ttf-parser 経由）
//!
//! ラスタライズは含まない。

pub mod opentype;
pub mod outline;
pub mod type1;

pub use outline::{BBox, Outline, Segment};

/// 一つの字形の輪郭と送り幅。座標は字形単位（Type1 は通常 1000 / em、OpenType は `units_per_em`）。
#[derive(Debug, Clone, PartialEq)]
pub struct Glyph {
    pub outline: Outline,
    pub advance: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlyphError {
    pub message: String,
}

impl std::fmt::Display for GlyphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for GlyphError {}

pub(crate) fn gerr<T>(message: impl Into<String>) -> Result<T, GlyphError> {
    Err(GlyphError {
        message: message.into(),
    })
}
