//! Type1 フォント（Adobe "Type 1 Font Format"）。
//!
//! - PFB のセグメント分解と PFA の受け入れ
//! - eexec 暗号（r = 55665）と charstring 暗号（r = 4330、`lenIV`）の復号
//! - `/Encoding`、`/FontMatrix`、`/Subrs`、`/CharStrings` の抽出
//! - Type1 charstring 解釈器: hsbw / sbw、rlineto 系、rrcurveto 系、closepath、callsubr / return、
//!   div、seac（合成字形）、callothersubr による flex（OtherSubrs 0〜2）と hint replacement（3）、setcurrentpoint。
//!   ヒント命令（hstem / vstem / hstem3 / vstem3 / dotsection）は読み飛ばす。

use std::collections::HashMap;

use crate::outline::{Outline, Segment};
use crate::{gerr, Glyph, GlyphError};

#[derive(Debug, Clone)]
pub struct Type1Font {
    pub font_name: String,
    /// FontMatrix。通常 [0.001 0 0 0.001 0 0]
    pub font_matrix: [f64; 6],
    /// 組み込みの符号化（256 個。None は .notdef）
    pub encoding: Vec<Option<String>>,
    subrs: Vec<Vec<u8>>,
    charstrings: HashMap<String, Vec<u8>>,
    /// 名前の一覧（定義順）
    pub glyph_names: Vec<String>,
}

const EEXEC_R: u16 = 55665;
const CHARSTRING_R: u16 = 4330;

fn decrypt(data: &[u8], mut r: u16, skip: usize) -> Vec<u8> {
    const C1: u16 = 52845;
    const C2: u16 = 22719;
    let mut out = Vec::with_capacity(data.len());
    for &c in data {
        let p = c ^ (r >> 8) as u8;
        r = (c as u16).wrapping_add(r).wrapping_mul(C1).wrapping_add(C2);
        out.push(p);
    }
    if out.len() > skip {
        out.drain(..skip);
    } else {
        out.clear();
    }
    out
}

/// PFB のセグメントを結合して (平文, eexec 部) を返す。PFA ならテキストとして扱う
fn split_segments(data: &[u8]) -> Result<(Vec<u8>, Vec<u8>), GlyphError> {
    if data.first() == Some(&0x80) {
        let mut clear = Vec::new();
        let mut binary = Vec::new();
        let mut pos = 0;
        while pos + 6 <= data.len() && data[pos] == 0x80 {
            let kind = data[pos + 1];
            if kind == 3 {
                break;
            }
            let len =
                u32::from_le_bytes([data[pos + 2], data[pos + 3], data[pos + 4], data[pos + 5]])
                    as usize;
            let seg = data.get(pos + 6..pos + 6 + len).ok_or_else(|| GlyphError {
                message: "truncated PFB segment".into(),
            })?;
            match kind {
                1 => {
                    if binary.is_empty() {
                        clear.extend_from_slice(seg);
                    }
                    // 末尾の cleartomark 部は無視
                }
                2 => binary.extend_from_slice(seg),
                _ => return gerr(format!("bad PFB segment type {kind}")),
            }
            pos += 6 + len;
        }
        return Ok((clear, binary));
    }
    // PFA: "eexec" の後が 16 進または生バイナリ
    let text = data;
    let idx = find(text, b"eexec").ok_or_else(|| GlyphError {
        message: "no eexec section".into(),
    })?;
    let clear = text[..idx].to_vec();
    let mut rest = &text[idx + 5..];
    while let Some((&c, r)) = rest.split_first() {
        if c == b'\r' || c == b'\n' || c == b' ' || c == b'\t' {
            rest = r;
        } else {
            break;
        }
    }
    let is_hex = rest.iter().take(4).all(|c| c.is_ascii_hexdigit());
    let binary = if is_hex {
        let mut out = Vec::new();
        let mut hi: Option<u8> = None;
        for &c in rest {
            let v = match c {
                b'0'..=b'9' => c - b'0',
                b'a'..=b'f' => c - b'a' + 10,
                b'A'..=b'F' => c - b'A' + 10,
                _ => continue,
            };
            match hi {
                None => hi = Some(v),
                Some(h) => {
                    out.push((h << 4) | v);
                    hi = None;
                }
            }
        }
        out
    } else {
        rest.to_vec()
    };
    Ok((clear, binary))
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn find_from(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    find(&hay[from..], needle).map(|i| i + from)
}

/// 平文または復号済み private 部の簡易字句解析
struct Lexer<'a> {
    s: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    fn skip_ws(&mut self) {
        while self.pos < self.s.len() && (self.s[self.pos].is_ascii_whitespace()) {
            self.pos += 1;
        }
    }
    fn token(&mut self) -> Option<&'a [u8]> {
        self.skip_ws();
        if self.pos >= self.s.len() {
            return None;
        }
        let start = self.pos;
        let c = self.s[self.pos];
        if c == b'/' || c.is_ascii_alphanumeric() || c == b'-' || c == b'.' || c == b'_' {
            self.pos += 1;
            while self.pos < self.s.len() {
                let d = self.s[self.pos];
                if d.is_ascii_whitespace() || b"/[]{}()<>".contains(&d) {
                    break;
                }
                self.pos += 1;
            }
        } else {
            self.pos += 1;
        }
        Some(&self.s[start..self.pos])
    }
    fn int(&mut self) -> Option<i64> {
        let t = self.token()?;
        std::str::from_utf8(t).ok()?.parse().ok()
    }
}

