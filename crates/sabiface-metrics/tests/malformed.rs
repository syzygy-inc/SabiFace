//! 不正入力の corpus（C-RESOURCE）。外部資源に依らず、手で組んだ最小の TFM / VF / JFM / enc を壊して、
//! 解析器が panic せずに Err を返すことを確かめる。決定的な擬似乱数の塊も通し、panic しないことだけを見る。
//! 各 case は `specification/cases.md` の「不正入力 corpus」。

use sabiface_metrics::enc::Encoding;
use sabiface_metrics::jfm::Jfm;
use sabiface_metrics::tfm::Tfm;
use sabiface_metrics::vf::Vf;
use std::panic::catch_unwind;

fn be16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_be_bytes());
}

fn be32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_be_bytes());
}

/// TFM の最小の正例: 文字 0 だけ、幅 0.5、高さ・深さ・補正 0。lh = 2、nw = 2（index 0 は 0 でなければならない）
struct MinTfm {
    lh: u16,
    bc: u16,
    ec: u16,
    nw: u16,
    nh: u16,
    nd: u16,
    ni: u16,
    nl: u16,
    nk: u16,
    ne: u16,
    np: u16,
    /// 見出しの 12 語の後に続く本体（header、char_info、各表）
    body: Vec<u8>,
    /// lf を本体から計算せず、この値で上書きする（見出しの不整合を作る）
    lf_override: Option<u16>,
}

impl MinTfm {
    fn valid() -> MinTfm {
        let mut body = Vec::new();
        be32(&mut body, 0); // checksum
        be32(&mut body, 10 << 20); // design size 10pt
        body.extend_from_slice(&[1, 0, 0, 0]); // char 0: width index 1、他 0、tag なし
        be32(&mut body, 0); // widths[0]
        be32(&mut body, 1 << 19); // widths[1] = 0.5
        be32(&mut body, 0); // heights[0]
        be32(&mut body, 0); // depths[0]
        be32(&mut body, 0); // italics[0]
        MinTfm {
            lh: 2,
            bc: 0,
            ec: 0,
            nw: 2,
            nh: 1,
            nd: 1,
            ni: 1,
            nl: 0,
            nk: 0,
            ne: 0,
            np: 0,
            body,
            lf_override: None,
        }
    }

    fn bytes(&self) -> Vec<u8> {
        let mut v = Vec::new();
        let lf = self.lf_override.unwrap_or((6 + self.body.len() / 4) as u16);
        for x in [
            lf, self.lh, self.bc, self.ec, self.nw, self.nh, self.nd, self.ni, self.nl, self.nk,
            self.ne, self.np,
        ] {
            be16(&mut v, x);
        }
        v.extend_from_slice(&self.body);
        v
    }
}

fn parses_to_err<F: FnOnce() -> bool + std::panic::UnwindSafe>(name: &str, f: F) {
    let r = catch_unwind(f);
    assert!(
        matches!(r, Ok(true)),
        "{name}: expected Err without panic, got {r:?}"
    );
}

fn does_not_panic<F: FnOnce() + std::panic::UnwindSafe>(name: &str, f: F) {
    assert!(catch_unwind(f).is_ok(), "{name}: panicked");
}

#[test]
fn the_minimal_tfm_is_valid() {
    let t = Tfm::parse(&MinTfm::valid().bytes()).expect("minimal TFM parses");
    assert_eq!((t.first_char, t.last_char), (0, 0));
    let c = t.char_info(0).expect("char 0");
    assert_eq!(c.width.0, 1 << 19);
}

