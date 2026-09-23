//! 外部フォントに依存しない Type1 の検査。その場で PFB を組み立てる。

use sabiface_glyph::type1::Type1Font;

fn eexec_encrypt(plain: &[u8]) -> Vec<u8> {
    let mut r = 55665u16;
    plain
        .iter()
        .map(|p| {
            let c = p ^ (r >> 8) as u8;
            r = (c as u16)
                .wrapping_add(r)
                .wrapping_mul(52845)
                .wrapping_add(22719);
            c
        })
        .collect()
}

fn charstring_encrypt(plain: &[u8], len_iv: usize) -> Vec<u8> {
    let mut r = 4330u16;
    let mut data = vec![0u8; len_iv];
    data.extend_from_slice(plain);
    data.iter()
        .map(|p| {
            let c = p ^ (r >> 8) as u8;
            r = (c as u16)
                .wrapping_add(r)
                .wrapping_mul(52845)
                .wrapping_add(22719);
            c
        })
        .collect()
}

/// `private` は eexec 部の平文（先頭に 4 バイトの乱数が要る）
fn pfb(private: &[u8]) -> Vec<u8> {
    let clear = b"%!PS-AdobeFont-1.0: Probe 1.0\n/FontName /Probe def\n/FontMatrix [0.001 0 0 0.001 0 0] def\n/Encoding StandardEncoding def\ncurrentfile eexec\n";
    let encrypted = eexec_encrypt(private);
    let mut out = Vec::new();
    for (kind, data) in [(1u8, clear.as_slice()), (2, encrypted.as_slice())] {
        out.extend_from_slice(&[128, kind]);
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(data);
    }
    out.extend_from_slice(&[128, 3]);
    out
}

/// `0 500 hsbw endchar`
const A_PLAIN: [u8; 5] = [139, 248, 136, 13, 14];

#[test]
fn len_iv_minus_one_means_unencrypted_charstrings() {
    let mut private =
        b"abcd/lenIV -1 def /Subrs 0 array /CharStrings 1 dict dup begin /A 5 RD ".to_vec();
    private.extend_from_slice(&A_PLAIN);
    private.extend_from_slice(b" ND end");
    let font = Type1Font::parse(&pfb(&private)).unwrap();
    let g = font.glyph("A").unwrap();
    assert_eq!(g.advance, 500.0);
}

#[test]
fn explicit_len_iv_is_honoured() {
    for len_iv in [0usize, 1, 4, 7] {
        let cs = charstring_encrypt(&A_PLAIN, len_iv);
        let mut private = format!(
            "abcd/lenIV {len_iv} def /Subrs 0 array /CharStrings 1 dict dup begin /A {} RD ",
            cs.len()
        )
        .into_bytes();
        private.extend_from_slice(&cs);
        private.extend_from_slice(b" ND end");
        let font = Type1Font::parse(&pfb(&private)).unwrap();
        assert_eq!(font.glyph("A").unwrap().advance, 500.0, "lenIV {len_iv}");
    }
}

#[test]
fn missing_len_iv_defaults_to_four() {
    let cs = charstring_encrypt(&A_PLAIN, 4);
    let mut private = format!(
        "abcd/Subrs 0 array /CharStrings 1 dict dup begin /A {} RD ",
        cs.len()
    )
    .into_bytes();
    private.extend_from_slice(&cs);
    private.extend_from_slice(b" ND end");
    let font = Type1Font::parse(&pfb(&private)).unwrap();
    assert_eq!(font.glyph("A").unwrap().advance, 500.0);
}

#[test]
fn other_negative_len_iv_is_an_error() {
    let mut private =
        b"abcd/lenIV -2 def /Subrs 0 array /CharStrings 1 dict dup begin /A 5 RD ".to_vec();
    private.extend_from_slice(&A_PLAIN);
    private.extend_from_slice(b" ND end");
    assert!(Type1Font::parse(&pfb(&private)).is_err());
}