impl Type1Font {
    pub fn parse(data: &[u8]) -> Result<Type1Font, GlyphError> {
        let (clear, binary) = split_segments(data)?;
        let font_name = extract_name(&clear, b"/FontName").unwrap_or_default();
        let font_matrix = extract_matrix(&clear).unwrap_or([0.001, 0.0, 0.0, 0.001, 0.0, 0.0]);
        let encoding = extract_encoding(&clear);
        let private = decrypt(&binary, EEXEC_R, 4);
        let len_iv = find(&private, b"/lenIV")
            .map(|i| {
                Lexer {
                    s: &private,
                    pos: i + 6,
                }
                .int()
                .unwrap_or(4) as usize
            })
            .unwrap_or(4);
        let subrs = parse_subrs(&private, len_iv);
        let (charstrings, glyph_names) = parse_charstrings(&private, len_iv)?;
        Ok(Type1Font {
            font_name,
            font_matrix,
            encoding,
            subrs,
            charstrings,
            glyph_names,
        })
    }

    pub fn has_glyph(&self, name: &str) -> bool {
        self.charstrings.contains_key(name)
    }

    /// 字形名から輪郭と送り幅（字形単位。FontMatrix 適用前）
    pub fn glyph(&self, name: &str) -> Result<Glyph, GlyphError> {
        let cs = self.charstrings.get(name).ok_or_else(|| GlyphError {
            message: format!("no glyph {name}"),
        })?;
        let mut interp = Interp::new(self);
        interp.run(cs, 0)?;
        interp.finish();
        Ok(Glyph {
            outline: interp.outline,
            advance: interp.advance,
        })
    }

    /// 符号化位置から字形（組み込み符号化を使う）
    pub fn glyph_by_code(&self, code: u8) -> Result<Glyph, GlyphError> {
        let name = self.encoding[code as usize]
            .as_deref()
            .ok_or_else(|| GlyphError {
                message: format!("code {code} is .notdef"),
            })?;
        self.glyph(name)
    }
}

fn extract_name(clear: &[u8], key: &[u8]) -> Option<String> {
    let i = find(clear, key)?;
    let mut lx = Lexer {
        s: clear,
        pos: i + key.len(),
    };
    let t = lx.token()?;
    Some(String::from_utf8_lossy(t.strip_prefix(b"/").unwrap_or(t)).into_owned())
}

