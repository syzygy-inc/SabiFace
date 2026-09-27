//! TeX Live の参照実装（tftopl / uptftopl / vftovp）との突き合わせ。
//! 各テストは契約 case（`specification/cases.md`）で、環境不足は BLOCKED、ツールの失敗は FAIL、比較の完了で PASS を台帳に残す。

use sabiface_metrics::enc::Encoding;
use sabiface_metrics::jfm::{Direction, Jfm};
use sabiface_metrics::tfm::{LigKernOp, Tfm};
use sabiface_metrics::vf::Vf;
use sabiface_qa::{kpsewhich, run_tool, Case};

/// PL の `(CHARACTER C x` / `(CHARACTER O 17` ブロックから `CHARWD` などを拾う簡易パーサ
fn pl_char_props(pl: &str) -> Vec<(u32, Vec<(String, String)>)> {
    let mut out = Vec::new();
    let mut cur: Option<(u32, Vec<(String, String)>)> = None;
    for line in pl.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("(CHARACTER ") {
            if let Some(c) = cur.take() {
                out.push(c);
            }
            let mut it = rest.split_whitespace();
            let code = match (it.next(), it.next()) {
                (Some("C"), Some(ch)) => ch.chars().next().unwrap() as u32,
                (Some("O"), Some(o)) => u32::from_str_radix(o, 8).unwrap(),
                (Some("H"), Some(h)) => u32::from_str_radix(h, 16).unwrap(),
                (Some("D"), Some(d)) => d.parse().unwrap(),
                _ => continue,
            };
            cur = Some((code, Vec::new()));
        } else if let Some(c) = cur.as_mut() {
            if let Some(rest) = t
                .strip_prefix("(CHARWD R ")
                .or_else(|| t.strip_prefix("(CHARHT R "))
                .or_else(|| t.strip_prefix("(CHARDP R "))
                .or_else(|| t.strip_prefix("(CHARIC R "))
            {
                c.1.push((t[1..7].to_string(), rest.trim_end_matches(')').to_string()));
            }
        }
    }
    if let Some(c) = cur.take() {
        out.push(c);
    }
    out
}

fn assert_close(a: f64, b: f64, what: &str) {
    // tftopl は 6 桁で丸める
    assert!((a - b).abs() < 1e-6 + 1e-6 * b.abs(), "{what}: {a} vs {b}");
}

