//! Type1 の charstring 解釈器を、AMS Computer Modern の AFM の境界箱で検証する。

use sabiface_glyph::type1::Type1Font;
use sabiface_metrics::afm::Afm;
use std::process::Command;

fn kpsewhich(name: &str) -> Option<std::path::PathBuf> {
    let out = Command::new("kpsewhich").arg(name).output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(std::path::PathBuf::from(s))
    }
}

fn check_font(pfb: &str, afm: &str) -> Option<(usize, usize)> {
    let (Some(pfb_path), Some(afm_path)) = (kpsewhich(pfb), kpsewhich(afm)) else {
        skip(&format!("{pfb} / {afm} not found"));
        return None;
    };
    let font = Type1Font::parse(&std::fs::read(&pfb_path).unwrap()).unwrap();
    let afm = Afm::parse(&std::fs::read_to_string(&afm_path).unwrap());
    let mut checked = 0;
    let mut failures = Vec::new();
    for c in &afm.chars {
        if !font.has_glyph(&c.name) {
            failures.push(format!("{}: missing", c.name));
            continue;
        }
        let g = font
            .glyph(&c.name)
            .unwrap_or_else(|e| panic!("{}: {e}", c.name));
        checked += 1;
        if (g.advance - c.width).abs() > 0.5 {
            failures.push(format!("{}: advance {} vs {}", c.name, g.advance, c.width));
        }
        match g.outline.bbox() {
            Some(bb) => {
                let exp = c.bbox;
                let got = [bb.xmin, bb.ymin, bb.xmax, bb.ymax];
                // AFM は整数に丸めている。曲線の極値を含めた厳密な境界箱なので 1 単位の差を許す
                if got.iter().zip(exp.iter()).any(|(a, b)| (a - b).abs() > 1.0) {
                    failures.push(format!(
                        "{}: bbox {:?} vs {:?}",
                        c.name,
                        got.map(|v| v.round()),
                        exp
                    ));
                }
            }
            None => {
                if c.bbox != [0.0, 0.0, 0.0, 0.0] {
                    failures.push(format!(
                        "{}: empty outline, expected bbox {:?}",
                        c.name, c.bbox
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{pfb}: {} of {checked} glyphs differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
    Some((checked, failures.len()))
}

#[test]
fn cmr10_outlines_match_afm_bboxes() {
    if let Some((n, _)) = check_font("cmr10.pfb", "cmr10.afm") {
        assert!(n > 100);
    }
}

#[test]
fn cmmi10_and_cmsy10_outlines_match_afm_bboxes() {
    check_font("cmmi10.pfb", "cmmi10.afm");
    check_font("cmsy10.pfb", "cmsy10.afm");
}

#[test]
fn cmex10_outlines_match_afm_bboxes() {
    check_font("cmex10.pfb", "cmex10.afm");
}

#[test]
fn seac_composites_in_a_text_font() {
    // cm-super や Latin Modern の Type1 は seac を使う。無ければ飛ばす
    let Some(path) = ["lmr10.pfb", "sfrm1000.pfb"]
        .iter()
        .find_map(|n| kpsewhich(n))
    else {
        skip("no seac-using font found");
        return;
    };
    let font = Type1Font::parse(&std::fs::read(&path).unwrap()).unwrap();
    for name in ["Aacute", "eacute", "odieresis"] {
        if font.has_glyph(name) {
            let g = font.glyph(name).unwrap();
            assert!(!g.outline.is_empty(), "{name} should have an outline");
            assert!(
                g.outline.bbox().unwrap().ymax > 500.0,
                "{name} should include the accent"
            );
        }
    }
}

/// 参照環境（TeX Live、フォント）が無いときは飛ばす。`SABI_STRICT_TESTS` が設定されていれば失敗にする
fn skip(reason: &str) {
    if std::env::var_os("SABI_STRICT_TESTS").is_some() {
        panic!("required reference environment is missing: {reason}");
    }
    eprintln!("skipped: {reason}");
}
