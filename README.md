# SabiFace

TeX 処理系が使うフォントを「読む」側のライブラリ群。フォントを「作る」側（METAFONT の姉妹実装）は別リポジトリに予約している。
ラスタライズと整形（シェーピング）は含まない。DVI にも PDF にも依存しない。

| クレート | 内容 |
|---|---|
| `sabiface-metrics` | メトリクスと符号化: TFM、JFM（pTeX / upTeX、縦組）、VF（仮想フォント）、AFM、`.enc`（PostScript 符号化ベクトル） |
| `sabiface-glyph` | 字形の供給源: Type1（`.pfb` / `.pfa`、charstring 解釈器、seac と flex を含む）、CFF / TrueType（ttf-parser 経由）、PK（予定） |
| `sabiface-map` | フォントの解決: `pdftex.map` 系と `kanjix.map` の解釈、TeX フォント名からファイルと符号化への対応、部分集合化（予定） |

設計は `specification/design.md`。

## 検証の方針

形式ごとに TeX Live の参照実装を基準にする。テストは `kpsewhich` で TeX Live のファイルを探し、
無ければ飛ばす（CI では TeX Live の有無で結果が変わらないよう、飛ばした件数を報告する）。

- TFM / JFM / VF: `tftopl`、`uptftopl`、`vftovp` の出力（PL / VPL）と突き合わせる。
- Type1: AMS の Computer Modern の `.afm` にある字形ごとの境界箱と、charstring 解釈器が作った輪郭の境界箱を突き合わせる。

## 開発

```sh
cargo test --workspace
cargo clippy --workspace --all-targets
```
