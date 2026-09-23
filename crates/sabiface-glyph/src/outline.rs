//! 輪郭の表現。直線と三次ベジエからなる閉じた経路の列。座標は字形単位、f64。

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Segment {
    MoveTo(f64, f64),
    LineTo(f64, f64),
    /// 制御点 2 つと終点
    CurveTo(f64, f64, f64, f64, f64, f64),
    Close,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Outline {
    pub segments: Vec<Segment>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BBox {
    pub xmin: f64,
    pub ymin: f64,
    pub xmax: f64,
    pub ymax: f64,
}

impl Outline {
    pub fn is_empty(&self) -> bool {
        !self
            .segments
            .iter()
            .any(|s| matches!(s, Segment::LineTo(..) | Segment::CurveTo(..)))
    }

    /// 曲線の極値まで含めた厳密な境界箱。空なら None
    pub fn bbox(&self) -> Option<BBox> {
        let mut bb: Option<BBox> = None;
        let mut cur = (0.0, 0.0);
        let mut include = |x: f64, y: f64| {
            bb = Some(match bb {
                None => BBox {
                    xmin: x,
                    ymin: y,
                    xmax: x,
                    ymax: y,
                },
                Some(b) => BBox {
                    xmin: b.xmin.min(x),
                    ymin: b.ymin.min(y),
                    xmax: b.xmax.max(x),
                    ymax: b.ymax.max(y),
                },
            });
        };
        for s in &self.segments {
            match *s {
                Segment::MoveTo(x, y) => cur = (x, y),
                Segment::LineTo(x, y) => {
                    include(cur.0, cur.1);
                    include(x, y);
                    cur = (x, y);
                }
                Segment::CurveTo(x1, y1, x2, y2, x3, y3) => {
                    include(cur.0, cur.1);
                    include(x3, y3);
                    for t in cubic_extrema(cur.0, x1, x2, x3)
                        .into_iter()
                        .chain(cubic_extrema(cur.1, y1, y2, y3))
                    {
                        let (x, y) = cubic_point(cur, (x1, y1), (x2, y2), (x3, y3), t);
                        include(x, y);
                    }
                    cur = (x3, y3);
                }
                Segment::Close => {}
            }
        }
        bb
    }

    /// アフィン変換 [a b c d e f]（x' = a x + c y + e, y' = b x + d y + f）
    pub fn transform(&self, m: [f64; 6]) -> Outline {
        let p = |x: f64, y: f64| (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]);
        Outline {
            segments: self
                .segments
                .iter()
                .map(|s| match *s {
                    Segment::MoveTo(x, y) => {
                        let (x, y) = p(x, y);
                        Segment::MoveTo(x, y)
                    }
                    Segment::LineTo(x, y) => {
                        let (x, y) = p(x, y);
                        Segment::LineTo(x, y)
                    }
                    Segment::CurveTo(x1, y1, x2, y2, x3, y3) => {
                        let (a, b) = p(x1, y1);
                        let (c, d) = p(x2, y2);
                        let (e, f) = p(x3, y3);
                        Segment::CurveTo(a, b, c, d, e, f)
                    }
                    Segment::Close => Segment::Close,
                })
                .collect(),
        }
    }
}

/// 三次ベジエの一次元成分 p0..p3 の導関数の零点 t ∈ (0,1)
fn cubic_extrema(p0: f64, p1: f64, p2: f64, p3: f64) -> Vec<f64> {
    // B'(t) = 3[(p1-p0)(1-t)^2 + 2(p2-p1)(1-t)t + (p3-p2)t^2] = a t^2 + b t + c
    let a = 3.0 * (-p0 + 3.0 * p1 - 3.0 * p2 + p3);
    let b = 6.0 * (p0 - 2.0 * p1 + p2);
    let c = 3.0 * (p1 - p0);
    let mut out = Vec::new();
    if a.abs() < 1e-12 {
        if b.abs() > 1e-12 {
            let t = -c / b;
            if t > 0.0 && t < 1.0 {
                out.push(t);
            }
        }
        return out;
    }
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return out;
    }
    let sq = disc.sqrt();
    for t in [(-b + sq) / (2.0 * a), (-b - sq) / (2.0 * a)] {
        if t > 0.0 && t < 1.0 {
            out.push(t);
        }
    }
    out
}

fn cubic_point(
    p0: (f64, f64),
    p1: (f64, f64),
    p2: (f64, f64),
    p3: (f64, f64),
    t: f64,
) -> (f64, f64) {
    let u = 1.0 - t;
    let f = |a: f64, b: f64, c: f64, d: f64| {
        u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d
    };
    (f(p0.0, p1.0, p2.0, p3.0), f(p0.1, p1.1, p2.1, p3.1))
}
