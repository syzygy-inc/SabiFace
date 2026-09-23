//! フォントの解決（SabiFace）。TeX フォント名から実フォントのファイルと符号化を決める。
//!
//! - `pdftex.map` 系: `tfmname psname "opts" <[enc.enc <font.pfb`（dvipdfmx も同じ形式を読む）
//! - `kanjix.map`: `tfmname CMap fontfile`
//!
//! 部分集合化は将来ここに置く（字形の読み出しは sabiface-glyph）。

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MapEntry {
    pub tfm_name: String,
    /// PostScript 名（省略時は tfm 名）
    pub ps_name: String,
    /// `<[foo.enc` または `<foo.enc`
    pub encoding_file: Option<String>,
    /// `<foo.pfb` / `<<foo.ttf`。無ければ標準の 35 フォント等の非埋め込み
    pub font_file: Option<String>,
    /// 引用符の中の PostScript 断片（`SlantFont`、`ExtendFont`）
    pub slant: Option<f64>,
    pub extend: Option<f64>,
    /// dvipdfmx の拡張（`-r`, `-w`, `-l` などの追加指定）。そのまま保持
    pub extra: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct KanjiMapEntry {
    pub tfm_name: String,
    /// CMap 名（`UniJIS-UTF16-H` など）または `Identity-H`
    pub cmap: String,
    /// フォントファイル。`:0:` のような TTC の添字を含み得る
    pub font_file: String,
}

#[derive(Debug, Default, Clone)]
pub struct FontMap {
    pub entries: HashMap<String, MapEntry>,
    pub kanji: HashMap<String, KanjiMapEntry>,
}

impl FontMap {
    /// `pdftex.map` 形式を読み足す
    pub fn add_pdftex_map(&mut self, text: &str) {
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('%') || line.starts_with('#') {
                continue;
            }
            if let Some(e) = parse_pdftex_line(line) {
                self.entries.insert(e.tfm_name.clone(), e);
            }
        }
    }

    /// `kanjix.map` 形式を読み足す
    pub fn add_kanji_map(&mut self, text: &str) {
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('%') || line.starts_with('#') {
                continue;
            }
            let mut it = line.split_whitespace();
            if let (Some(tfm), Some(cmap), Some(file)) = (it.next(), it.next(), it.next()) {
                self.kanji.insert(
                    tfm.to_string(),
                    KanjiMapEntry {
                        tfm_name: tfm.to_string(),
                        cmap: cmap.to_string(),
                        font_file: file.to_string(),
                    },
                );
            }
        }
    }

    pub fn get(&self, tfm_name: &str) -> Option<&MapEntry> {
        self.entries.get(tfm_name)
    }

    pub fn get_kanji(&self, tfm_name: &str) -> Option<&KanjiMapEntry> {
        self.kanji.get(tfm_name)
    }
}

fn parse_pdftex_line(line: &str) -> Option<MapEntry> {
    let mut e = MapEntry::default();
    // 引用符の中を先に抜く
    let mut quoted = String::new();
    let mut rest = line.to_string();
    if let Some(q1) = line.find('"') {
        if let Some(q2) = line[q1 + 1..].find('"') {
            quoted = line[q1 + 1..q1 + 1 + q2].to_string();
            rest = format!("{} {}", &line[..q1], &line[q1 + 2 + q2..]);
        }
    }
    let mut words = rest.split_whitespace();
    e.tfm_name = words.next()?.to_string();
    for w in words {
        if let Some(f) = w.strip_prefix("<<") {
            e.font_file = Some(f.to_string());
        } else if let Some(f) = w.strip_prefix("<[") {
            e.encoding_file = Some(f.to_string());
        } else if let Some(f) = w.strip_prefix('<') {
            if f.ends_with(".enc") {
                e.encoding_file = Some(f.to_string());
            } else {
                e.font_file = Some(f.to_string());
            }
        } else if w.starts_with('-') {
            e.extra.push(w.to_string());
        } else if e.ps_name.is_empty() {
            e.ps_name = w.to_string();
        } else {
            e.extra.push(w.to_string());
        }
    }
    if e.ps_name.is_empty() {
        e.ps_name = e.tfm_name.clone();
    }
    // "0.167 SlantFont" / "1.2 ExtendFont"
    let toks: Vec<&str> = quoted.split_whitespace().collect();
    for (i, t) in toks.iter().enumerate() {
        if i == 0 {
            continue;
        }
        let v = toks[i - 1].parse::<f64>().ok();
        match *t {
            "SlantFont" => e.slant = v,
            "ExtendFont" => e.extend = v,
            _ => {}
        }
    }
    Some(e)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_typical_pdftex_map_lines() {
        let mut m = FontMap::default();
        m.add_pdftex_map(
            "cmr10 CMR10 <cmr10.pfb\nptmr8r Times-Roman \"TeXBase1Encoding ReEncodeFont\" <8r.enc <ptmr8a.pfb\nptmro8r Times-Roman \".167 SlantFont TeXBase1Encoding ReEncodeFont\" <8r.enc <ptmr8a.pfb\nlmr10 LMRoman10-Regular <lmr10.pfb\n",
        );
        let c = m.get("cmr10").unwrap();
        assert_eq!(c.font_file.as_deref(), Some("cmr10.pfb"));
        assert_eq!(c.encoding_file, None);
        let t = m.get("ptmr8r").unwrap();
        assert_eq!(t.ps_name, "Times-Roman");
        assert_eq!(t.encoding_file.as_deref(), Some("8r.enc"));
        assert_eq!(t.font_file.as_deref(), Some("ptmr8a.pfb"));
        assert_eq!(m.get("ptmro8r").unwrap().slant, Some(0.167));
    }

    #[test]
    fn parses_kanjix_map_lines() {
        let mut m = FontMap::default();
        m.add_kanji_map("% comment\nrml H HaranoAjiMincho-Regular.otf\nupjisr-h UniJIS-UTF16-H HaranoAjiMincho-Regular.otf\nuprml-v UniJIS-UTF16-V :0:HaranoAjiMincho-Regular.otf\n");
        assert_eq!(m.get_kanji("upjisr-h").unwrap().cmap, "UniJIS-UTF16-H");
        assert_eq!(
            m.get_kanji("uprml-v").unwrap().font_file,
            ":0:HaranoAjiMincho-Regular.otf"
        );
    }
}
