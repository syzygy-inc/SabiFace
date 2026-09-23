# SabiFace 設計 v0

## 位置づけ

Sabi 系列の分担:

- SabiTeX: 組版。整形（HarfBuzz 相当）はここに留める。
- **SabiFace**: フォントを読む。メトリクス・符号化・字形の輪郭・フォントの解決。ラスタライズはしない。
- SabiRender: PDF 演算子の評価器と、装置非依存の描画命令列を描く後端（CPU 参照ラスタライザ、GPU）。字形の輪郭は入口側が SabiFace から供給する。
- SabiDVI / SabiPDF: SabiRender の入口。DVI（+ dvipdfmx 方言の `pdf:` special）と PDF。
- SabiFont / SabiMF / SabiMeta: フォントを作る（予約）。

## ある文字を紙面に出すための三つの問い

| 問い | 層 | 形式 |
|---|---|---|
| どれだけの幅・高さか | メトリクス | TFM、JFM、OFM（将来）、VF、AFM |
| どの字形か | 符号化 | `.enc`、CMap（将来）、OpenType cmap、字形 ID の直指定（XDV） |
| どんな形か | 字形の供給源 | Type1 charstring、CFF（Type2）、TrueType glyf、PK / GF ビットマップ |

これに「TeX フォント名からファイルと符号化を決める」解決（`.map`、`kanjix.map`）と、Web 配信のための部分集合化が加わる。

## クレートの境界

- `sabiface-metrics` と `sabiface-glyph` は互いに依存しない。VF の展開のように両方が要る処理は利用側（SabiDVI）で組む。
- `sabiface-map` は上二つに依存してよい（部分集合化で字形を読む）。
- どのクレートもラスタライズ・DVI・PDF に依存しない。`no_std` は目標にしない（`alloc` 前提の `std`）。

## 形式ごとの仕様の出典

- TFM: tex.web §539〜§575、`tftopl.web`。
- JFM: pTeX の `jfm.pdf`（ptex-manual）。upTeX の拡張 JFM は文字コードを 24 ビットにする（`01uptex_doc`）。`char_type` の 4 バイトは、
  下位 16 ビットを先頭 2 バイト、上位 8 ビットを第 3 バイト、type を第 4 バイトとして読む。pTeX の JFM は第 3 バイトが 0 なので互換。
- VF: `vftovp.web`、`vptovf.web`。
- Type1: Adobe "Type 1 Font Format"（黒本）。eexec と charstring の暗号（r = 55665 / 4330）、`lenIV`、seac、flex と hint replacement は OtherSubrs 0〜3。
- AFM: Adobe Font Metrics File Format Specification 4.1。
- `.enc`: dvips の PostScript 符号化ベクトル（`/Name [ /a /b ... ] def`）。
- `.map`: pdftex.map（`tfmname psname <opts> <enc.enc <font.pfb`）、kanjix.map（`tfmname CMap fontfile`）。

## 数値の扱い

- TFM の `fix_word` は 2^-20 単位の 32 ビット固定小数。`Fix` 型で保持し、`f64` への変換とデザインサイズによる拡大縮小を提供する。
- Type1 の座標は FontMatrix 適用前の字形単位（通常 1000 / em）。輪郭は `f64` で持つ。

## 検証

- メトリクス: TeX Live の `tftopl` / `uptftopl` / `vftovp` の PL / VPL 出力と、こちらの解析結果を数値で比較する。
- Type1: AMS Computer Modern の `.afm` の境界箱（`B llx lly urx ury`）と、輪郭から計算した厳密な境界箱（三次曲線の極値を含む）を比較する。
- テストは TeX Live のファイルを `kpsewhich` で探し、無ければ飛ばす。環境変数 `SABI_STRICT_TESTS` を設定すると、飛ばす代わりに失敗にする（参照環境を必須にする CI 用）。
- Type1 の `lenIV`（-1 = 非暗号化、0〜255）は外部フォントに依存しない合成 PFB で検査する（`tests/type1_synthetic.rs`）。