fn extract_matrix(clear: &[u8]) -> Option<[f64; 6]> {
    let i = find(clear, b"/FontMatrix")?;
    let lb = find_from(clear, b"[", i)?;
    let rb = find_from(clear, b"]", lb)?;
    let nums: Vec<f64> = std::str::from_utf8(&clear[lb + 1..rb])
        .ok()?
        .split_whitespace()
        .filter_map(|x| x.parse().ok())
        .collect();
    if nums.len() == 6 {
        Some([nums[0], nums[1], nums[2], nums[3], nums[4], nums[5]])
    } else {
        None
    }
}

fn extract_encoding(clear: &[u8]) -> Vec<Option<String>> {
    let mut enc: Vec<Option<String>> = vec![None; 256];
    let Some(i) = find(clear, b"/Encoding") else {
        return enc;
    };
    let mut lx = Lexer {
        s: clear,
        pos: i + 9,
    };
    match lx.token() {
        Some(b"StandardEncoding") => {
            let std = sabi_standard_encoding();
            for (k, n) in std.iter().enumerate() {
                if !n.is_empty() {
                    enc[k] = Some(n.to_string());
                }
            }
        }
        _ => {
            // "dup <code> /<name> put" を readonly def まで拾う
            let end = find_from(clear, b" def", lx.pos).unwrap_or(clear.len());
            let mut lx = Lexer {
                s: &clear[..end],
                pos: lx.pos,
            };
            while let Some(t) = lx.token() {
                if t == b"dup" {
                    let code = lx.int();
                    let name = lx.token();
                    if let (Some(code), Some(name)) = (code, name) {
                        if let Some(n) = name.strip_prefix(b"/") {
                            if (0..256).contains(&code) {
                                enc[code as usize] = Some(String::from_utf8_lossy(n).into_owned());
                            }
                        }
                    }
                }
            }
        }
    }
    enc
}

/// `dup <i> <len> RD <bin> NP` の列
fn parse_subrs(private: &[u8], len_iv: usize) -> Vec<Vec<u8>> {
    let Some(i) = find(private, b"/Subrs") else {
        return Vec::new();
    };
    let mut lx = Lexer {
        s: private,
        pos: i + 6,
    };
    let count = lx.int().unwrap_or(0).max(0) as usize;
    let mut subrs = vec![Vec::new(); count];
    let mut pos = lx.pos;
    for _ in 0..count {
        let Some(d) = find_from(private, b"dup ", pos) else {
            break;
        };
        let mut lx = Lexer {
            s: private,
            pos: d + 4,
        };
        let (Some(idx), Some(len)) = (lx.int(), lx.int()) else {
            break;
        };
        let _rd = lx.token(); // RD または -|
        let start = lx.pos + 1; // RD の後の 1 スペース
        let end = start + len as usize;
        if end > private.len() {
            break;
        }
        if (idx as usize) < count {
            subrs[idx as usize] = decrypt(&private[start..end], CHARSTRING_R, len_iv);
        }
        pos = end;
    }
    subrs
}

/// `/<name> <len> RD <bin> ND` の列
type CharStrings = (HashMap<String, Vec<u8>>, Vec<String>);

fn parse_charstrings(private: &[u8], len_iv: usize) -> Result<CharStrings, GlyphError> {
    let i = find(private, b"/CharStrings").ok_or_else(|| GlyphError {
        message: "no /CharStrings".into(),
    })?;
    let mut lx = Lexer {
        s: private,
        pos: i + 12,
    };
    let count = lx.int().unwrap_or(0).max(0) as usize;
    let mut map = HashMap::with_capacity(count);
    let mut names = Vec::with_capacity(count);
    // "begin" の後から
    let mut pos = find_from(private, b"begin", lx.pos)
        .map(|p| p + 5)
        .unwrap_or(lx.pos);
    while let Some(slash) = find_from(private, b"/", pos) {
        let mut lx = Lexer {
            s: private,
            pos: slash,
        };
        let Some(name_tok) = lx.token() else { break };
        if name_tok == b"/" {
            pos = slash + 1;
            continue;
        }
        let name = String::from_utf8_lossy(&name_tok[1..]).into_owned();
        let Some(len) = lx.int() else {
            // "end" に達した
            break;
        };
        let _rd = lx.token();
        let start = lx.pos + 1;
        let end = start + len as usize;
        if end > private.len() {
            return gerr(format!("charstring {name} runs past end"));
        }
        map.insert(
            name.clone(),
            decrypt(&private[start..end], CHARSTRING_R, len_iv),
        );
        names.push(name);
        pos = end;
        if names.len() >= count && count > 0 {
            break;
        }
    }
    Ok((map, names))
}