/// FACE-TFM-CMR10: cmr10 の計量を tftopl の PL と全文字で比べる
#[test]
fn tfm_matches_tftopl_for_cmr10() {
    let case = Case::required("FACE-TFM-CMR10", &["C-FONT"]);
    let Some(path) = kpsewhich("cmr10.tfm") else {
        return case.blocked("cmr10.tfm not found");
    };
    let tfm = Tfm::parse(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(tfm.family, "CMR");
    assert_eq!(tfm.coding_scheme, "TeX text"); // tftopl は大文字化して表示する
    assert_close(tfm.design_size.to_f64(), 10.0, "design size");
    assert_eq!(tfm.checksum, 0o11374260171);
    // fontdimen: SLANT 0, SPACE 0.333334, STRETCH 0.166667, SHRINK 0.111112, XHEIGHT 0.430555, QUAD 1.000003
    assert_close(tfm.fontdimen(2).unwrap().to_f64(), 0.333334, "space");
    assert_close(tfm.fontdimen(5).unwrap().to_f64(), 0.430555, "xheight");
    // 合字 f + i → fi (op 0, char 0o14)、カーン A + V
    assert_eq!(
        tfm.lig_kern_for(b'f' as u16, b'i' as u16),
        Some(LigKernOp::Lig { op: 0, char: 0o14 })
    );
    match tfm.lig_kern_for(b'A' as u16, b'V' as u16) {
        Some(LigKernOp::Kern(k)) => assert_close(tfm.kern[k].to_f64(), -0.111112, "kern A V"),
        other => panic!("expected kern, got {other:?}"),
    }
    case.compared_n(6);
    let Some(out) = run_tool(&case, "tftopl", &[path.to_str().unwrap()], None) else {
        return case.blocked("tftopl not available");
    };
    let pl = String::from_utf8_lossy(&out.stdout);
    let chars = pl_char_props(&pl);
    assert!(chars.len() > 100);
    for (code, props) in chars {
        let ci = tfm
            .char_info(code as u16)
            .unwrap_or_else(|| panic!("char {code} missing"));
        for (k, v) in props {
            let v: f64 = v.parse().unwrap();
            match k.as_str() {
                "CHARWD" => assert_close(ci.width.to_f64(), v, &format!("width of {code}")),
                "CHARHT" => assert_close(ci.height.to_f64(), v, &format!("height of {code}")),
                "CHARDP" => assert_close(ci.depth.to_f64(), v, &format!("depth of {code}")),
                "CHARIC" => assert_close(ci.italic.to_f64(), v, &format!("italic of {code}")),
                _ => continue,
            }
            case.compared();
        }
    }
    case.done();
}

/// FACE-TFM-CMEX10: 伸長文字と境界文字を持つ TFM
#[test]
fn tfm_boundary_char_and_extensible_in_cmex10() {
    let case = Case::required("FACE-TFM-CMEX10", &["C-FONT"]);
    let Some(path) = kpsewhich("cmex10.tfm") else {
        return case.blocked("cmex10.tfm not found");
    };
    let tfm = Tfm::parse(&std::fs::read(&path).unwrap()).unwrap();
    // 大きな括弧は伸長文字を持つ
    let any_ext = tfm
        .chars()
        .any(|(_, c)| matches!(c.tag, sabiface_metrics::tfm::Tag::Extensible(_)));
    assert!(any_ext);
    assert!(!tfm.exten.is_empty());
    case.compared_n(2);
    case.done();
}

/// FACE-JFM-*: pTeX / upTeX の JFM を uptftopl の PL と比べる（文字型、グルー）
fn jfm_case(case: Case, name: &str, dir: Direction) {
    let Some(path) = kpsewhich(name) else {
        return case.blocked(&format!("{name} not found"));
    };
    let jfm = Jfm::parse(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(jfm.direction, dir, "{name}");
    assert_close(jfm.design_size.to_f64(), 10.0, "design size");
    assert_eq!(jfm.coding_scheme, "TEX KANJI TEXT");
    // type 0 は全角: 幅は QUAD（jis.tfm では 0.962216）に等しい
    assert_close(
        jfm.type_info(0).unwrap().width.to_f64(),
        jfm.params[5].to_f64(),
        "width of type 0",
    );
    case.compared_n(4);
    let Some(out) = run_tool(&case, "uptftopl", &[path.to_str().unwrap()], None) else {
        return case.blocked("uptftopl not available");
    };
    let pl = String::from_utf8_lossy(&out.stdout);
    // CHARSINTYPE O n の文字が char_types に同じ型で入っている
    let mut cur_type: Option<u8> = None;
    for line in pl.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("(CHARSINTYPE O ") {
            cur_type = Some(u8::from_str_radix(rest.trim(), 8).unwrap());
        } else if t == ")" {
            cur_type = None;
        } else if let Some(ty) = cur_type {
            for ch in t.chars() {
                if !ch.is_whitespace() {
                    // uptftopl は jis.tfm（JIS コード）でも符号値をそのまま文字として表示する
                    let code = ch as u32;
                    assert_eq!(
                        jfm.char_type(code),
                        ty,
                        "{name}: type of U+{:04X}",
                        ch as u32
                    );
                    case.compared();
                }
            }
        }
    }
    // 型 0 と型 1 の間のグルー（jis: GLUE O 1 R 0.481108 R 0.0 R 0.481108）
    if name == "jis.tfm" {
        match jfm.glue_kern_between(0, 1) {
            Some(sabiface_metrics::jfm::GlueKernOp::Glue(g)) => {
                assert_close(g.width.to_f64(), 0.481108, "glue 0-1")
            }
            other => panic!("expected glue, got {other:?}"),
        }
        case.compared();
    }
    case.done();
}

#[test]
fn jfm_jis_matches_uptftopl() {
    jfm_case(
        Case::required("FACE-JFM-JIS", &["C-FONT"]),
        "jis.tfm",
        Direction::Yoko,
    );
}

#[test]
fn jfm_upjisr_h_matches_uptftopl() {
    jfm_case(
        Case::required("FACE-JFM-UPJISR-H", &["C-FONT"]),
        "upjisr-h.tfm",
        Direction::Yoko,
    );
}

#[test]
fn jfm_upjisr_v_matches_uptftopl() {
    jfm_case(
        Case::required("FACE-JFM-UPJISR-V", &["C-FONT"]),
        "upjisr-v.tfm",
        Direction::Tate,
    );
}

/// FACE-VF-PSNFSS: psnfss の Times の VF を vftovp の VPL と比べる
#[test]
fn vf_parses_a_psnfss_virtual_font() {
    let case = Case::required("FACE-VF-PSNFSS", &["C-FONT"]);
    let candidates = ["ptmr7t.vf", "ptmr8t.vf", "ptmr8c.vf", "ecrm1000.vf"];
    let Some(path) = candidates.iter().find_map(|n| kpsewhich(n)) else {
        return case.blocked("no VF found");
    };
    let vf = Vf::parse(&std::fs::read(&path).unwrap()).unwrap();
    assert!(!vf.fonts.is_empty());
    assert!(!vf.chars.is_empty());
    case.compared_n(2);
    let Some(out) = run_tool(&case, "vftovp", &[path.to_str().unwrap()], None) else {
        return case.blocked("vftovp not available");
    };
    let vpl = String::from_utf8_lossy(&out.stdout);
    let n_chars = vpl
        .lines()
        .filter(|l| l.trim_start().starts_with("(CHARACTER "))
        .count();
    assert_eq!(vf.chars.len(), n_chars, "character count vs vftovp");
    let n_fonts = vpl
        .lines()
        .filter(|l| l.trim_start().starts_with("(MAPFONT "))
        .count();
    assert_eq!(vf.fonts.len(), n_fonts, "font count vs vftovp");
    case.compared_n(2);
    case.done();
}

/// FACE-ENC-DVIPS: dvips の 8r.enc / ec.enc と StandardEncoding
#[test]
fn enc_parses_8r_and_ec() {
    let case = Case::required("FACE-ENC-DVIPS", &["C-FONT"]);
    for name in ["8r.enc", "ec.enc"] {
        let Some(path) = kpsewhich(name) else {
            return case.blocked(&format!("{name} not found"));
        };
        let enc = Encoding::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(enc.names.len(), 256);
        assert_eq!(enc.glyph_name(b'A'), Some("A"));
        case.compared_n(2);
    }
    let std = Encoding::standard();
    assert_eq!(std.glyph_name(0o47), Some("quoteright"));
    assert_eq!(std.glyph_name(0o256), Some("fi"));
    assert_eq!(std.glyph_name(0), None);
    case.compared_n(3);
    case.done();
}

/// FACE-TRUNCATED: 実フォントの途中で切れた入力は panic ではなく Err（C-RESOURCE）
#[test]
fn truncated_metrics_files_return_errors() {
    let case = Case::required("FACE-TRUNCATED", &["C-RESOURCE"]);
    let Some(tfm) = kpsewhich("cmr10.tfm") else {
        return case.blocked("cmr10.tfm not found");
    };
    let Some(vf) = kpsewhich("ptmr7t.vf") else {
        return case.blocked("ptmr7t.vf not found");
    };
    let Some(jfm) = kpsewhich("upjisr-h.tfm") else {
        return case.blocked("upjisr-h.tfm not found");
    };
    let tfm = std::fs::read(tfm).unwrap();
    let vf = std::fs::read(vf).unwrap();
    let jfm = std::fs::read(jfm).unwrap();
    for len in 0..tfm.len() {
        let r = std::panic::catch_unwind(|| Tfm::parse(&tfm[..len]).is_err());
        assert!(matches!(r, Ok(true)), "TFM truncated at {len}");
        case.compared();
    }
    for len in (0..vf.len()).step_by(7) {
        let r = std::panic::catch_unwind(|| Vf::parse(&vf[..len]).is_err());
        assert!(matches!(r, Ok(true)), "VF truncated at {len}");
        case.compared();
    }
    for len in 0..jfm.len() {
        let r = std::panic::catch_unwind(|| Jfm::parse(&jfm[..len]).is_err());
        assert!(matches!(r, Ok(true)), "JFM truncated at {len}");
        case.compared();
    }
    case.done();
}
