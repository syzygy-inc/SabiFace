# 契約 case 台帳

品質保証指針 `qa-v0`（`SabiSeries/qa/`）の [記録テンプレート](../../SabiSeries/qa/record-template.md) に対応する、このリポジトリの予定 case。
各 case はテスト関数 1 つに対応し、実行すると台帳（`target/qa-ledger/*.tsv`、または `SABI_QA_LEDGER`）に
PASS / FAIL / BLOCKED / NOT-RUN と比較件数を残す。`scripts/qa-ledger.sh` が本表の予定件数と突き合わせる。

`required` は oracle プロファイルで必須（無ければ BLOCKED で失敗）。`optional` は追加資源に依り、`SABI_STRICT_OPTIONAL` を設定したときだけ失敗にする。
任意かどうかはこの表と `Case::optional` の宣言で決め、理由の文字列から推測しない。

| case | 契約 | プロファイル | 必須 | 参照資源 | 内容 |
|---|---|---|---|---|---|
| FACE-TFM-CMR10 | C-FONT | font-metrics | required | cmr10.tfm、tftopl | 計量・fontdimen・合字とカーン・全文字の幅高さ深さイタリック補正を PL と比較 |
| FACE-TFM-CMEX10 | C-FONT | font-metrics | required | cmex10.tfm | 伸長文字の表 |
| FACE-JFM-JIS | C-FONT | font-metrics | required | jis.tfm、uptftopl | 組方向、文字型、グルー |
| FACE-JFM-UPJISR-H | C-FONT | font-metrics | required | upjisr-h.tfm、uptftopl | 同上（upTeX、24 ビット符号） |
| FACE-JFM-UPJISR-V | C-FONT | font-metrics | required | upjisr-v.tfm、uptftopl | 同上（縦組） |
| FACE-VF-PSNFSS | C-FONT | font-metrics | required | ptmr7t.vf 等、vftovp | 文字数・フォント数を VPL と比較 |
| FACE-ENC-DVIPS | C-FONT | font-metrics | required | 8r.enc、ec.enc | 符号化ベクトルと StandardEncoding |
| FACE-TRUNCATED | C-RESOURCE | font-metrics | required | cmr10.tfm、ptmr7t.vf、upjisr-h.tfm | 途中で切れた入力が panic せず Err |
| FACE-T1-CMR10 | C-FONT | font-type1 | required | cmr10.pfb、cmr10.afm | 全字形の送り幅と境界箱 |
| FACE-T1-CMMI10 | C-FONT | font-type1 | required | cmmi10.pfb/.afm | 同上 |
| FACE-T1-CMSY10 | C-FONT | font-type1 | required | cmsy10.pfb/.afm | 同上 |
| FACE-T1-CMEX10 | C-FONT | font-type1 | required | cmex10.pfb/.afm | 同上 |
| FACE-T1-SEAC | C-FONT | font-type1 | required | lmr10.pfb（Latin Modern） | seac による合成字形 |
| FACE-T1-TRUNCATED | C-RESOURCE | font-type1 | required | cmr10.pfb | 途中で切れた PFB が panic しない |

外部資源に依らない検査（合成 PFB による lenIV、境界箱の解析形状、pdftex.map / kanjix.map の解釈）は
`cargo test` の単体テストとして常に実行し、台帳には載せない。

## 参照資源のロック

oracle プロファイルの基準は TeX Live 2025 の配布物。CI は Ubuntu の `texlive-binaries` / `texlive-base` /
`texlive-fonts-recommended` / `texlive-lang-japanese` / `lmodern` を導入する（`.github/workflows/ci.yml`）。
版と digest の台帳化は段階 1 の残件。
