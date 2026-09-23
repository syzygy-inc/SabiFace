//! AFM（Adobe Font Metrics）の読み出し。仕様 4.1 のうち、字形の幅・名前・境界箱と大域情報を読む。

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct AfmChar {
    /// 符号化位置。-1 は未符号化
    pub code: i32,
    pub width: f64,
    pub name: String,
    /// 境界箱 (llx, lly, urx, ury)
    pub bbox: [f64; 4],
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Afm {
    pub font_name: String,
    pub font_bbox: Option<[f64; 4]>,
    pub chars: Vec<AfmChar>,
    /// 名前 → chars の添字
    pub by_name: HashMap<String, usize>,
}

impl Afm {
    pub fn parse(text: &str) -> Afm {
        let mut afm = Afm::default();
        for line in text.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("FontName ") {
                afm.font_name = rest.trim().to_string();
            } else if let Some(rest) = line.strip_prefix("FontBBox ") {
                let v: Vec<f64> = rest
                    .split_whitespace()
                    .filter_map(|x| x.parse().ok())
                    .collect();
                if v.len() == 4 {
                    afm.font_bbox = Some([v[0], v[1], v[2], v[3]]);
                }
            } else if line.starts_with("C ") {
                let mut code = -1;
                let mut width = 0.0;
                let mut name = String::new();
                let mut bbox = [0.0; 4];
                for field in line.split(';') {
                    let mut it = field.split_whitespace();
                    match it.next() {
                        Some("C") => code = it.next().and_then(|x| x.parse().ok()).unwrap_or(-1),
                        Some("WX") => width = it.next().and_then(|x| x.parse().ok()).unwrap_or(0.0),
                        Some("N") => name = it.next().unwrap_or("").to_string(),
                        Some("B") => {
                            let v: Vec<f64> = it.filter_map(|x| x.parse().ok()).collect();
                            if v.len() == 4 {
                                bbox = [v[0], v[1], v[2], v[3]];
                            }
                        }
                        _ => {}
                    }
                }
                afm.by_name.insert(name.clone(), afm.chars.len());
                afm.chars.push(AfmChar {
                    code,
                    width,
                    name,
                    bbox,
                });
            }
        }
        afm
    }

    pub fn by_name(&self, name: &str) -> Option<&AfmChar> {
        self.by_name.get(name).map(|&i| &self.chars[i])
    }
}
