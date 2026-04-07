# PauseInk ハンドオフ実装リポジトリ

製品名: **PauseInk**  
プロジェクト拡張子: **`.pauseink`**

この repository は、PauseInk v1.0 を仕様固定済みの handoff package として実装していくための Rust workspace です。  
現在は single-window のデスクトップアプリ、`.pauseink` 保存/読込、free ink、guide/template 補助、manual clear、transparent/composite export、portable data 管理、transport bar、Undo/Redo shortcut、template font dropdown、effect editor、出現速度 editor、`先端アクセント` editor、project ごとの style/entrance/template/guide 状態保存、style/entrance の分離 preset と field-level 継承/上書き/reset、user preset CRUD、outline 起点の複数選択 / flat group / z-order foundation、same page の連続筆記 auto-group まで接続されています。

## PauseInk とは

PauseInk は、ローカル動画を一時停止しながら手書き注釈を重ね、必要に応じて再生し、そのまま動画合成または注釈レイヤー単体として書き出すための desktop-first アプリです。

想定用途:

- Vlog への手書き補足
- ゆるい実況や commentary overlay
- VOICEROID 風の手書き説明
- シーン局所の素早い scribble annotation

whiteboard アプリでも、通常フォント置換アプリでもありません。  
最終表示はユーザー自身の stroke data を主とします。

## 現在の実装範囲

- `.pauseink` の lenient load / normalized save / entity-level unknown field 保持
- bounded undo/redo
- free ink capture と shift grouping
- manual clear event による screen-wide clear
- guide capture と Ctrl タップによる次文字縦ガイド送り
- template slot preview、読み込み済み font dropdown、実 font shaping ベースの字幅/字詰め
- single-window GUI
- transport bar と seek slider
- autosave cadence と recovery prompt
- portable root (`pauseink_data/`) 配下への状態集約
- local font discovery
- Google Fonts configured family 管理と portable cache
- export family / distribution profile の分離
- transparent PNG sequence export
- composite AVI smoke export
- ProRes 4444 / PNG sequence 向け alpha export path
- cache manager / runtime diagnostics / preferences UI
- built-in + user style preset の overlay 読み込み、追加保存、上書き保存、削除
- built-in + user entrance preset の overlay 読み込み、追加保存、上書き保存、削除
- outline / drop shadow / glow / blend mode の inspector 編集
- entrance kind / scope / order / duration mode / duration / speed scalar の inspector 編集
- 下部 `オブジェクト一覧` からの複数選択、group / ungroup、`背面へ` / `一つ後ろ` / `一つ前` / `前面へ`
- 同じ page・同じ style / entrance の連続筆記を flat group として自動でまとめる auto-group
- `.pauseink` / `settings.json5` への resolved base style snapshot、resolved entrance snapshot、選択 preset ID、field-level binding state、template text/font/layout、guide 傾きの保存と復元

## まだ mainline packaging に含めていないもの

- FFmpeg sidecar runtime 自体の同梱
- optional codec pack の正式導線
- Windows 向け release packaging
- clear / combo preset の専用 UI 適用
- post-action chain
- partial clear
- pen pressure

## repository 構成

- `AGENTS.md`
  PauseInk 実装時の最優先運用ルール
- `.docs/`
  仕様、architecture、testing、implementation plan
- `crates/domain`
  typed domain model、command history、clear/page semantics
- `crates/project_io`
  `.pauseink` load/save と unknown field 保持
- `crates/portable_fs`
  portable root、settings、cache/autosave/runtime path
- `crates/presets_core`
  export profile catalog と style / entrance / clear / combo preset loader
- `crates/fonts`
  local font discovery、Google Fonts CSS/cache helper
- `crates/template_layout`
  template slot / guide geometry
- `crates/media`
  FFmpeg runtime discovery、probe、preview frame
- `crates/renderer`
  CPU-safe overlay renderer
- `crates/export`
  export planning と transparent/composite 実行
- `crates/app`
  app session と eframe/egui GUI
- `manual/`
  user / developer guide と tutorials
- `presets/`
  export profile / style preset / entrance preset / clear preset / combo preset 定義
- `samples/`
  sample project / settings
- `docs/implementation_report_v1.0.0.md`
  実装ログと検証ログ
- `progress.md`
  フェーズ進捗と概算率

## 使い方の最短経路

1. `cargo run -p pauseink-app`
2. アプリ上部の `メディア読込` で動画を開く
3. 中央キャンバスへ直接描く
4. `全消去` で page 境界を追加する
5. 必要なら右ペインで style preset と entrance preset を個別に適用し、effect / 出現速度 / 先端アクセントも調整してから user preset として保存する
6. `保存` で `.pauseink` を保存する
7. 上部直下の transport bar で再生 / 一時停止とシークを行う
8. 右ペインの `書き出し` から family / profile を選び、transparent または composite export を実行する

## build / test

主要コマンド:

- `cargo check -p pauseink-app --all-targets`
- `cargo test --workspace`
- `cargo test -p pauseink-export`
- `cargo test -p pauseink-fonts`

実 export smoke は `pauseink-export` の test で行っています。  
詳細な検証ログは [docs/implementation_report_v1.0.0.md](/home/yukatayu/dev/pause_ink/docs/implementation_report_v1.0.0.md) を参照してください。

## portable data

既定の mutable state は executable 直下の `pauseink_data/` に集約します。

```text
<executable dir>/
  pauseink_data/
    config/
      style_presets/
      entrance_presets/
      clear_presets/
      combo_presets/
    cache/
    logs/
    autosave/
    runtime/
    temp/
```

開発/テストでは環境変数 `PAUSEINK_PORTABLE_ROOT` で上書きできます。

## export 方針

PauseInk は export を次の 2 層に分けます。

- family: container / codec family
- profile: distribution / delivery preset

mainline family:

- WebM / VP9 / Opus
- WebM / AV1 / Opus
- MP4 / AV1 / AAC-LC
- MOV / ProRes 422 HQ / PCM
- MOV / ProRes 4444 / PCM
- PNG Sequence / RGBA
- AVI / MJPEG / PCM

profile:

- 低
- 中
- 高
- YouTube
- X
- Instagram
- Adobe 編集
- Adobe アルファ
- カスタム

非 `カスタム` profile では数値欄は計算結果の表示のみ、`カスタム` では直接編集できます。

## ライセンス / runtime

- app core は MIT-friendly な構成を維持する
- FFmpeg は provider abstraction の後ろに置く
- host の apt `ffmpeg` は検証用であり、mainline release 前提にはしない
- runtime 診断から再検出でき、Windows / macOS / Linux の代表的な system path も探索する
- H.264 / HEVC は optional codec pack 扱いを維持する

## 参照

- 実装進捗: [progress.md](/home/yukatayu/dev/pause_ink/progress.md)
- 実装レポート: [docs/implementation_report_v1.0.0.md](/home/yukatayu/dev/pause_ink/docs/implementation_report_v1.0.0.md)
- ユーザーガイド: [manual/user_guide.md](/home/yukatayu/dev/pause_ink/manual/user_guide.md)
- 開発者ガイド: [manual/developer_guide.md](/home/yukatayu/dev/pause_ink/manual/developer_guide.md)
