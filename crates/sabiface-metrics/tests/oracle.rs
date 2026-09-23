//! TeX Live の参照実装（tftopl / uptftopl / vftovp）との突き合わせ。ファイルが無ければ飛ばす。

use sabiface_metrics::enc::Encoding;
use sabiface_metrics::jfm::{Direction, Jfm};
use sabiface_metrics::tfm::{LigKernOp, Tfm};
use sabiface_metrics::vf::Vf;
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

fn tool(name: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(name).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

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

#[test]
fn tfm_matches_tftopl_for_cmr10() {
    let Some(path) = kpsewhich("cmr10.tfm") else {
        skip("cmr10.tfm not found");
        return;
    };
    let tfm = Tfm::parse(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(tfm.family, "CMR");
    assert_eq!(tfm.coding_scheme, "TeX text"); // tftopl は大文字化して表示する
    assert_close(tfm.design_size.to_f64(), 10.0, "design size");
    assert_eq!(tfm.checksum, 0o11374260171);
    // fontdimen: SLANT 0, SPACE 0.333334, STRETCH 0.166667, SHRINK 0.111112, XHEIGHT 0.430555, QUAD 1.000003
    assert_close(tfm.fontdimen(2).unwrap().to_f64(), 0.333334, "space");
    assert_close(tfm.fontdimen(5).unwrap().to_f64(), 0.430555, "xheight");
    // 合字 f + i → fi (op 0, char 0o14), カーン A + V
    assert_eq!(
        tfm.lig_kern_for(b'f' as u16, b'i' as u16),
        Some(LigKernOp::Lig { op: 0, char: 0o14 })
    );
    match tfm.lig_kern_for(b'A' as u16, b'V' as u16) {
        Some(LigKernOp::Kern(k)) => assert_close(tfm.kern[k].to_f64(), -0.111112, "kern A V"),
        other => panic!("expected kern, got {other:?}"),
    }
    let Some(pl) = tool("tftopl", &[path.to_str().unwrap()]) else {
        skip("tftopl not available");
        return;
    };
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
                _ => {}
            }
        }
    }
}

#[test]
fn tfm_boundary_char_and_extensible_in_cmex10() {
    let Some(path) = kpsewhich("cmex10.tfm") else {
        return;
    };
    let tfm = Tfm::parse(&std::fs::read(&path).unwrap()).unwrap();
    // cmex10 の '(' (0o0) は list、大きな括弧は伸長文字を持つ
    let any_ext = tfm
        .chars()
        .any(|(_, c)| matches!(c.tag, sabiface_metrics::tfm::Tag::Extensible(_)));
    assert!(any_ext);
    assert!(!tfm.exten.is_empty());
}

#[test]
fn jfm_matches_uptftopl() {
    for (name, dir) in [
        ("jis.tfm", Direction::Yoko),
        ("upjisr-h.tfm", Direction::Yoko),
        ("upjisr-v.tfm", Direction::Tate),
    ] {
        let Some(path) = kpsewhich(name) else {
            skip(&format!("{name} not found"));
            continue;
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
        let Some(pl) = tool("uptftopl", &[path.to_str().unwrap()]) else {
            continue;
        };
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
        }
    }
}

#[test]
fn vf_parses_a_psnfss_virtual_font() {
    // ptmr7t.vf（psnfss の Times, T1）が代表。無ければ他の VF を探す
    let candidates = ["ptmr7t.vf", "ptmr8t.vf", "ptmr8c.vf", "ecrm1000.vf"];
    let Some(path) = candidates.iter().find_map(|n| kpsewhich(n)) else {
        skip("no VF found");
        return;
    };
    let vf = Vf::parse(&std::fs::read(&path).unwrap()).unwrap();
    assert!(!vf.fonts.is_empty());
    assert!(!vf.chars.is_empty());
    if let Some(vpl) = tool("vftovp", &[path.to_str().unwrap()]) {
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
    }
}

#[test]
fn enc_parses_8r_and_ec() {
    for name in ["8r.enc", "ec.enc"] {
        let Some(path) = kpsewhich(name) else {
            continue;
        };
        let enc = Encoding::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(enc.names.len(), 256);
        assert_eq!(enc.glyph_name(b'A'), Some("A"));
    }
    let std = Encoding::standard();
    assert_eq!(std.glyph_name(0o47), Some("quoteright"));
    assert_eq!(std.glyph_name(0o256), Some("fi"));
    assert_eq!(std.glyph_name(0), None);
}

/// 参照環境（TeX Live、フォント）が無いときは飛ばす。`SABI_STRICT_TESTS` が設定されていれば失敗にする
fn skip(reason: &str) {
    if std::env::var_os("SABI_STRICT_TESTS").is_some() {
        panic!("required reference environment is missing: {reason}");
    }
    eprintln!("skipped: {reason}");
}