// ---------- charstring 解釈器 ----------

struct Interp<'a> {
    font: &'a Type1Font,
    stack: Vec<f64>,
    ps_stack: Vec<f64>,
    x: f64,
    y: f64,
    sbx: f64,
    sby: f64,
    advance: f64,
    outline: Outline,
    open: bool,
    flex_points: Vec<(f64, f64)>,
    in_flex: bool,
}

impl<'a> Interp<'a> {
    fn new(font: &'a Type1Font) -> Self {
        Interp {
            font,
            stack: Vec::new(),
            ps_stack: Vec::new(),
            x: 0.0,
            y: 0.0,
            sbx: 0.0,
            sby: 0.0,
            advance: 0.0,
            outline: Outline::default(),
            open: false,
            flex_points: Vec::new(),
            in_flex: false,
        }
    }

    fn moveto(&mut self, x: f64, y: f64) {
        self.x = x;
        self.y = y;
        if self.in_flex {
            self.flex_points.push((x, y));
            return;
        }
        self.close();
        self.outline.segments.push(Segment::MoveTo(x, y));
        self.open = true;
    }
    fn lineto(&mut self, x: f64, y: f64) {
        if !self.open {
            self.outline.segments.push(Segment::MoveTo(self.x, self.y));
            self.open = true;
        }
        self.x = x;
        self.y = y;
        self.outline.segments.push(Segment::LineTo(x, y));
    }
    fn curveto(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64) {
        if !self.open {
            self.outline.segments.push(Segment::MoveTo(self.x, self.y));
            self.open = true;
        }
        self.x = x3;
        self.y = y3;
        self.outline
            .segments
            .push(Segment::CurveTo(x1, y1, x2, y2, x3, y3));
    }
    fn close(&mut self) {
        if self.open {
            self.outline.segments.push(Segment::Close);
            self.open = false;
        }
    }
    fn finish(&mut self) {
        self.close();
    }

