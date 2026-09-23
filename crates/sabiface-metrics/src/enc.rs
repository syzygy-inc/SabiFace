//! PostScript の符号化ベクトル（dvips の `.enc`）。`/Name [ /a /b ... ] def` の 256 個の字形名。

use crate::{err, ParseError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Encoding {
    pub name: String,
    /// 256 個の字形名。`.notdef` は None
    pub names: Vec<Option<String>>,
}

impl Encoding {
    pub fn parse(text: &str) -> Result<Encoding, ParseError> {
        // コメントを落とす
        let stripped: String = text
            .lines()
            .map(|l| l.split('%').next().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n");
        let start = stripped.find('/').ok_or_else(|| ParseError {
            offset: 0,
            message: "no encoding name".into(),
        })?;
        let after = &stripped[start + 1..];
        let name_end = after
            .find(|c: char| c.is_whitespace() || c == '[')
            .unwrap_or(after.len());
        let name = after[..name_end].to_string();
        let lb = stripped.find('[').ok_or_else(|| ParseError {
            offset: 0,
            message: "no '['".into(),
        })?;
        let rb = stripped[lb..]
            .find(']')
            .map(|i| lb + i)
            .ok_or_else(|| ParseError {
                offset: lb,
                message: "no ']'".into(),
            })?;
        let mut names = Vec::with_capacity(256);
        for tok in stripped[lb + 1..rb].split_whitespace() {
            let n = tok.strip_prefix('/').ok_or_else(|| ParseError {
                offset: lb,
                message: format!("bad token {tok}"),
            })?;
            names.push(if n == ".notdef" {
                None
            } else {
                Some(n.to_string())
            });
        }
        if names.len() != 256 {
            return err(
                lb,
                format!("encoding has {} entries, expected 256", names.len()),
            );
        }
        Ok(Encoding { name, names })
    }

    pub fn glyph_name(&self, code: u8) -> Option<&str> {
        self.names[code as usize].as_deref()
    }

    /// Adobe StandardEncoding（Type1 の既定符号化）
    pub fn standard() -> Encoding {
        let mut names: Vec<Option<String>> = vec![None; 256];
        for (i, n) in STANDARD.iter().enumerate() {
            if !n.is_empty() {
                names[i] = Some(n.to_string());
            }
        }
        Encoding {
            name: "StandardEncoding".into(),
            names,
        }
    }
}

/// Adobe StandardEncoding の 256 項目（空文字列は .notdef）
const STANDARD: [&str; 256] = {
    let mut t = [""; 256];
    let lower: [(usize, &str); 95] = [
        (32, "space"),
        (33, "exclam"),
        (34, "quotedbl"),
        (35, "numbersign"),
        (36, "dollar"),
        (37, "percent"),
        (38, "ampersand"),
        (39, "quoteright"),
        (40, "parenleft"),
        (41, "parenright"),
        (42, "asterisk"),
        (43, "plus"),
        (44, "comma"),
        (45, "hyphen"),
        (46, "period"),
        (47, "slash"),
        (48, "zero"),
        (49, "one"),
        (50, "two"),
        (51, "three"),
        (52, "four"),
        (53, "five"),
        (54, "six"),
        (55, "seven"),
        (56, "eight"),
        (57, "nine"),
        (58, "colon"),
        (59, "semicolon"),
        (60, "less"),
        (61, "equal"),
        (62, "greater"),
        (63, "question"),
        (64, "at"),
        (65, "A"),
        (66, "B"),
        (67, "C"),
        (68, "D"),
        (69, "E"),
        (70, "F"),
        (71, "G"),
        (72, "H"),
        (73, "I"),
        (74, "J"),
        (75, "K"),
        (76, "L"),
        (77, "M"),
        (78, "N"),
        (79, "O"),
        (80, "P"),
        (81, "Q"),
        (82, "R"),
        (83, "S"),
        (84, "T"),
        (85, "U"),
        (86, "V"),
        (87, "W"),
        (88, "X"),
        (89, "Y"),
        (90, "Z"),
        (91, "bracketleft"),
        (92, "backslash"),
        (93, "bracketright"),
        (94, "asciicircum"),
        (95, "underscore"),
        (96, "quoteleft"),
        (97, "a"),
        (98, "b"),
        (99, "c"),
        (100, "d"),
        (101, "e"),
        (102, "f"),
        (103, "g"),
        (104, "h"),
        (105, "i"),
        (106, "j"),
        (107, "k"),
        (108, "l"),
        (109, "m"),
        (110, "n"),
        (111, "o"),
        (112, "p"),
        (113, "q"),
        (114, "r"),
        (115, "s"),
        (116, "t"),
        (117, "u"),
        (118, "v"),
        (119, "w"),
        (120, "x"),
        (121, "y"),
        (122, "z"),
        (123, "braceleft"),
        (124, "bar"),
        (125, "braceright"),
        (126, "asciitilde"),
    ];
    let mut i = 0;
    while i < lower.len() {
        t[lower[i].0] = lower[i].1;
        i += 1;
    }
    let upper: [(usize, &str); 54] = [
        (161, "exclamdown"),
        (162, "cent"),
        (163, "sterling"),
        (164, "fraction"),
        (165, "yen"),
        (166, "florin"),
        (167, "section"),
        (168, "currency"),
        (169, "quotesingle"),
        (170, "quotedblleft"),
        (171, "guillemotleft"),
        (172, "guilsinglleft"),
        (173, "guilsinglright"),
        (174, "fi"),
        (175, "fl"),
        (177, "endash"),
        (178, "dagger"),
        (179, "daggerdbl"),
        (180, "periodcentered"),
        (182, "paragraph"),
        (183, "bullet"),
        (184, "quotesinglbase"),
        (185, "quotedblbase"),
        (186, "quotedblright"),
        (187, "guillemotright"),
        (188, "ellipsis"),
        (189, "perthousand"),
        (191, "questiondown"),
        (193, "grave"),
        (194, "acute"),
        (195, "circumflex"),
        (196, "tilde"),
        (197, "macron"),
        (198, "breve"),
        (199, "dotaccent"),
        (200, "dieresis"),
        (202, "ring"),
        (203, "cedilla"),
        (205, "hungarumlaut"),
        (206, "ogonek"),
        (207, "caron"),
        (208, "emdash"),
        (225, "AE"),
        (227, "ordfeminine"),
        (232, "Lslash"),
        (233, "Oslash"),
        (234, "OE"),
        (235, "ordmasculine"),
        (241, "ae"),
        (245, "dotlessi"),
        (248, "lslash"),
        (249, "oslash"),
        (250, "oe"),
        (251, "germandbls"),
    ];
    let mut j = 0;
    while j < upper.len() {
        t[upper[j].0] = upper[j].1;
        j += 1;
    }
    t
};
