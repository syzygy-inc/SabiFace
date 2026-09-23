//! JFM（pTeX / upTeX の和文フォントメトリクス）の読み出し。
//!
//! 文字コードではなく「文字型（char_type）」ごとに寸法を持ち、文字型の間のグルーとカーンを表で持つ。
//! id = 11 が横組、9 が縦組。upTeX の拡張 JFM は文字コードを 24 ビットにする（`specification/design.md`）。

use crate::{bcpl, err, Fix, ParseError, Reader};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// 横組（id = 11）
    Yoko,
    /// 縦組（id = 9）
    Tate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TypeInfo {
    pub width: Fix,
    pub height: Fix,
    pub depth: Fix,
    pub italic: Fix,
    /// glue_kern プログラムの開始位置（tag = 1 のとき）
    pub glue_kern_start: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glue {
    pub width: Fix,
    pub stretch: Fix,
    pub shrink: Fix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlueKernOp {
    Kern(Fix),
    Glue(Glue),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlueKernStep {
    pub skip: u8,
    pub next_type: u8,
    pub op: GlueKernOp,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Jfm {
    pub direction: Direction,
    pub checksum: u32,
    pub design_size: Fix,
    pub coding_scheme: String,
    pub family: String,
    /// 文字コード → 文字型。表に無い文字は型 0（既定）
    pub char_types: Vec<(u32, u8)>,
    /// 文字型（0..=ec）→ 寸法
    pub types: Vec<Option<TypeInfo>>,
    pub glue_kern: Vec<GlueKernStep>,
    pub params: Vec<Fix>,
}

impl Jfm {
    pub fn parse(data: &[u8]) -> Result<Jfm, ParseError> {
        let mut r = Reader::new(data);
        let id = r.u16()?;
        let direction = match id {
            11 => Direction::Yoko,
            9 => Direction::Tate,
            _ => return err(0, format!("not a JFM (id = {id})")),
        };
        let nt = r.u16()? as usize;
        let lf = r.u16()? as usize;
        let lh = r.u16()? as usize;
        let bc = r.u16()? as usize;
        let ec = r.u16()? as usize;
        let nw = r.u16()? as usize;
        let nh = r.u16()? as usize;
        let nd = r.u16()? as usize;
        let ni = r.u16()? as usize;
        let nl = r.u16()? as usize;
        let nk = r.u16()? as usize;
        let ng = r.u16()? as usize;
        let np = r.u16()? as usize;
        if bc != 0 {
            return err(8, "JFM must have bc = 0");
        }
        let nc = ec + 1;
        if lf != 7 + lh + nt + nc + nw + nh + nd + ni + nl + nk + ng + np {
            return err(0, "file length does not match the subfile lengths");
        }
        if lf * 4 != data.len() {
            return err(
                0,
                format!("file is {} bytes but lf says {}", data.len(), lf * 4),
            );
        }
        let header = r.bytes(lh * 4)?;
        let checksum = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
        let design_size = Fix(i32::from_be_bytes([
            header[4], header[5], header[6], header[7],
        ]));
        let coding_scheme = if lh > 2 {
            bcpl(&header[8..(8 + 40).min(header.len())])
        } else {
            String::new()
        };
        let family = if lh > 12 {
            bcpl(&header[48..(48 + 20).min(header.len())])
        } else {
            String::new()
        };

        let mut char_types = Vec::with_capacity(nt);
        for _ in 0..nt {
            let b = [r.u8()?, r.u8()?, r.u8()?, r.u8()?];
            // 下位 16 ビットが先頭 2 バイト、上位 8 ビットが第 3 バイト（upTeX 拡張）、type が第 4 バイト
            let code = ((b[2] as u32) << 16) | ((b[0] as u32) << 8) | b[1] as u32;
            char_types.push((code, b[3]));
        }
        let mut raw_types = Vec::with_capacity(nc);
        for _ in 0..nc {
            raw_types.push([r.u8()?, r.u8()?, r.u8()?, r.u8()?]);
        }
        let read_fix = |r: &mut Reader, n: usize| -> Result<Vec<Fix>, ParseError> {
            let mut v = Vec::with_capacity(n);
            for _ in 0..n {
                v.push(r.fix()?);
            }
            Ok(v)
        };
        let widths = read_fix(&mut r, nw)?;
        let heights = read_fix(&mut r, nh)?;
        let depths = read_fix(&mut r, nd)?;
        let italics = read_fix(&mut r, ni)?;
        let mut raw_gk = Vec::with_capacity(nl);
        for _ in 0..nl {
            raw_gk.push([r.u8()?, r.u8()?, r.u8()?, r.u8()?]);
        }
        let kern = read_fix(&mut r, nk)?;
        // ng は語数。glue 1 項目は 3 語
        if !ng.is_multiple_of(3) {
            return err(0, "glue table length is not a multiple of 3");
        }
        let mut glue = Vec::with_capacity(ng / 3);
        for _ in 0..ng / 3 {
            glue.push(Glue {
                width: r.fix()?,
                stretch: r.fix()?,
                shrink: r.fix()?,
            });
        }
        let params = read_fix(&mut r, np)?;

        let mut glue_kern = Vec::with_capacity(nl);
        for (i, b) in raw_gk.iter().enumerate() {
            let op = if b[2] >= 128 {
                let k = 256 * (b[2] as usize - 128) + b[3] as usize;
                GlueKernOp::Kern(*kern.get(k).ok_or_else(|| ParseError {
                    offset: 0,
                    message: format!("glue_kern[{i}] kern index out of range"),
                })?)
            } else {
                let g = 256 * b[2] as usize + b[3] as usize;
                GlueKernOp::Glue(*glue.get(g).ok_or_else(|| ParseError {
                    offset: 0,
                    message: format!("glue_kern[{i}] glue index out of range"),
                })?)
            };
            glue_kern.push(GlueKernStep {
                skip: b[0],
                next_type: b[1],
                op,
            });
        }
        let mut types = Vec::with_capacity(nc);
        for (t, b) in raw_types.iter().enumerate() {
            let wi = b[0] as usize;
            if wi == 0 {
                types.push(None);
                continue;
            }
            let hi = (b[1] >> 4) as usize;
            let di = (b[1] & 15) as usize;
            let ii = (b[2] >> 2) as usize;
            let tag = b[2] & 3;
            let get = |v: &Vec<Fix>, i: usize, name: &str| -> Result<Fix, ParseError> {
                v.get(i).copied().ok_or_else(|| ParseError {
                    offset: 0,
                    message: format!("type {t} {name} index {i} out of range"),
                })
            };
            let glue_kern_start = if tag == 1 {
                let start = b[3] as usize;
                let start = match raw_gk.get(start) {
                    Some(s) if s[0] > 128 => 256 * s[2] as usize + s[3] as usize,
                    _ => start,
                };
                Some(start)
            } else {
                None
            };
            types.push(Some(TypeInfo {
                width: get(&widths, wi, "width")?,
                height: get(&heights, hi, "height")?,
                depth: get(&depths, di, "depth")?,
                italic: get(&italics, ii, "italic")?,
                glue_kern_start,
            }));
        }
        Ok(Jfm {
            direction,
            checksum,
            design_size,
            coding_scheme,
            family,
            char_types,
            types,
            glue_kern,
            params,
        })
    }

    /// 文字コードの文字型。表に無ければ 0
    pub fn char_type(&self, code: u32) -> u8 {
        self.char_types
            .iter()
            .find(|(c, _)| *c == code)
            .map(|(_, t)| *t)
            .unwrap_or(0)
    }

    pub fn type_info(&self, char_type: u8) -> Option<&TypeInfo> {
        self.types.get(char_type as usize).and_then(|t| t.as_ref())
    }

    /// 文字型 `left` と `right` の間に入るグルーまたはカーン
    pub fn glue_kern_between(&self, left: u8, right: u8) -> Option<GlueKernOp> {
        let mut i = self.type_info(left)?.glue_kern_start?;
        loop {
            let s = self.glue_kern.get(i)?;
            if s.next_type == right && s.skip <= 128 {
                return Some(s.op);
            }
            if s.skip >= 128 {
                return None;
            }
            i += s.skip as usize + 1;
        }
    }
}