    /// true を返したら endchar
    fn run(&mut self, cs: &[u8], depth: usize) -> Result<bool, GlyphError> {
        if depth > 30 {
            return gerr("charstring recursion too deep");
        }
        let mut i = 0;
        while i < cs.len() {
            let v = cs[i] as i32;
            i += 1;
            if v >= 32 {
                let n = if v <= 246 {
                    v - 139
                } else if v <= 250 {
                    let w = *cs.get(i).ok_or_else(|| GlyphError {
                        message: "truncated number".into(),
                    })? as i32;
                    i += 1;
                    (v - 247) * 256 + w + 108
                } else if v <= 254 {
                    let w = *cs.get(i).ok_or_else(|| GlyphError {
                        message: "truncated number".into(),
                    })? as i32;
                    i += 1;
                    -(v - 251) * 256 - w - 108
                } else {
                    let b = cs.get(i..i + 4).ok_or_else(|| GlyphError {
                        message: "truncated number".into(),
                    })?;
                    i += 4;
                    i32::from_be_bytes([b[0], b[1], b[2], b[3]])
                };
                self.stack.push(n as f64);
                continue;
            }
            match v {
                13 => {
                    // hsbw: sbx wx
                    let (sbx, wx) = (self.arg(0), self.arg(1));
                    self.sbx = sbx;
                    self.sby = 0.0;
                    self.advance = wx;
                    self.x = sbx;
                    self.y = 0.0;
                    self.stack.clear();
                }
                9 => {
                    self.close();
                    self.stack.clear();
                }
                1 | 3 => self.stack.clear(), // hstem vstem
                21 => {
                    let (dx, dy) = (self.arg(0), self.arg(1));
                    self.moveto(self.x + dx, self.y + dy);
                    self.stack.clear();
                }
                22 => {
                    let dx = self.arg(0);
                    self.moveto(self.x + dx, self.y);
                    self.stack.clear();
                }
                4 => {
                    let dy = self.arg(0);
                    self.moveto(self.x, self.y + dy);
                    self.stack.clear();
                }
                5 => {
                    let (dx, dy) = (self.arg(0), self.arg(1));
                    self.lineto(self.x + dx, self.y + dy);
                    self.stack.clear();
                }
                6 => {
                    let dx = self.arg(0);
                    self.lineto(self.x + dx, self.y);
                    self.stack.clear();
                }
                7 => {
                    let dy = self.arg(0);
                    self.lineto(self.x, self.y + dy);
                    self.stack.clear();
                }
                8 => {
                    let a = [
                        self.arg(0),
                        self.arg(1),
                        self.arg(2),
                        self.arg(3),
                        self.arg(4),
                        self.arg(5),
                    ];
                    self.rrcurveto(a[0], a[1], a[2], a[3], a[4], a[5]);
                    self.stack.clear();
                }
                30 => {
                    // vhcurveto dy1 dx2 dy2 dx3
                    let a = [self.arg(0), self.arg(1), self.arg(2), self.arg(3)];
                    self.rrcurveto(0.0, a[0], a[1], a[2], a[3], 0.0);
                    self.stack.clear();
                }
                31 => {
                    // hvcurveto dx1 dx2 dy2 dy3
                    let a = [self.arg(0), self.arg(1), self.arg(2), self.arg(3)];
                    self.rrcurveto(a[0], 0.0, a[1], a[2], 0.0, a[3]);
                    self.stack.clear();
                }
                10 => {
                    let n = self.stack.pop().ok_or_else(|| GlyphError {
                        message: "callsubr without index".into(),
                    })? as i64;
                    let sub = self
                        .font
                        .subrs
                        .get(n as usize)
                        .ok_or_else(|| GlyphError {
                            message: format!("no subr {n}"),
                        })?
                        .clone();
                    if self.run(&sub, depth + 1)? {
                        return Ok(true);
                    }
                }
                11 => return Ok(false),
                14 => {
                    self.close();
                    return Ok(true);
                }
                12 => {
                    let v2 = *cs.get(i).ok_or_else(|| GlyphError {
                        message: "truncated escape".into(),
                    })?;
                    i += 1;
                    match v2 {
                        0 => self.stack.clear(),     // dotsection
                        1 | 2 => self.stack.clear(), // vstem3 hstem3
                        6 => {
                            // seac: asb adx ady bchar achar
                            let (asb, adx, ady, bchar, achar) = (
                                self.arg(0),
                                self.arg(1),
                                self.arg(2),
                                self.arg(3) as u8,
                                self.arg(4) as u8,
                            );
                            self.stack.clear();
                            self.seac(asb, adx, ady, bchar, achar)?;
                            return Ok(true);
                        }
                        7 => {
                            // sbw sbx sby wx wy
                            self.sbx = self.arg(0);
                            self.sby = self.arg(1);
                            self.advance = self.arg(2);
                            self.x = self.sbx;
                            self.y = self.sby;
                            self.stack.clear();
                        }
                        12 => {
                            let b = self.stack.pop().unwrap_or(1.0);
                            let a = self.stack.pop().unwrap_or(0.0);
                            self.stack.push(if b == 0.0 { 0.0 } else { a / b });
                        }
                        16 => self.callothersubr()?,
                        17 => {
                            let v = self.ps_stack.pop().unwrap_or(0.0);
                            self.stack.push(v);
                        }
                        33 => {
                            // setcurrentpoint
                            self.x = self.arg(0);
                            self.y = self.arg(1);
                            self.stack.clear();
                        }
                        _ => return gerr(format!("unknown escape op 12 {v2}")),
                    }
                }
                _ => return gerr(format!("unknown op {v}")),
            }
        }
        Ok(false)
    }

