//! TFM（TeX Font Metric）の読み出し。tex.web §539〜§575 と tftopl.web に従う。

use crate::{bcpl, err, Fix, ParseError, Reader};

/// 文字の寸法と付加情報（tex.web §543 の `char_info`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CharInfo {
    pub width: Fix,
    pub height: Fix,
    pub depth: Fix,
    pub italic: Fix,
    pub tag: Tag,
}

/// `char_info` の tag（§544）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tag {
    #[default]
    None,
    /// lig_kern プログラムの開始位置（`lig_kern` の添字）
    LigKern(usize),
    /// 一回り大きい文字（`char_list`）
    Larger(u8),
    /// 伸長文字（`exten` の添字）
    Extensible(usize),
}

/// lig_kern プログラムの一命令（§545）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LigKernStep {
    pub skip: u8,
    pub next_char: u8,
    pub op: LigKernOp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LigKernOp {
    /// カーニング（`kern` 表の添字）
    Kern(usize),
    /// 合字。`op_byte` の値（0..=11）と置換文字。a = op>>2, b = (op>>1)&1, c = op&1（§545）
    Lig { op: u8, char: u8 },
}

/// 伸長文字（§546）。0 は「無し」。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Extensible {
    pub top: u8,
    pub mid: u8,
    pub bot: u8,
    pub rep: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tfm {
    pub checksum: u32,
    /// デザインサイズ（pt）。fix_word × 1pt
    pub design_size: Fix,
    pub coding_scheme: String,
    pub family: String,
    pub face: Option<u8>,
    pub first_char: u16,
    pub last_char: u16,
    /// 文字コード → 寸法。範囲外や `width_index = 0` の文字は None
    chars: Vec<Option<CharInfo>>,
    pub lig_kern: Vec<LigKernStep>,
    pub kern: Vec<Fix>,
    pub exten: Vec<Extensible>,
    /// `fontdimen`。params[0] が slant（デザインサイズを掛けない）、以降は fix_word（§547）
    pub params: Vec<Fix>,
    /// 右境界文字（§545。`lig_kern[0].skip == 255` のとき）
    pub right_boundary: Option<u8>,
    /// 左境界文字の lig_kern プログラムの開始位置
    pub left_boundary_program: Option<usize>,
}

