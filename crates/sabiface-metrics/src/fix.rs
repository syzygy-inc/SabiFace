//! TFM の `fix_word`（tex.web §541）: 2^-20 を単位とする 32 ビット固定小数。

/// 2^-20 単位の固定小数。デザインサイズを掛ける前の値。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub struct Fix(pub i32);

impl Fix {
    pub const ONE: Fix = Fix(1 << 20);
    pub const ZERO: Fix = Fix(0);

    pub fn to_f64(self) -> f64 {
        self.0 as f64 / (1u32 << 20) as f64
    }

    pub fn from_f64(v: f64) -> Fix {
        Fix((v * (1u32 << 20) as f64).round() as i32)
    }

    /// デザインサイズ（pt）を掛けた値（pt）。TeX が `store_scaled` で行う計算の実数版。
    pub fn scaled(self, design_size_pt: f64) -> f64 {
        self.to_f64() * design_size_pt
    }

    /// TeX の `sp`（2^-16 pt）単位に、デザインサイズ（sp）を掛けた値。tex.web §571 の `store_scaled` と同じ丸め。
    /// `z` はデザインサイズを sp で表したもの（`font_size`）。
    pub fn to_sp(self, z_sp: i32) -> i32 {
        // §571: alpha/beta による 16 ビット精度の乗算。ここでは i64 で厳密に計算し、同じ切り捨てを行う。
        let z = z_sp as i64;
        let v = self.0 as i64;
        // fix_word × z / 2^20、負の値は正の値の否定として扱う（TeX と同じ）
        if v >= 0 {
            ((v * z) >> 20) as i32
        } else {
            -(((-v) * z) >> 20) as i32
        }
    }
}

impl std::fmt::Display for Fix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_f64())
    }
}
