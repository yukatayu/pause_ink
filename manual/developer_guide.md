# PauseInk 開発者ガイド

## 1. 基本方針

PauseInk は次の分離を守る前提で組んでいます。

- UI は business rule を持ち込みすぎない
- `.pauseink` の parse/save は renderer に依存しない
- FFmpeg は provider abstraction の後ろへ置く
- export worker は immutable snapshot を受ける
- portable state は `pauseink_data/` に閉じる

## 2. crate 構成

- `pauseink-domain`
  clear/page semantics、stroke/object/group、style、command history
- `pauseink-project-io`
  `.pauseink` lenient load / normalized save / unknown field preservation
- `pauseink-portable-fs`
  portable root、settings、cache/autosave/runtime path、cache cleanup helper
- `pauseink-presets-core`
  export family/profile catalog、base style preset loader
- `pauseink-fonts`
  local font discovery、Google Fonts CSS/cache helper、選択 family の lazy load
- `pauseink-template-layout`
  grapheme scale rule と guide geometry
- `pauseink-media`
  FFmpeg runtime discovery、probe、preview frame、capability parsing
- `pauseink-renderer`
  CPU-safe overlay render
- `pauseink-export`
  plan/export 実行、transparent/composite、HW try / software fallback
- `pauseink-app`
  app session と eframe/egui GUI

依存方向の基本:

`app -> {domain, project_io, portable_fs, presets_core, fonts, template_layout, media, renderer, export}`

## 3. `.pauseink` の扱い

- load は JSON5 ベースで comments / trailing commas を許容
- save は canonical JSON に正規化
- project metadata / media / settings / presets は generic JSON を残しつつ、strokes / objects / groups / clear_events は typed wrapper 化
- unknown field は可能な範囲で entity 単位に保持

主要テスト:

- `cargo test -p pauseink-project-io`
- `cargo test -p pauseink-app`

## 4. portable state

既定 layout:

```text
pauseink_data/
  config/
  cache/
    google_fonts/
    font_index/
    media_probe/
    thumbnails/
  logs/
  autosave/
  runtime/
    ffmpeg/
  temp/
```

override:

- 環境変数 `PAUSEINK_PORTABLE_ROOT`

主要テスト:

- `cargo test -p pauseink-portable-fs`

## 5. export family / profile

PauseInk は family と profile を分離しています。

- family
  codec / container / alpha / audio / required muxer / required encoder
- profile
  distribution preset、bucket ごとの bitrate / sample rate / keyframe

主な API:

- `ExportCatalog::load_builtin_from_dir`
- `ExportCatalog::profiles_for_family`
- `plan_export`
- `execute_export_with_settings`

主要テスト:

- `cargo test -p pauseink-presets-core`
- `cargo test -p pauseink-export`

## 6. built-in style preset

現実装では `presets/style_presets/*.json5` から base style preset を読み込みます。  
現在 UI で適用しているのは次の項目です。

- thickness
- color_rgba

entrance / clear / combo preset の宣言フィールドは将来拡張余地として保持していますが、v1.0 実装ではまだ active UI binding を絞っています。
renderer 側には outline / drop shadow / glow の描画処理があり、同一 object 内では outer effect を先に、stroke 本体を後に描く multi-pass compositor にしてあります。これにより、後続 stroke の outline が先行 stroke 本体を不自然に覆いにくくしています。いっぽうで inspector UI と preset loader はまだ thickness / color 中心で、effect パラメータを UI から細かく触る導線は未実装です。

## 7. FFmpeg runtime

runtime は mainline では sidecar provider 前提ですが、開発・検証では host runtime も使えます。

- `discover_runtime`
- `discover_sidecar_runtime`
- `discover_system_runtime`
- `FfprobeMediaProvider`

注意:

- host apt build は local validation 用
- release packaging の既定 runtime と同一視しない
- optional codec pack は mainline に混ぜない
- Windows で runtime 未検出時に案内する既定 sidecar layout は
  `pauseink_data/runtime/ffmpeg/windows-x86_64/ffmpeg.exe`
  `pauseink_data/runtime/ffmpeg/windows-x86_64/ffprobe.exe`
  `pauseink_data/runtime/ffmpeg/windows-x86_64/manifest.json`

## 8. template / guide preview の現在設計

- template preview の x 位置は fixed-width 仮定ではなく、選択 font の shaping 結果から拾う
- `VA` のような pair kerning を落とさないため、同一 scale run ごとに layout して grapheme ごとの自然な開始位置を取り出す
- `tracking` は shaping 後の grapheme 間へ追加オフセットとして足す
- slope は baseline の y オフセットだけでなく、glyph と slot box の回転にも反映する
- guide の次文字縦線は `GuideOverlayState` が保持し、Ctrl タップ時は horizontal guide を固定したまま `next_cell_origin_x` だけを更新する
- Ctrl guide capture は `GuideCaptureState` で保持し、modifier 押下中の複数 stroke は同一 reference object に append し、modifier release でだけ guide geometry を確定する
- 描画中の stroke は renderer の export/preview request へ混ぜず、`AppSession::current_stroke_preview` で stabilized path を取り出して `main.rs` の `egui::Painter` overlay として描く
- live stroke preview は committed overlay texture の上、template / guide overlay の下に置き、編集中の視認性と補助線の見やすさを両立する
- guide の手動解除は overlay だけでなく capture 文脈、modifier 状態、last committed bounds もまとめて捨てる
- 既存 object へ stroke を append する場合は `SetGlyphObjectStyleCommand` で object style も最新の active style へ同期し、renderer が object.style を参照しても見た目がずれないようにしている

## 9. HW fallback

現在の export 実装では composite path で次を行います。

1. `media_hwaccel_enabled` が有効か
2. runtime capability に hwaccel が見えているか
3. 条件を満たすと `-hwaccel auto` 付きで一度試行
4. 失敗時は software path へ fallback

対応テスト:

- `cargo test -p pauseink-export hardware_fallback_is_only_attempted_when_enabled_and_available`

## 10. テストと検証コマンド

日常コマンド:

- `cargo check -p pauseink-app --all-targets`
- `cargo test --workspace`
- `cargo test -p pauseink-export`
- `cargo test -p pauseink-fonts`
- `cargo test -p pauseink-template-layout`

代表的な smoke:

- transparent export: `cargo test -p pauseink-export transparent_png_sequence_export_smoke_if_host_runtime_exists`
- composite export: `cargo test -p pauseink-export composite_avi_export_smoke_if_host_runtime_exists`

## 11. CI / Release workflow

GitHub Actions は次の 2 本です。

- `.github/workflows/ci.yml`
  `main` への push と、他ブランチからの `pull_request` で `cargo check -p pauseink-app --all-targets` と `cargo test --workspace` を実行します。
- `.github/workflows/release.yml`
  tag push 時、または tag 付き commit が `main` に流入した push 時に、未完成の release 対象だけを拾って release build を走らせます。Linux / macOS / Windows の `pauseink-app` release binary を生成し、archive 化して GitHub Release へ添付します。

release artifact の packaging は `scripts/package_release_asset.py` に寄せています。  
現時点で GitHub Release に載るのは app binary archive で、portable FFmpeg sidecar の同梱はまだ別タスクです。

## 12. 新しい export profile を追加する

手順は [manual/tutorials/01_add_export_profile.md](/home/yukatayu/dev/pause_ink/manual/tutorials/01_add_export_profile.md) を参照してください。

## 13. 新しい built-in preset を追加する

手順は [manual/tutorials/02_add_builtin_preset.md](/home/yukatayu/dev/pause_ink/manual/tutorials/02_add_builtin_preset.md) を参照してください。
