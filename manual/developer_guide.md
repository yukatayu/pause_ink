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
  export family/profile catalog、style / entrance / clear / combo preset loader
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
- `project.media.source_path` は raw string をそのまま保持し、project open 時だけ runtime 用に再解決する。relative path は `.pauseink` の親ディレクトリ基準、absolute path はそのまま使う
- open 時の media restore は `dirty` を立てず、保存済み `source_path` 文字列も上書きしない

主要テスト:

- `cargo test -p pauseink-project-io`
- `cargo test -p pauseink-app`

## 4. portable state

既定 layout:

```text
pauseink_data/
  config/
    style_presets/
    entrance_presets/
    clear_presets/
    combo_presets/
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

## 6. preset catalog

現実装では catalog を 4 系統に分けています。

- style preset
- entrance preset
- clear preset
- combo preset

built-in と user の配置先:

- built-in style: `presets/style_presets/*.json5`
- built-in entrance: `presets/entrance_presets/*.json5` または legacy style file 内の `entrance`
- built-in clear: `presets/clear_presets/*.json5`
- built-in combo: `presets/combo_presets/*.json5`
- user style: `pauseink_data/config/style_presets/*.json5`
- user entrance: `pauseink_data/config/entrance_presets/*.json5`
- user clear: `pauseink_data/config/clear_presets/*.json5`
- user combo: `pauseink_data/config/combo_presets/*.json5`

読み込み順は built-in -> user overlay で、同じ `id` がある場合は user preset が優先されます。style preset に legacy で `entrance` が同居していても、loader 側で entrance preset candidate として救済します。normalized save では style と entrance を別 file に書きます。built-in は読み取り専用、user preset は GUI から style / entrance の追加保存 / 上書き保存 / 削除ができます。

project には mutable preset file そのものではなく、`project.presets.base_style` に resolved base style snapshot、`project.presets.entrance` に resolved entrance snapshot と任意の preset ID を保存します。field-level の継承 / 上書き state は `project.settings.pauseink_editor_ui` と `settings.json5` 側へ保存します。template / guide の project-specific UI state も同じ editor UI state 側です。

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
- entrance.scope
- entrance.order
- entrance.duration_mode
- entrance.duration_ms
- entrance.speed_scalar

renderer 側には outline / drop shadow / glow の描画処理があり、現在は object-first ではなく layer-first の multi-pass compositor にしてあります。これにより、後から描いた object の outer effect も含めて、先にある body を不自然に覆いにくくしています。出現時間は `fixed_total_duration` と `proportional_to_stroke_length` の 2 モードを持ち、後者は 600px を基準長として `speed_scalar` を掛けています。
entrance sequencing は page 全体 1 本の queue ではなく、同じ `created_at` を持つ paused batch lane ごとに計算します。`Instant` object は lane の先頭時刻から即表示され、timed entrance だけが同じ lane 内の前の timed object 完了を待ちます。preview では current paused batch だけ `fully visible` override を掛け、`再生` / `保存` / `書き出し` では lane 本来の reveal へ戻します。
未接続の残項目は reveal-head effect、post-action chain、clear / combo preset の専用 UI です。

## 6.5 selection / group / z-order foundation

- selection の source of truth は `pauseink-app` の `AppSession.selection` です
- `SelectionState` は `selected_object_ids`、`selected_group_ids`、focus ID を持ち、project file へは保存しません
- inspector の style / entrance 編集は project を直接 mutate せず、`BatchSetGlyphObjectStyleCommand` と `BatchSetGlyphObjectEntranceCommand` を通します
- outline 起点で選択し、通常クリックで単一選択、`Ctrl` / `Cmd` 併用クリックで toggle multi-select を行います
- group は flat です。v1.0 では入れ子を許さず、1 object が複数 group へ属する workflow も想定しません
- group 化は `InsertGroupCommand`、group 解除は `RemoveGroupCommand` を履歴へ積みます
- `UpdateGroupMembershipCommand` は member 更新を history-safe に扱うための土台で、今後の outline 強化でも使い回せます
- z-order は renderer と同じく `(z_index, capture_order, id)` を基準に解釈し、UI 操作時は dense な `z_index` に正規化します
- 前後移動は選択 object の相対順を保ったまま `NormalizeZOrderCommand` を積む形です
- undo / redo 後は `repair_selection_after_project_change()` で selection を prune し、直前の group/ungroup 文脈が残っていれば group 選択または member object 選択へ復元します
- 現 UI は outline panel 起点の最小導線です。canvas 直接選択、複雑な tree 編集、group style editor は `V1-06` 以降の範囲です

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
- guide の次文字縦線は `GuideOverlayState` が保持し、Ctrl タップ時は horizontal guide と縦線セット幅を固定したまま、前回 guide 確定/送り以降に commit された stroke object 群の union bounds の `max.x` へ `next_cell_origin_x` を更新する
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

- selection の source of truth は `AppSession::selection`。`selected_object_ids` / `selected_group_ids` / `focused_*` を持つが、renderer へは持ち込まない
- outline panel は `AppSession` を直接 mutate して object / group selection を切り替える。現段階では canvas 直接選択は scope 外
- group は flat data のまま扱い、v1.0 では入れ子を禁止する。group 化は未所属 object だけを束ねる
- group/ungroup 後の undo/redo では `last_group_selection_context` で selection を修復し、group が存在する状態では group selection、消えた状態では member object selection へ戻す
- inspector の style / entrance 適用は direct mutation をやめ、`BatchSetGlyphObjectStyleCommand` / `BatchSetGlyphObjectEntranceCommand` を通して history へ積む
- z-order の再配置は `NormalizeZOrderCommand` へ `GlyphObjectZIndexChange` の列を渡し、renderer 想定の `(z_index, capture_order, id)` 順を保ったまま dense な `z_index` を振り直す
- 左右ペインは `show_side_panel_scroll_body()` で `固定ヘッダ + ScrollArea::vertical()` に分け、outer panel の ID と `resizable(true)` は維持して幅変更を壊さない
- left は `draw_left_panel_scroll_body()`、right は `draw_right_panel_scroll_body()` へ本文を逃がし、低い画面でも export や font controls へ到達できるようにしている
- template advanced controls は `draw_template_details_window()` の別 window に置き、`apply_template_settings_change()` で placed slot / preview を即時更新する
- `guide_next_gap_ratio` は `Settings` と `ProjectEditorUiState` の両方へ保存し、guide slope と同じ reopen / relaunch 経路で復元する
- guide の次文字位置は `GuideOverlayState::next_cell_anchor_x` を基準に持ち、表示位置は `guide_next_cell_origin_x(anchor, cell_width, gap_ratio)` で解決する。縦線セット幅は変えず、bounds 無し advance だけ `guide_fallback_advance_step()` で正方向へ clamp する
- 下部パネルは `bottom_panel_content_width` を state に持ち、`ScrollArea::both().auto_shrink([false, false])` で固定高さのまま内容だけを scroll させる
- object list や log の件数増加で bottom panel 自体の高さを揺らさない
- export 実行中は `PendingExportJob` が progress fraction と stage label を保持し、右ペインと `書き出しキュー` の両方で progress bar を描く
- export UI は `export_progress_hint` で stage label から説明文を出し、frame 生成 / encode / cleanup の待ち内容を日本語で補足する
- ffmpeg 実行系は `-progress pipe:1 -stats_period 0.25 -nostats` を付け、`out_time` で 0.92..0.99 の stage 内進捗を更新し、`progress=end` は `最終処理中` ラベルへ写像する
- hardware fallback で encode 経路が切り替わっても、UI 側の pending progress は逆走しないよう max で保持する
- export worker は ffmpeg 完了後に `3/3 一時ファイルを整理中` を送ってから working directory cleanup を行い、cleanup 待ちが「止まった」ように見えないようにする

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
