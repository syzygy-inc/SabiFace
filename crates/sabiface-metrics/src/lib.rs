//! TeX のメトリクスと符号化の読み出し（SabiFace、`specification/design.md`）。
//!
//! - `tfm`: TFM（tex.web §539〜§575）
//! - `jfm`: JFM（pTeX / upTeX。縦組を含む）
//! - `vf`: VF（仮想フォント）
//! - `afm`: AFM（Adobe Font Metrics）
//! - `enc`: PostScript 符号化ベクトル（dvips の `.enc`）
//!
//! ラスタライズ・整形・DVI・PDF には依存しない。

pub mod afm;
pub mod enc;
pub mod fix;
pub mod jfm;
pub mod tfm;
pub mod vf;

pub use fix::Fix;

/// 解析の失敗。位置はファイル先頭からのバイトオフセット。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub offset: usize,
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (at byte {})", self.message, self.offset)
    }
}

impl std::error::Error for ParseError {}

pub(crate) fn err<T>(offset: usize, message: impl Into<String>) -> Result<T, ParseError> {
    Err(ParseError {
        offset,
        message: message.into(),
    })
}

/// ビッグエンディアンの読み出し補助。
pub(crate) struct Reader<'a> {
    pub data: &'a [u8],
    pub pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Reader { data, pos: 0 }
    }
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }
    pub fn u8(&mut self) -> Result<u8, ParseError> {
        let b = *self.data.get(self.pos).ok_or_else(|| ParseError {
            offset: self.pos,
            message: "unexpected end".into(),
        })?;
        self.pos += 1;
        Ok(b)
    }
    pub fn u16(&mut self) -> Result<u16, ParseError> {
        Ok(((self.u8()? as u16) << 8) | self.u8()? as u16)
    }
    pub fn u24(&mut self) -> Result<u32, ParseError> {
        Ok(((self.u16()? as u32) << 8) | self.u8()? as u32)
    }
    pub fn u32(&mut self) -> Result<u32, ParseError> {
        Ok(((self.u16()? as u32) << 16) | self.u16()? as u32)
    }
    pub fn i32(&mut self) -> Result<i32, ParseError> {
        Ok(self.u32()? as i32)
    }
    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8], ParseError> {
        if self.remaining() < n {
            return err(self.pos, format!("unexpected end: need {n} bytes"));
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    pub fn fix(&mut self) -> Result<Fix, ParseError> {
        Ok(Fix(self.i32()?))
    }
}

/// BCPL 文字列（先頭 1 バイトが長さ）。TFM のヘッダのコーディングスキームとファミリ名に使う。
pub(crate) fn bcpl(bytes: &[u8]) -> String {
    match bytes.first() {
        Some(&n) => {
            let n = (n as usize).min(bytes.len().saturating_sub(1));
            String::from_utf8_lossy(&bytes[1..1 + n]).into_owned()
        }
        None => String::new(),
    }
}
