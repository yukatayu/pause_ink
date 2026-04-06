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
    style_presets/
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
- `execute_export_with_settings_with_progress`

主要テスト:

- `cargo test -p pauseink-presets-core`
- `cargo test -p pauseink-export`

## 6. style preset

現実装では built-in preset を `presets/style_presets/*.json5` から、user preset を `pauseink_data/config/style_presets/*.json5` から読み込みます。  
読み込み順は built-in -> user overlay で、同じ `id` がある場合は user preset が優先されます。built-in は読み取り専用、user preset は GUI から追加保存 / 上書き保存 / 削除できます。  
project には mutable preset file そのものではなく、`project.presets.base_style` に resolved base style snapshot、`project.presets.entrance` に resolved entrance snapshot と任意の preset ID を保存します。template / guide の project-specific UI state は `project.settings.pauseink_editor_ui` に保存します。

現在 UI / preset apply で接続済みなのは次の項目です。

- thickness
- color_rgba
- opacity
- outline
- drop_shadow
- glow
- blend_mode
- stabilization_strength
- entrance.kind
- entrance.duration_mode
- entrance.duration_ms
- entrance.speed_scalar

renderer 側には outline / drop shadow / glow の描画処理があり、同一 object 内では outer effect を先に、stroke 本体を後に描く multi-pass compositor にしてあります。これにより、後続 stroke の outline が先行 stroke 本体を不自然に覆いにくくしています。出現時間は `fixed_total_duration` と `proportional_to_stroke_length` の 2 モードを持ち、後者は 600px を基準長として `speed_scalar` を掛けています。
未接続の残項目は reveal-head effect、post-action chain、clear / combo preset の専用 UI です。

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
- runtime 診断の `機能情報更新` / `診断を再取得` は capability 更新だけでなく runtime discovery もやり直す
- startup 時と手動再検出時の失敗理由は `last_runtime_error` として診断 UI に残す
- system runtime 探索は `PATH` に加えて代表的な既知パスも見る
  - Windows: `%LOCALAPPDATA%\\Microsoft\\WinGet\\Links`、`%LOCALAPPDATA%\\Microsoft\\WindowsApps`、`%LOCALAPPDATA%\\Microsoft\\WinGet\\Packages\\...`、`~/scoop/shims`
  - macOS: `/opt/homebrew/bin`、`/usr/local/bin`、`/opt/local/bin`、`/usr/bin`
  - Linux: `/usr/bin`、`/usr/local/bin`、`/bin`、`/snap/bin`、`~/.local/bin`、Linuxbrew 系
- sidecar の既定 layout は current platform id ベースで組み立てる。例えば Windows x86_64 では
  `pauseink_data/runtime/ffmpeg/windows-x86_64/ffmpeg.exe`
  `pauseink_data/runtime/ffmpeg/windows-x86_64/ffprobe.exe`
  `pauseink_data/runtime/ffmpeg/windows-x86_64/manifest.json`

## 8. template / guide preview の現在設計

- template preview の x 位置は fixed-width 仮定ではなく、選択 font の shaping 結果から拾う
- `VA` のような pair kerning を落とさないため、同一 scale run ごとに layout して grapheme ごとの自然な開始位置を取り出す
- `tracking` は shaping 後の grapheme 間へ追加オフセットとして足す
- slope は baseline の y オフセットだけでなく、glyph と slot box の回転にも反映する
- 一度配置した template でも、`placed_origin` を保持しておき、文字列 / font / font size / tracking / slope が変わったら slot box を再計算する
- guide の次文字縦線は `GuideOverlayState` が保持し、Ctrl タップ時は horizontal guide と縦線セット幅を固定したまま、`next_cell_origin_x` だけを直前文字の右端へ更新する
- Ctrl guide capture は `GuideCaptureState` で保持し、modifier 押下中の複数 stroke は同一 reference object に append し、modifier release でだけ guide geometry を確定する
- undo / redo shortcut を consume した frame は `guide_modifier_tap_suppressed` を立て、修飾キー release が来ても guide の次文字送りへ変換しない
- 描画中の stroke は renderer の export/preview request へ混ぜず、`AppSession::current_stroke_preview` で stabilized path を取り出して `main.rs` の `egui::Painter` overlay として描く
- pointer input は preview 描画より先に処理し、current frame の `PointerButton { pressed: true }` 座標を最初の sample として優先的に取り込む
- press frame の duplicate sample は `append_stroke_point` 側で抑止し、1 点目は dot preview として見えるようにしている
- live preview の線幅は `renderer` の `render_scale.stroke` と同じ考え方で `min(frame_rect.width/source_width, frame_rect.height/source_height)` を掛け、確定後より不自然に太く見えないようにしている
- guide の横線は geometry 上の基準傾きを保ったまま、描画時に current frame 幅いっぱいへ延長する
- template underlay は committed/live stroke より下、guide overlay より下に置く
- live stroke preview は committed overlay texture の上、guide overlay の下に置き、編集中の視認性と補助線の見やすさを両立する
- guide の手動解除は overlay だけでなく capture 文脈、modifier 状態、last committed bounds もまとめて捨てる
- 既存 object へ stroke を append する場合は `SetGlyphObjectStyleCommand` で object style も最新の active style へ同期し、renderer が object.style を参照しても見た目がずれないようにしている
- 基本スタイルの color picker は RGB のみを編集し、alpha は `active_style.opacity` を唯一の source of truth とする

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
- `cargo test -p pauseink-app --lib --bins`

代表的な smoke:

- transparent export: `cargo test -p pauseink-export transparent_png_sequence_export_smoke_if_host_runtime_exists`
- composite export: `cargo test -p pauseink-export composite_avi_export_smoke_if_host_runtime_exists`

UI 回帰メモ:

- 下部パネルは `bottom_panel_content_width` を state に持ち、`ScrollArea::both().auto_shrink([false, false])` で固定高さのまま内容だけを scroll させる
- object list や log の件数増加で bottom panel 自体の高さを揺らさない
- export 実行中は `PendingExportJob` が progress fraction と stage label を保持し、右ペインと `書き出しキュー` の両方で progress bar を描く
- ffmpeg 実行系は `-progress pipe:1 -nostats` を付け、`out_time` / `progress=end` から 0.92..0.99 の stage 内進捗へ写像する
- hardware fallback で encode 経路が切り替わっても、UI 側の pending progress は逆走しないよう max で保持する

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