#[test]
fn tfm_header_inconsistencies_are_errors_not_panics() {
    // 見出しの lf が本体と合わない
    let mut t = MinTfm::valid();
    t.lf_override = Some(20);
    let d = t.bytes();
    parses_to_err("lf larger than file", move || Tfm::parse(&d).is_err());
    // 本体が見出しより長い
    let mut d = MinTfm::valid().bytes();
    d.extend_from_slice(&[0; 4]);
    parses_to_err("trailing bytes", move || Tfm::parse(&d).is_err());
    // 表の長さの和が lf と合わない
    let mut t = MinTfm::valid();
    t.nw = 3;
    let d = t.bytes();
    parses_to_err("subfile sum", move || Tfm::parse(&d).is_err());
    // bc > ec + 1、ec >= 256
    let mut t = MinTfm::valid();
    t.bc = 2;
    let d = t.bytes();
    parses_to_err("bc > ec + 1", move || Tfm::parse(&d).is_err());
    let mut t = MinTfm::valid();
    t.ec = 256;
    let d = t.bytes();
    parses_to_err("ec >= 256", move || Tfm::parse(&d).is_err());
    // header が短い、寸法表が空
    let mut t = MinTfm::valid();
    t.lh = 1;
    t.body.drain(4..8);
    let d = t.bytes();
    parses_to_err("lh < 2", move || Tfm::parse(&d).is_err());
    let mut t = MinTfm::valid();
    t.nh = 0;
    t.body.drain(20..24);
    let d = t.bytes();
    parses_to_err("nh = 0", move || Tfm::parse(&d).is_err());
}

#[test]
fn tfm_dangling_indices_are_errors_not_panics() {
    // 幅の index が表の外
    let mut t = MinTfm::valid();
    t.body[8] = 2;
    let d = t.bytes();
    parses_to_err("width index", move || Tfm::parse(&d).is_err());
    // 高さの index が表の外
    let mut t = MinTfm::valid();
    t.body[9] = 0x10;
    let d = t.bytes();
    parses_to_err("height index", move || Tfm::parse(&d).is_err());
    // lig_kern の kern 参照が表の外（nl = 1、nk = 0）
    let mut t = MinTfm::valid();
    t.nl = 1;
    t.body.extend_from_slice(&[0, 0, 128, 0]);
    let d = t.bytes();
    parses_to_err("kern index", move || Tfm::parse(&d).is_err());
    // lig_kern プログラムの開始位置が表の外（tag 1、rem 5、nl = 1）
    let mut t = MinTfm::valid();
    t.nl = 1;
    t.body[10] = 1;
    t.body[11] = 5;
    t.body.extend_from_slice(&[0, 0, 0, 0]);
    let d = t.bytes();
    parses_to_err("lig_kern start", move || Tfm::parse(&d).is_err());
    // 伸長文字の index が表の外（tag 3、ne = 0）
    let mut t = MinTfm::valid();
    t.body[10] = 3;
    let d = t.bytes();
    parses_to_err("exten index", move || Tfm::parse(&d).is_err());
}

#[test]
fn tfm_short_inputs_are_errors_not_panics() {
    for n in [0usize, 1, 3, 12, 23, 24, 31, 55] {
        let d = MinTfm::valid().bytes()[..n].to_vec();
        parses_to_err(&format!("{n} bytes"), move || Tfm::parse(&d).is_err());
    }
}

/// VF の最小の正例: 注釈なし、フォントも文字もなし
fn min_vf() -> Vec<u8> {
    let mut v = vec![247, 202, 0];
    be32(&mut v, 0);
    be32(&mut v, 10 << 20);
    v.push(248);
    v.extend_from_slice(&[223; 4]);
    v
}

#[test]
fn vf_malformed_inputs_are_errors_not_panics() {
    assert!(Vf::parse(&min_vf()).is_ok());
    // 先頭が pre でない、id が違う
    let mut d = min_vf();
    d[0] = 0;
    parses_to_err("not pre", move || Vf::parse(&d).is_err());
    let mut d = min_vf();
    d[1] = 2;
    parses_to_err("wrong id", move || Vf::parse(&d).is_err());
    // post の前に pre（許されない命令）
    let mut d = min_vf();
    d.insert(11, 247);
    parses_to_err("pre inside", move || Vf::parse(&d).is_err());
    // long_char の pl がファイルの外
    let mut d = min_vf();
    d.truncate(11);
    d.push(242);
    be32(&mut d, 1 << 30);
    be32(&mut d, 65);
    be32(&mut d, 0);
    parses_to_err("long_char too long", move || Vf::parse(&d).is_err());
    // fnt_def の名前がファイルの外
    let mut d = min_vf();
    d.truncate(11);
    d.extend_from_slice(&[243, 0]);
    be32(&mut d, 0);
    be32(&mut d, 1 << 20);
    be32(&mut d, 10 << 20);
    d.extend_from_slice(&[0, 200]);
    parses_to_err("fnt_def name too long", move || Vf::parse(&d).is_err());
    // post が無い
    let mut d = min_vf();
    d.truncate(11);
    parses_to_err("no post", move || Vf::parse(&d).is_err());
    for n in [0usize, 1, 2, 3, 6, 10] {
        let d = min_vf()[..n].to_vec();
        parses_to_err(&format!("{n} bytes"), move || Vf::parse(&d).is_err());
    }
}

