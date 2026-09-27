//! Type1 の charstring 解釈器を、AMS Computer Modern の AFM の境界箱と送り幅で検証する。
//! 各テストは契約 case（`specification/cases.md`）。環境不足は BLOCKED、比較の完了で PASS を台帳に残す。

use sabiface_glyph::type1::Type1Font;
use sabiface_metrics::afm::Afm;
use sabiface_qa::{kpsewhich, Case};

/// 1 フォントの全字形を AFM と比べる。比べた字形数を返す
fn check_font(case: &Case, pfb: &str, afm: &str) -> Option<usize> {
    let (Some(pfb_path), Some(afm_path)) = (kpsewhich(pfb), kpsewhich(afm)) else {
        case.blocked(&format!("{pfb} / {afm} not found"));
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
        case.compared();
    }
    assert!(
        failures.is_empty(),
        "{pfb}: {} of {checked} glyphs differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
    Some(checked)
}

/// FACE-T1-CMR10
#[test]
fn cmr10_outlines_match_afm_bboxes() {
    let case = Case::required("FACE-T1-CMR10", &["C-FONT"]);
    if let Some(n) = check_font(&case, "cmr10.pfb", "cmr10.afm") {
        assert!(n > 100);
        case.done();
    }
}

/// FACE-T1-CMMI10
#[test]
fn cmmi10_outlines_match_afm_bboxes() {
    let case = Case::required("FACE-T1-CMMI10", &["C-FONT"]);
    if check_font(&case, "cmmi10.pfb", "cmmi10.afm").is_some() {
        case.done();
    }
}

/// FACE-T1-CMSY10
#[test]
fn cmsy10_outlines_match_afm_bboxes() {
    let case = Case::required("FACE-T1-CMSY10", &["C-FONT"]);
    if check_font(&case, "cmsy10.pfb", "cmsy10.afm").is_some() {
        case.done();
    }
}

/// FACE-T1-CMEX10
#[test]
fn cmex10_outlines_match_afm_bboxes() {
    let case = Case::required("FACE-T1-CMEX10", &["C-FONT"]);
    if check_font(&case, "cmex10.pfb", "cmex10.afm").is_some() {
        case.done();
    }
}

/// FACE-T1-SEAC: 合成字形（seac）。Latin Modern の Type1 が要る
#[test]
fn seac_composites_in_a_text_font() {
    let case = Case::required("FACE-T1-SEAC", &["C-FONT"]);
    let Some(path) = ["lmr10.pfb", "sfrm1000.pfb"]
        .iter()
        .find_map(|n| kpsewhich(n))
    else {
        return case.blocked("no seac-using font (lmr10.pfb / sfrm1000.pfb) found");
    };
    let font = Type1Font::parse(&std::fs::read(&path).unwrap()).unwrap();
    for name in ["Aacute", "eacute", "odieresis"] {
        assert!(font.has_glyph(name), "{name} should exist in {path:?}");
        let g = font.glyph(name).unwrap();
        assert!(!g.outline.is_empty(), "{name} should have an outline");
        assert!(
            g.outline.bbox().unwrap().ymax > 500.0,
            "{name} should include the accent"
        );
        case.compared();
    }
    case.done();
}

/// FACE-T1-TRUNCATED: 実フォントの途中で切れた入力は panic ではなく Err（C-RESOURCE）
#[test]
fn truncated_pfb_does_not_panic() {
    let case = Case::required("FACE-T1-TRUNCATED", &["C-RESOURCE"]);
    let Some(path) = kpsewhich("cmr10.pfb") else {
        return case.blocked("cmr10.pfb not found");
    };
    let data = std::fs::read(path).unwrap();
    // 全長は 30 KB 程度。素数の歩幅で切る
    for len in (0..data.len()).step_by(97) {
        let r = std::panic::catch_unwind(|| {
            // 途中で切れていても構文として閉じていれば Ok になり得る。panic しないことを確かめる
            let _ = Type1Font::parse(&data[..len]).map(|f| f.glyph("A").ok());
        });
        assert!(r.is_ok(), "PFB truncated at {len} panicked");
        case.compared();
    }
    case.done();
}
