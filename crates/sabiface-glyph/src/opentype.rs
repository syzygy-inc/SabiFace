//! CFF / TrueType（OpenType）の字形。ttf-parser に委ね、輪郭を [`Outline`] に変換する。
//! 字形 ID は XDV の `set_glyphs` や CMap の解決結果としてそのまま与えられる。

use crate::outline::{Outline, Segment};
use crate::{gerr, Glyph, GlyphError};

pub struct OpenTypeFont<'a> {
    face: ttf_parser::Face<'a>,
}

struct Builder {
    segments: Vec<Segment>,
}

impl ttf_parser::OutlineBuilder for Builder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.segments.push(Segment::MoveTo(x as f64, y as f64));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.segments.push(Segment::LineTo(x as f64, y as f64));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        // 二次を三次に持ち上げる。直前の点が要る
        let (x0, y0) = self.current();
        let (x1, y1, x, y) = (x1 as f64, y1 as f64, x as f64, y as f64);
        let c1 = (x0 + 2.0 / 3.0 * (x1 - x0), y0 + 2.0 / 3.0 * (y1 - y0));
        let c2 = (x + 2.0 / 3.0 * (x1 - x), y + 2.0 / 3.0 * (y1 - y));
        self.segments
            .push(Segment::CurveTo(c1.0, c1.1, c2.0, c2.1, x, y));
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.segments.push(Segment::CurveTo(
            x1 as f64, y1 as f64, x2 as f64, y2 as f64, x as f64, y as f64,
        ));
    }
    fn close(&mut self) {
        self.segments.push(Segment::Close);
    }
}

impl Builder {
    fn current(&self) -> (f64, f64) {
        for s in self.segments.iter().rev() {
            match *s {
                Segment::MoveTo(x, y) | Segment::LineTo(x, y) => return (x, y),
                Segment::CurveTo(_, _, _, _, x, y) => return (x, y),
                Segment::Close => continue,
            }
        }
        (0.0, 0.0)
    }
}

impl<'a> OpenTypeFont<'a> {
    pub fn parse(data: &'a [u8], index: u32) -> Result<Self, GlyphError> {
        let face = ttf_parser::Face::parse(data, index).map_err(|e| GlyphError {
            message: format!("{e:?}"),
        })?;
        Ok(OpenTypeFont { face })
    }

    pub fn units_per_em(&self) -> u16 {
        self.face.units_per_em()
    }

    pub fn glyph_count(&self) -> u16 {
        self.face.number_of_glyphs()
    }

    /// Unicode の cmap から字形 ID
    pub fn glyph_index(&self, ch: char) -> Option<u16> {
        self.face.glyph_index(ch).map(|g| g.0)
    }

    /// 字形名から字形 ID（post テーブルまたは CFF の charset）
    pub fn glyph_index_by_name(&self, name: &str) -> Option<u16> {
        self.face.glyph_index_by_name(name).map(|g| g.0)
    }

    pub fn glyph(&self, id: u16) -> Result<Glyph, GlyphError> {
        let gid = ttf_parser::GlyphId(id);
        if id >= self.face.number_of_glyphs() {
            return gerr(format!("glyph {id} out of range"));
        }
        let mut b = Builder {
            segments: Vec::new(),
        };
        self.face.outline_glyph(gid, &mut b);
        let advance = self.face.glyph_hor_advance(gid).unwrap_or(0) as f64;
        Ok(Glyph {
            outline: Outline {
                segments: b.segments,
            },
            advance,
        })
    }

    /// 縦組の送り（vmtx）。無ければ None
    pub fn vertical_advance(&self, id: u16) -> Option<f64> {
        self.face
            .glyph_ver_advance(ttf_parser::GlyphId(id))
            .map(|v| v as f64)
    }
}