#[test]
fn jfm_malformed_inputs_are_errors_not_panics() {
    // id が JFM でない
    let mut d = Vec::new();
    be16(&mut d, 7);
    d.extend_from_slice(&[0; 26]);
    parses_to_err("bad id", move || Jfm::parse(&d).is_err());
    // bc != 0
    let mut d = Vec::new();
    for x in [11u16, 0, 7, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0] {
        be16(&mut d, x);
    }
    parses_to_err("bc != 0", move || Jfm::parse(&d).is_err());
    // lf が本体と合わない
    let mut d = Vec::new();
    for x in [11u16, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] {
        be16(&mut d, x);
    }
    parses_to_err("lf mismatch", move || Jfm::parse(&d).is_err());
    for n in [0usize, 1, 2, 5, 27] {
        let d = vec![0u8, 11][..n.min(2)].to_vec();
        let mut d = d;
        d.resize(n, 0);
        parses_to_err(&format!("{n} bytes"), move || Jfm::parse(&d).is_err());
    }
}

#[test]
fn enc_malformed_inputs_are_errors_not_panics() {
    let ok = format!("/E [{}] def", " /a".repeat(256));
    assert!(Encoding::parse(&ok).is_ok());
    for (name, text) in [
        ("empty", String::new()),
        ("no name", "[ /a ]".into()),
        ("no [", "/E /a ] def".into()),
        ("no ]", "/E [ /a /b".into()),
        ("bad token", format!("/E [{} b ] def", " /a".repeat(255))),
        ("255 entries", format!("/E [{}] def", " /a".repeat(255))),
        ("257 entries", format!("/E [{}] def", " /a".repeat(257))),
        ("only comments", "% /E [ /a ] def".into()),
    ] {
        parses_to_err(name, move || Encoding::parse(&text).is_err());
    }
}

/// 決定的な擬似乱数（xorshift）の塊。解析器は Err か Ok を返し、panic しない
#[test]
fn random_bytes_never_panic_any_parser() {
    let mut s = 0x9e37_79b9_7f4a_7c15u64;
    let mut next = move || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s
    };
    for i in 0..300 {
        let len = (next() % 400) as usize;
        let mut d = Vec::with_capacity(len);
        while d.len() < len {
            d.extend_from_slice(&next().to_le_bytes());
        }
        d.truncate(len);
        // 見出しっぽくもする: 先頭を VF / JFM の印にした変種も通す
        let mut variants = vec![d.clone()];
        if len >= 2 {
            let mut v = d.clone();
            v[0] = 247;
            v[1] = 202;
            variants.push(v);
            let mut j = d.clone();
            j[0] = 0;
            j[1] = 11;
            variants.push(j);
        }
        for (k, v) in variants.into_iter().enumerate() {
            let name = format!("random #{i}.{k} ({len} bytes)");
            let t = v.clone();
            does_not_panic(&name, move || {
                let _ = Tfm::parse(&t);
            });
            let t = v.clone();
            does_not_panic(&name, move || {
                let _ = Vf::parse(&t);
            });
            let t = v.clone();
            does_not_panic(&name, move || {
                let _ = Jfm::parse(&t);
            });
            let text = String::from_utf8_lossy(&v).into_owned();
            does_not_panic(&name, move || {
                let _ = Encoding::parse(&text);
            });
        }
    }
}
