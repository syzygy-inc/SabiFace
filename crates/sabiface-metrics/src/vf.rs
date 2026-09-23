//! VF（仮想フォント）の読み出し。vftovp.web に従う。
//!
//! VF は TFM と対になり、各文字を「実フォントの文字と DVI 命令の列」として定義する。
//! 展開（DVI 命令列の実行）は利用側で行い、ここでは構造だけを読む。

use crate::{err, Fix, ParseError, Reader};

pub const PRE: u8 = 247;
pub const ID: u8 = 202;
pub const LONG_CHAR: u8 = 242;
pub const FNT_DEF1: u8 = 243;
pub const POST: u8 = 248;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontDef {
    /// `fnt_def` の番号。VF 内の DVI 命令の `fnt` はこの番号を参照する
    pub number: u32,
    pub checksum: u32,
    /// 実フォントの大きさ（デザインサイズに対する比。fix_word）
    pub scale: Fix,
    /// 実フォントのデザインサイズ（fix_word × 1pt）
    pub design_size: Fix,
    pub area: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VfChar {
    pub code: u32,
    /// TFM の幅（fix_word。対の TFM と一致しなければならない）
    pub width: Fix,
    /// DVI 命令の列（`fnt`、`set_char`、`right`、`push` など。VF が許すサブセット）
    pub dvi: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Vf {
    pub comment: String,
    pub checksum: u32,
    pub design_size: Fix,
    pub fonts: Vec<FontDef>,
    pub chars: Vec<VfChar>,
}

impl Vf {
    pub fn parse(data: &[u8]) -> Result<Vf, ParseError> {
        let mut r = Reader::new(data);
        if r.u8()? != PRE || r.u8()? != ID {
            return err(0, "not a VF file");
        }
        let k = r.u8()? as usize;
        let comment = String::from_utf8_lossy(r.bytes(k)?).into_owned();
        let checksum = r.u32()?;
        let design_size = r.fix()?;
        let mut fonts = Vec::new();
        let mut chars = Vec::new();
        loop {
            let at = r.pos;
            let op = r.u8()?;
            match op {
                FNT_DEF1..=246 => {
                    let n = (op - FNT_DEF1) as usize + 1;
                    let mut number = 0u32;
                    for _ in 0..n {
                        number = (number << 8) | r.u8()? as u32;
                    }
                    let checksum = r.u32()?;
                    let scale = r.fix()?;
                    let design_size = r.fix()?;
                    let a = r.u8()? as usize;
                    let l = r.u8()? as usize;
                    let area = String::from_utf8_lossy(r.bytes(a)?).into_owned();
                    let name = String::from_utf8_lossy(r.bytes(l)?).into_owned();
                    fonts.push(FontDef {
                        number,
                        checksum,
                        scale,
                        design_size,
                        area,
                        name,
                    });
                }
                LONG_CHAR => {
                    let pl = r.u32()? as usize;
                    let code = r.u32()?;
                    let width = r.fix()?;
                    let dvi = r.bytes(pl)?.to_vec();
                    chars.push(VfChar { code, width, dvi });
                }
                0..=241 => {
                    let pl = op as usize;
                    let code = r.u8()? as u32;
                    let width = Fix(r.u24()? as i32); // 3 バイトの fix_word（正）
                    let dvi = r.bytes(pl)?.to_vec();
                    chars.push(VfChar { code, width, dvi });
                }
                POST => break,
                _ => return err(at, format!("unexpected opcode {op} in VF")),
            }
        }
        Ok(Vf {
            comment,
            checksum,
            design_size,
            fonts,
            chars,
        })
    }

    pub fn char(&self, code: u32) -> Option<&VfChar> {
        self.chars.iter().find(|c| c.code == code)
    }
}