impl Tfm {
    pub fn parse(data: &[u8]) -> Result<Tfm, ParseError> {
        let mut r = Reader::new(data);
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
        let ne = r.u16()? as usize;
        let np = r.u16()? as usize;
        if !(bc <= ec + 1 && ec < 256) {
            return err(4, format!("bad bc/ec: {bc}/{ec}"));
        }
        let nc = if bc > ec { 0 } else { ec - bc + 1 };
        if lf != 6 + lh + nc + nw + nh + nd + ni + nl + nk + ne + np {
            return err(0, "file length does not match the subfile lengths");
        }
        if lf * 4 != data.len() {
            return err(
                0,
                format!("file is {} bytes but lf says {}", data.len(), lf * 4),
            );
        }
        if lh < 2 || nw == 0 || nh == 0 || nd == 0 || ni == 0 {
            return err(2, "header or dimension tables too short");
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
        let face = if lh > 17 { Some(header[71]) } else { None };

        let mut raw_chars = Vec::with_capacity(nc);
        for _ in 0..nc {
            raw_chars.push([r.u8()?, r.u8()?, r.u8()?, r.u8()?]);
        }
        let mut widths = Vec::with_capacity(nw);
        for _ in 0..nw {
            widths.push(r.fix()?);
        }
        let mut heights = Vec::with_capacity(nh);
        for _ in 0..nh {
            heights.push(r.fix()?);
        }
        let mut depths = Vec::with_capacity(nd);
        for _ in 0..nd {
            depths.push(r.fix()?);
        }
        let mut italics = Vec::with_capacity(ni);
        for _ in 0..ni {
            italics.push(r.fix()?);
        }
        let mut raw_lk = Vec::with_capacity(nl);
        for _ in 0..nl {
            raw_lk.push([r.u8()?, r.u8()?, r.u8()?, r.u8()?]);
        }
        let mut kern = Vec::with_capacity(nk);
        for _ in 0..nk {
            kern.push(r.fix()?);
        }
        let mut exten = Vec::with_capacity(ne);
        for _ in 0..ne {
            exten.push(Extensible {
                top: r.u8()?,
                mid: r.u8()?,
                bot: r.u8()?,
                rep: r.u8()?,
            });
        }
        let mut params = Vec::with_capacity(np);
        for _ in 0..np {
            params.push(r.fix()?);
        }

        let lig_kern: Vec<LigKernStep> = raw_lk
            .iter()
            .map(|b| LigKernStep {
                skip: b[0],
                next_char: b[1],
                op: if b[2] >= 128 {
                    LigKernOp::Kern(256 * (b[2] as usize - 128) + b[3] as usize)
                } else {
                    LigKernOp::Lig {
                        op: b[2],
                        char: b[3],
                    }
                },
            })
            .collect();
        for (i, s) in lig_kern.iter().enumerate() {
            if let LigKernOp::Kern(k) = s.op {
                if k >= kern.len() {
                    return err(
                        0,
                        format!("lig_kern[{i}] refers to kern {k} out of {}", kern.len()),
                    );
                }
            }
        }
        let right_boundary = if nl > 0 && raw_lk[0][0] == 255 {
            Some(raw_lk[0][1])
        } else {
            None
        };
        let left_boundary_program = if nl > 0 && raw_lk[nl - 1][0] == 255 {
            Some(256 * raw_lk[nl - 1][2] as usize + raw_lk[nl - 1][3] as usize)
        } else {
            None
        };

        let mut chars = Vec::with_capacity(nc);
        for (i, b) in raw_chars.iter().enumerate() {
            let wi = b[0] as usize;
            if wi == 0 {
                chars.push(None);
                continue;
            }
            let hi = (b[1] >> 4) as usize;
            let di = (b[1] & 15) as usize;
            let ii = (b[2] >> 2) as usize;
            let tag_bits = b[2] & 3;
            let rem = b[3];
            let get = |v: &Vec<Fix>, i: usize, name: &str| -> Result<Fix, ParseError> {
                v.get(i).copied().ok_or_else(|| ParseError {
                    offset: 0,
                    message: format!("char {} {name} index {i} out of range", bc + i),
                })
            };
            let tag = match tag_bits {
                0 => Tag::None,
                1 => {
                    let start = rem as usize;
                    // §545: 最初の命令の skip_byte > 128 なら真の開始位置は 256*op+rem
                    let start = match raw_lk.get(start) {
                        Some(s) if s[0] > 128 => 256 * s[2] as usize + s[3] as usize,
                        _ => start,
                    };
                    if start >= nl {
                        return err(
                            0,
                            format!("char {} lig_kern start {start} out of range", bc + i),
                        );
                    }
                    Tag::LigKern(start)
                }
                2 => Tag::Larger(rem),
                _ => {
                    if rem as usize >= ne {
                        return err(0, format!("char {} exten index out of range", bc + i));
                    }
                    Tag::Extensible(rem as usize)
                }
            };
            chars.push(Some(CharInfo {
                width: get(&widths, wi, "width")?,
                height: get(&heights, hi, "height")?,
                depth: get(&depths, di, "depth")?,
                italic: get(&italics, ii, "italic")?,
                tag,
            }));
        }
        Ok(Tfm {
            checksum,
            design_size,
            coding_scheme,
            family,
            face,
            first_char: bc as u16,
            last_char: ec as u16,
            chars,
            lig_kern,
            kern,
            exten,
            params,
            right_boundary,
            left_boundary_program,
        })
    }

    /// 文字の寸法。存在しない文字は None
    pub fn char_info(&self, code: u16) -> Option<&CharInfo> {
        if code < self.first_char || code > self.last_char {
            return None;
        }
        self.chars
            .get((code - self.first_char) as usize)
            .and_then(|c| c.as_ref())
    }

    pub fn chars(&self) -> impl Iterator<Item = (u16, &CharInfo)> {
        self.chars
            .iter()
            .enumerate()
            .filter_map(move |(i, c)| c.as_ref().map(|c| (self.first_char + i as u16, c)))
    }

    /// `fontdimen n`（1 始まり）。slant（n = 1）はデザインサイズを掛けない
    pub fn fontdimen(&self, n: usize) -> Option<Fix> {
        if n == 0 {
            return None;
        }
        self.params.get(n - 1).copied()
    }

    /// 文字 `left` の lig_kern プログラムを、`right` を次の文字として実行し、
    /// 合字か、カーン（fix_word）かを返す。§1040 の手順を右境界を含めて再現する。
    pub fn lig_kern_for(&self, left: u16, right: u16) -> Option<LigKernOp> {
        let start = match self.char_info(left)?.tag {
            Tag::LigKern(s) => s,
            _ => return None,
        };
        self.run_lig_kern(start, right)
    }

    /// 左境界（単語の先頭）の lig_kern プログラム
    pub fn left_boundary_lig_kern(&self, right: u16) -> Option<LigKernOp> {
        self.run_lig_kern(self.left_boundary_program?, right)
    }

    fn run_lig_kern(&self, mut i: usize, right: u16) -> Option<LigKernOp> {
        let right = u8::try_from(right).ok()?;
        loop {
            let s = self.lig_kern.get(i)?;
            if s.next_char == right && s.skip <= 128 {
                return Some(s.op);
            }
            if s.skip >= 128 {
                return None;
            }
            i += s.skip as usize + 1;
        }
    }
}
