# 固定決定

| 項目 | 決定 |
|---|---|
| clear の挿入 | v1.0 では手動のみ |
| clear の範囲 | screen-wide |
| 部分 clear | v1.0 には入れない |
| 最終出力の見た目の主な出典 | ユーザーの stroke data |
| フォントの役割 | template / layout / underlay のみに限定 |
| プロジェクト拡張子 | `.pauseink` |
| プロジェクト形式 | 人間が読める JSON5 風テキスト |
| load 挙動 | 寛容に読む |
| save 挙動 | 正規化された canonical save |
| 未知フィールド | 可能な範囲で保持 |
| undo 深さ | 設定可能、既定 256 |
| 状態の置き場 | executable-local portable root |
| Google Fonts | キャッシュ付きで対応 |
| ペン圧 | v1.0 では対象外 |
| stroke smoothing | v1.0 で必須 |
| effect scripting | v1.0 では対象外 |
| GPU 利用 | UI/preview と media acceleration を別トグルにする |
| 最終合成経路 | CPU 安全な baseline |
| media runtime | FFmpeg sidecar/provider |
| mainline での FFmpeg 取得 | アプリが自動ダウンロードしない |
| UI モデル | 単一ウィンドウ |
| export family | WebM VP9/AV1、MP4 AV1 advanced、ProRes、PNG seq、AVI MJPEG |
| Adobe 寄り出力 | ProRes 422 HQ、ProRes 4444、PNG sequence |
| H.264/HEVC | optional codec-pack 領域であり mainline 前提ではない |
