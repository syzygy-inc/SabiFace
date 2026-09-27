# SabiFace 内部品質保証

策定: 2026-09-24。参照設計版: `qa-v0`。系列 [契約](../../SabiSeries/qa/contracts.md) の C-FONT / C-RESULT / C-RESOURCE を担保する。描画・フォント探索ホストの制御をこのリポジトリの責任にしない。

## 内部不変条件

| 領域 | 保証する条件 | 検査する境界 |
|---|---|---|
| 共通 reader | 読取範囲、長さの checked arithmetic、確保上限 | 正常最小、末尾切断、長さ不整合、巨大宣言 |
| TFM/JFM | fix_word の符号と単位、表の添字、空文字・不存在、JFM 符号 | ゼロ/負数、境界コード、表参照、一次仕様・PL |
| VF | packet/定義の範囲と font 番号の保持 | 短長packet、切断、不正長。実行・再帰制限は利用側にも必要 |
| Type1 | eexec と charstring の区別、lenIV、Subrs、stack、再帰 | lenIV=-1/0/通常/不正負値、暗号・非暗号、subr/flex/seac |
| OpenType | index、glyph ID、単位、輪郭の妥当性 | TTC index、cmap、空輪郭、非対応形式、破損table |
| map/encoding | 解析したコード・変形・参照を捨てない | Type1/OpenType、引用、Slant/Extend、欠落資源 |
| outline | 曲線の意味、閉路、bbox の極値 | 解析形状、退化曲線、AFM 比較、負座標 |

フォントが返す情報と、それを利用側が適用する責務を分ける。SabiFace が FontMatrix を保持しても、SabiDVI で捨てれば C-FONT は不成立になるため、外側に consumer test を置く。両方に同じ変換を掛けて二重適用する修正を避ける。

## テスト構成

外部配布フォントなしでも再現する小さな TFM/JFM/VF/PFB fixture を基本にし、読める必要のある正例と受理してはいけない負例を持つ。Type1 の F1 は暗号化の有無と subroutine の両経路の回帰にする。

TeX Live の `tftopl` / `uptftopl` / `vftovp`、AFM と OpenType の参照資源は oracle プロファイルに版・digest を登録する。境界箱だけ合うが字形が違うケースを見逃さないよう、主要な輪郭・advance・字形対応も検査する。ttf-parser を呼ぶだけの層では、依存ライブラリの全性質を自前で証明したとは言わず、依存版と利用前提を記録する。

既存の実行入口:

```sh
cargo test --workspace
# 子プロセス環境に SABI_STRICT_TESTS=1 を設定して同じテストを実行する
```

strict の適合条件はすべての予定 oracle 比較の実行完了。ツールがない場合の continue、`if let Some` での黙った比較省略も対象にする。8r/ec 等の資源を optional にするなら case 台帳で事前に決め、未実行数を残す。現行の strict 設定だけでこの条件を満たすと見なさない。

## 局所的な形式化と保守

Lean の候補は固定小数の符号化/復号と範囲付き reader。証明する領域・検証済み長さと Rust の slice/確保との対応を分ける。Type1 インタプリタ全体を初期必須化せず、長さや stack 操作など責任範囲を小さくする。

parser のデータ構造・helper の形は内側の規約。外側は parse 成否・値・輪郭・診断で判定する。外部仕様を壊さない refactor で fixture や oracle の期待値を生成し直す必要がない設計にする。public 型変更時は系列の adapter と利用側で互換性を確認する。

## 当面の完了条件

oracle の不足/起動失敗/変換失敗/比較未実行を分類し、strict で取りこぼさないことを先に確認する。次に不正入力・lenIV・map の少数 fixture を必須 corpus にする。形式検証は [系列方針](../../SabiSeries/qa/formal-methods.md) に従う計画であり、現時点の証明成果ではない。