    fn arg(&self, i: usize) -> f64 {
        self.stack.get(i).copied().unwrap_or(0.0)
    }

    fn rrcurveto(&mut self, dx1: f64, dy1: f64, dx2: f64, dy2: f64, dx3: f64, dy3: f64) {
        let x1 = self.x + dx1;
        let y1 = self.y + dy1;
        let x2 = x1 + dx2;
        let y2 = y1 + dy2;
        let x3 = x2 + dx3;
        let y3 = y2 + dy3;
        self.curveto(x1, y1, x2, y2, x3, y3);
    }

    fn callothersubr(&mut self) -> Result<(), GlyphError> {
        let othersubr = self.stack.pop().unwrap_or(0.0) as i64;
        let n = self.stack.pop().unwrap_or(0.0) as usize;
        let start = self.stack.len().saturating_sub(n);
        let args: Vec<f64> = self.stack.drain(start..).collect();
        match othersubr {
            1 => {
                self.in_flex = true;
                self.flex_points.clear();
            }
            0 => {
                // flex 終了: 集めた点は [参照点, c1, c2, p1, c3, c4, p2]
                self.in_flex = false;
                if self.flex_points.len() >= 7 {
                    let p = &self.flex_points[1..7];
                    let p: Vec<(f64, f64)> = p.to_vec();
                    self.curveto(p[0].0, p[0].1, p[1].0, p[1].1, p[2].0, p[2].1);
                    self.curveto(p[3].0, p[3].1, p[4].0, p[4].1, p[5].0, p[5].1);
                }
                // 続く 2 回の pop に終点の y, x を返す
                self.ps_stack.clear();
                self.ps_stack.push(self.y);
                self.ps_stack.push(self.x);
            }
            2 => {}
            3 => {
                self.ps_stack.clear();
                self.ps_stack.push(3.0);
            }
            _ => {
                // 未知の OtherSubr: 引数をそのまま pop で返す
                self.ps_stack.clear();
                for a in args.into_iter().rev() {
                    self.ps_stack.push(a);
                }
            }
        }
        Ok(())
    }

    /// seac: 標準符号化の bchar と achar を合成する
    fn seac(
        &mut self,
        asb: f64,
        adx: f64,
        ady: f64,
        bchar: u8,
        achar: u8,
    ) -> Result<(), GlyphError> {
        let std = sabi_standard_encoding();
        let bname = std[bchar as usize];
        let aname = std[achar as usize];
        if bname.is_empty() || aname.is_empty() {
            return gerr("seac refers to .notdef");
        }
        let base = self.font.glyph(bname)?;
        let accent = self.font.glyph(aname)?;
        // アクセントの原点を (sbx - asb + adx, ady) に置く
        let (asbx, _) = accent_sidebearing(self.font, aname)?;
        let dx = self.sbx - asb + adx;
        let dy = ady;
        let _ = asbx;
        self.outline
            .segments
            .extend(base.outline.segments.iter().copied());
        self.outline.segments.extend(
            accent
                .outline
                .transform([1.0, 0.0, 0.0, 1.0, dx, dy])
                .segments,
        );
        Ok(())
    }
}

/// アクセント字形の左サイドベアリング（seac の計算に使う。hsbw の第 1 引数）
fn accent_sidebearing(font: &Type1Font, name: &str) -> Result<(f64, f64), GlyphError> {
    let cs = font.charstrings.get(name).ok_or_else(|| GlyphError {
        message: format!("no glyph {name}"),
    })?;
    let mut interp = Interp::new(font);
    interp.run(cs, 0)?;
    Ok((interp.sbx, interp.sby))
}

/// Adobe StandardEncoding（seac と組み込み符号化の解決用）
pub fn sabi_standard_encoding() -> &'static [&'static str; 256] {
    &STANDARD
}

const STANDARD: [&str; 256] = {
    let mut t = [""; 256];
    let entries: [(usize, &str); 149] = [
        (32, "space"),
        (33, "exclam"),
        (34, "quotedbl"),
        (35, "numbersign"),
        (36, "dollar"),
        (37, "percent"),
        (38, "ampersand"),
        (39, "quoteright"),
        (40, "parenleft"),
        (41, "parenright"),
        (42, "asterisk"),
        (43, "plus"),
        (44, "comma"),
        (45, "hyphen"),
        (46, "period"),
        (47, "slash"),
        (48, "zero"),
        (49, "one"),
        (50, "two"),
        (51, "three"),
        (52, "four"),
        (53, "five"),
        (54, "six"),
        (55, "seven"),
        (56, "eight"),
        (57, "nine"),
        (58, "colon"),
        (59, "semicolon"),
        (60, "less"),
        (61, "equal"),
        (62, "greater"),
        (63, "question"),
        (64, "at"),
        (65, "A"),
        (66, "B"),
        (67, "C"),
        (68, "D"),
        (69, "E"),
        (70, "F"),
        (71, "G"),
        (72, "H"),
        (73, "I"),
        (74, "J"),
        (75, "K"),
        (76, "L"),
        (77, "M"),
        (78, "N"),
        (79, "O"),
        (80, "P"),
        (81, "Q"),
        (82, "R"),
        (83, "S"),
        (84, "T"),
        (85, "U"),
        (86, "V"),
        (87, "W"),
        (88, "X"),
        (89, "Y"),
        (90, "Z"),
        (91, "bracketleft"),
        (92, "backslash"),
        (93, "bracketright"),
        (94, "asciicircum"),
        (95, "underscore"),
        (96, "quoteleft"),
        (97, "a"),
        (98, "b"),
        (99, "c"),
        (100, "d"),
        (101, "e"),
        (102, "f"),
        (103, "g"),
        (104, "h"),
        (105, "i"),
        (106, "j"),
        (107, "k"),
        (108, "l"),
        (109, "m"),
        (110, "n"),
        (111, "o"),
        (112, "p"),
        (113, "q"),
        (114, "r"),
        (115, "s"),
        (116, "t"),
        (117, "u"),
        (118, "v"),
        (119, "w"),
        (120, "x"),
        (121, "y"),
        (122, "z"),
        (123, "braceleft"),
        (124, "bar"),
        (125, "braceright"),
        (126, "asciitilde"),
        (161, "exclamdown"),
        (162, "cent"),
        (163, "sterling"),
        (164, "fraction"),
        (165, "yen"),
        (166, "florin"),
        (167, "section"),
        (168, "currency"),
        (169, "quotesingle"),
        (170, "quotedblleft"),
        (171, "guillemotleft"),
        (172, "guilsinglleft"),
        (173, "guilsinglright"),
        (174, "fi"),
        (175, "fl"),
        (177, "endash"),
        (178, "dagger"),
        (179, "daggerdbl"),
        (180, "periodcentered"),
        (182, "paragraph"),
        (183, "bullet"),
        (184, "quotesinglbase"),
        (185, "quotedblbase"),
        (186, "quotedblright"),
        (187, "guillemotright"),
        (188, "ellipsis"),
        (189, "perthousand"),
        (191, "questiondown"),
        (193, "grave"),
        (194, "acute"),
        (195, "circumflex"),
        (196, "tilde"),
        (197, "macron"),
        (198, "breve"),
        (199, "dotaccent"),
        (200, "dieresis"),
        (202, "ring"),
        (203, "cedilla"),
        (205, "hungarumlaut"),
        (206, "ogonek"),
        (207, "caron"),
        (208, "emdash"),
        (225, "AE"),
        (227, "ordfeminine"),
        (232, "Lslash"),
        (233, "Oslash"),
        (234, "OE"),
        (235, "ordmasculine"),
        (241, "ae"),
        (245, "dotlessi"),
        (248, "lslash"),
        (249, "oslash"),
        (250, "oe"),
        (251, "germandbls"),
    ];
    let mut i = 0;
    while i < entries.len() {
        t[entries[i].0] = entries[i].1;
        i += 1;
    }
    t
};
