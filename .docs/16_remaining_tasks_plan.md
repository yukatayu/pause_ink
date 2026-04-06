# 未実装要素タスク計画

この文書は、2026-04-06 時点の PauseInk repository に残っている **未実装要素 / 未接続要素 / productization gap** を、あとから task 番号指定だけで着手できる粒度まで落とした実行計画です。

実装時は `AGENTS.md` の優先順位を守り、この文書はその下位計画として使います。  
将来 user が「`V1-03` を実装してください」のように指定した場合、本書の該当 task をそのまま実装起点にしてください。

---

## 1. この計画の使い方

- `V1-*`
  v1.0 の spec 上は残っているが、現 repository では未完了の task。
- `PKG-*`
  productization / release / cross-platform validation 上の残 task。
- `FUT-*`
  `.docs/12_future_work.md` で明示的に future work とされている task。v1.0 の mainline 完了条件には含めない。

各 task には次を必ず入れています。

- 何が未実装か
- どこが根拠か
- どの crate / file を触るか
- 先に終わっているべき依存 task
- acceptance criteria
- 必須 test / validation

共通の実行ルール:

1. 実装前に `progress.md` の即時マイルストーンを task 番号で更新する。
2. `docs/implementation_report_v1.0.0.md` に、その task に入る理由、採用設計、実行コマンド、結果を書く。
3. UI や保存仕様が変わる task では `README.md`、`manual/user_guide.md`、`manual/developer_guide.md` を同期する。
4. commit は日本語で切り、`git -c commit.gpgsign=false commit ...` を使う。
5. commit 後は `origin/prototype` へ push する。

---

## 2. 優先順位つき一覧

| ID | 優先度 | 種別 | 概要 | 依存 |
|---|---|---|---|---|
| `V1-01` | P0 | renderer / UI | reveal-head effect を renderer / preset / inspector まで接続する | なし |
| `V1-02` | P0 | renderer / domain | post-action chain を最小安全解釈で実装する | `V1-01` 推奨 |
| `V1-03` | P0 | preset / clear | clear preset を schema / UI / save/load / clear insertion に接続する | なし |
| `V1-04` | P0 | preset / resolver | combo preset を schema / UI / resolver に接続する | `V1-01`, `V1-02`, `V1-03` |
| `V1-05` | P1 | editor state | selection / multi-select の state model と hit-test を入れる | なし |
| `V1-06` | P1 | panel UI | object outline panel を tree / current / batch edit の土台まで仕上げる | `V1-05` |
| `V1-07` | P1 | commands / UI | group / ungroup / bulk style-edit を UI まで接続する | `V1-05`, `V1-06` |
| `V1-08` | P1 | commands / UI | z-order 並べ替え UI と reorder action を接続する | `V1-05`, `V1-06` |
| `V1-09` | P1 | editor UX | visibility / lock / solo / auto-follow-current を editor-only state として入れる | `V1-05`, `V1-06` |
| `PKG-01` | P0 | release | portable FFmpeg sidecar bundling / provenance / notices を release packaging に入れる | なし |
| `PKG-02` | P0 | validation | Windows / macOS / Linux の build / runtime / export 実検証を揃える | `PKG-01` 推奨 |
| `PKG-03` | P2 | maintenance | `egui` deprecation cleanup と GUI smoke 導線を整える | なし |
| `FUT-01` | P3 | future | partial clear | なし |
| `FUT-02` | P3 | future | real pressure support | なし |
| `FUT-03` | P3 | future | pseudo-pressure / auto taper | `FUT-02` 非依存 |
| `FUT-04` | P3 | future | proxy media | なし |
| `FUT-05` | P3 | future | GPU export compositor | なし |
| `FUT-06` | P3 | future | codec-pack helper tool | `PKG-01` 推奨 |
| `FUT-07` | P3 | future | effect scripting | なし |

推奨の実装順:

1. `V1-01` → `V1-02` → `V1-03` → `V1-04`
2. `V1-05` → `V1-06` → `V1-07` / `V1-08` / `V1-09`
3. `PKG-01` → `PKG-02`
4. `PKG-03`
5. `FUT-*` は v1.0 mainline 完了後

---

## 3. 現状の根拠まとめ

### 3.1 reveal-head / post-action / clear-combo preset

- spec:
  - `.docs/02_final_spec_v1.0.0.md`
  - reveal-head effect: 5.3
  - post-action: 5.4
  - clear / combo preset: 6.1
- 現状コード:
  - `crates/domain/src/annotations.rs:323`
    `RevealHeadEffect` 型はある
  - `crates/domain/src/annotations.rs:349`
    `EntranceBehavior.head_effect` はある
  - `crates/domain/src/annotations.rs:409`
    `PostAction` 型はある
  - `crates/domain/src/annotations.rs:185`
    `PresetBindings.clear` / `combo` はある
  - `crates/app/src/main.rs:3550`
    inspector は `EntranceKind` / `duration_mode` / `duration` / `speed_scalar` まで
  - `crates/renderer/src/lib.rs:287`
    renderer は `EntranceKind` と duration 系だけを消費
  - `manual/user_guide.md:202`
    reveal-head / post-action / clear-combo preset UI は未実装と明記

### 3.2 selection / outline / group / z-order

- spec:
  - `.docs/02_final_spec_v1.0.0.md:9`
    object 選択、複数選択、group/ungroup、z-order 並べ替え、一括編集
  - `.docs/02_final_spec_v1.0.0.md:10.1`
    object outline tree, visibility / lock / solo, auto-follow-current
- 現状コード:
  - `crates/domain/src/project_commands.rs:103`
    z-index command はある
  - `crates/app/src/main.rs:2921`
    bottom tab の outline は flat text list のみ
  - `crates/app/src/lib.rs:345`
    shift-group append はあるが、group / ungroup UI はない
  - `docs/implementation_report_v1.0.0.md:1039`
    selection / multi-select / group / ungroup / z-order UI はまだ最小と記録済み

### 3.3 sidecar packaging / release / validation

- spec:
  - `AGENTS.md:11`
    v1.0 mainline は portable sidecar runtime 前提
  - `.docs/07_media_runtime_and_ffmpeg.md`
    provenance / compliance と packaging を明記する前提
- 現状コード / docs:
  - `crates/media/src/lib.rs:15`
    `RuntimeOrigin`, `manifest_path`, `license_summary` はある
  - `.github/workflows/release.yml`
    app binary archive だけを GitHub Release に載せる
  - `manual/developer_guide.md:222`
    sidecar 同梱は別タスク
  - `docs/implementation_report_v1.0.0.md:1034-1037`
    sidecar bundling 未完、Windows/macOS runtime 実検証未完

---

## 4. v1.0 残 task

### `V1-01` reveal-head effect を renderer / preset / inspector まで接続する

- 優先度:
  - `P0`
- 目的:
  - `EntranceBehavior.head_effect` を dead field のまま残さず、path-based entrance に対して見える効果として実装する。
- 根拠:
  - `.docs/02_final_spec_v1.0.0.md:204-219`
  - `crates/domain/src/annotations.rs:323`
  - `crates/renderer/src/lib.rs:287`
  - `manual/user_guide.md:202`
- 触る file:
  - `crates/domain/src/annotations.rs`
  - `crates/presets_core/src/lib.rs`
  - `crates/renderer/src/lib.rs`
  - `crates/app/src/main.rs`
  - `presets/style_presets/*.json5`
  - `manual/user_guide.md`
  - `manual/developer_guide.md`
  - `docs/implementation_report_v1.0.0.md`
  - `progress.md`
- 設計方針:
  - v1.0 の安全解釈として、head effect は `PathTrace` と `Wipe` でのみ有効にする。
  - `Instant` と `Dissolve` は head を持たない。UI では選べても renderer では no-op にせず、UI 側で無効化して誤解を防ぐ。
  - color source は `stroke color`、`preset accent`、`custom` をサポートする。
  - `comet tail` は visible progress の trailing window を使って alpha 減衰させる。geometry 自体は path を変形せず、renderer overlay 上の付加効果として扱う。
  - base style の `glow` と混同しない。head effect は entrance の一時効果であり、object 完了後に残さない。
- 実装ステップ:
  1. `presets_core` に `head_effect` の file schema を追加する。
  2. `app` の `SavedEntrancePresetState` と project 保存経路に `head_effect` を含める。
  3. inspector に head kind / color source / size / blur / tail / persistence / blend mode の UI を追加する。
  4. renderer に current reveal progress 上の head position 算出 helper を追加する。
  5. `PathTrace` / `Wipe` の visible path 先端へ head effect を描く。
  6. preview と export の双方で同一挙動になるよう `RenderRequest` ベースで統一する。
- 依存:
  - なし
- acceptance criteria:
  - `PathTrace` object で、preview と export の両方に head effect が見える。
  - head effect は reveal 完了後に残らない。
  - `Instant` / `Dissolve` では head 設定 UI が disable されるか、少なくとも効果が適用されないことが明示される。
  - style preset 保存 / project 保存 / 再起動復元で head 設定が戻る。
- 必須 test:
  - renderer unit:
    - `path_trace_head_effect_tracks_reveal_front`
    - `head_effect_disappears_after_reveal_completion`
    - `dissolve_ignores_head_effect`
  - app unit:
    - `head_effect_persists_through_settings_restart`
    - `style_preset_roundtrip_keeps_head_effect`

### `V1-02` post-action chain を最小安全解釈で実装する

- 優先度:
  - `P0`
- 目的:
  - `GlyphObject.post_actions` / `Group.post_actions` を renderer が消費し、reveal 後または途中の built-in action を実際の見た目に反映する。
- 根拠:
  - `.docs/02_final_spec_v1.0.0.md:221-235`
  - `crates/domain/src/annotations.rs:409`
  - `docs/implementation_report_v1.0.0.md:1041`
- 触る file:
  - `crates/domain/src/annotations.rs`
  - `crates/presets_core/src/lib.rs`
  - `crates/renderer/src/lib.rs`
  - `crates/app/src/main.rs`
  - `crates/app/src/lib.rs`
  - `manual/user_guide.md`
  - `manual/developer_guide.md`
  - `docs/implementation_report_v1.0.0.md`
  - `progress.md`
- 設計方針:
  - v1.0 の最小安全実装は `NoOp`、`StyleChange`、`InterpolatedStyleChange`、`Pulse`、`Blink` の順に実装する。
  - geometry や stroke path を後から書き換えない。post-action は style / alpha / thickness など renderer が合成できる範囲に限定する。
  - `timing_scope` は `GlyphObject` と `Group` の 2 段階から開始し、`Run` は group/object 経由で安全に畳み込む。`DuringReveal` は `PathTrace` / `Wipe` にのみ適用する。
  - post-action evaluator は immutable project snapshot と `time` から `effective_style` を返す純関数で持つ。
- 実装ステップ:
  1. evaluator 層を renderer 内に追加し、現在時刻で有効な post-action window を計算する。
  2. `StyleChange` と `InterpolatedStyleChange` を `StyleSnapshot` 上の overlay として反映する。
  3. `Pulse` / `Blink` は opacity / glow / scale 相当の安全な見た目に落とす。
  4. inspector に post-action editor を追加する。
  5. style preset と project 保存経路に post-action を含めるか、少なくとも combo preset 経由で扱えるよう布石を入れる。
- 依存:
  - `V1-01` 推奨
- acceptance criteria:
  - reveal 完了後に style change が発生し、preview と export の結果が一致する。
  - post-action は undo/redo と project save/load で壊れない。
  - `DuringReveal` の action が `Instant` object に誤適用されない。
- 必須 test:
  - renderer unit:
    - `style_change_applies_after_glyph_reveal_completion`
    - `interpolated_style_change_blends_over_requested_duration`
    - `pulse_and_blink_do_not_mutate_base_style_snapshot`
  - app/project_io:
    - `post_actions_roundtrip_through_project_save`

### `V1-03` clear preset を schema / UI / clear insertion に接続する

- 優先度:
  - `P0`
- 目的:
  - clear event を毎回 hard-code せず、declarative preset として選んで挿入できるようにする。
- 根拠:
  - `.docs/02_final_spec_v1.0.0.md:239-265`
  - `crates/domain/src/annotations.rs:185`
  - `crates/app/src/lib.rs:510`
  - `crates/app/src/main.rs:3085`
- 触る file:
  - `crates/domain/src/lib.rs`
  - `crates/presets_core/src/lib.rs`
  - `crates/app/src/lib.rs`
  - `crates/app/src/main.rs`
  - `presets/clear_presets/*.json5` 新設
  - `manual/user_guide.md`
  - `manual/developer_guide.md`
  - `README.md`
  - `docs/implementation_report_v1.0.0.md`
  - `progress.md`
- 設計方針:
  - v1.0 locked rule に従い、target granularity は `AllParallel` 固定を維持する。
  - clear preset が変更できるのは `kind` と、必要なら `ordering` だけに絞る。
  - UI は top bar の `全消去` を split button 相当にし、直近 preset を 1 click、他 preset は dropdown で選ぶ。
  - project 側には clear event ごとの resolved value を保存し、preset file の live 値には依存しない。
- 実装ステップ:
  1. `presets_core` に clear preset schema と catalog loader を追加する。
  2. built-in clear preset を少数用意する。`instant`, `wipe_out`, `dissolve_out`, `ordered`, `reverse_ordered`。
  3. `AppSession::insert_clear_event` が `ClearPreset` を受け取れるようにする。
  4. UI に clear preset 選択と current preset 表示を追加する。
  5. save/load と restart restore を接続する。
- 依存:
  - なし
- acceptance criteria:
  - user が UI で clear preset を選んで挿入できる。
  - clear event track と export に preset の見た目が反映される。
  - project reopen 後も、既存 clear event の resolved 値は保たれる。
- 必須 test:
  - presets_core:
    - `repository_clear_presets_load_from_json5_files`
  - app:
    - `insert_clear_uses_selected_clear_preset`
    - `clear_preset_selection_persists_across_restart`
  - renderer:
    - 既存 clear tests に preset 経由ケースを追加

### `V1-04` combo preset を schema / UI / resolver に接続する

- 優先度:
  - `P0`
- 目的:
  - style + entrance + clear をまとめて再利用できる combo preset を入れ、宣言的 preset 設計を v1.0 仕様まで閉じる。
- 根拠:
  - `.docs/02_final_spec_v1.0.0.md:239-265`
  - `crates/domain/src/annotations.rs:185`
  - `manual/user_guide.md:202`
- 触る file:
  - `crates/presets_core/src/lib.rs`
  - `crates/app/src/main.rs`
  - `crates/app/src/lib.rs`
  - `presets/combo_presets/*.json5` 新設
  - `manual/user_guide.md`
  - `manual/developer_guide.md`
  - `README.md`
  - `docs/implementation_report_v1.0.0.md`
  - `progress.md`
- 設計方針:
  - combo preset 自体は snapshot を持たず、`style preset id`、`entrance preset id`、`clear preset id` の参照束ねと表示名だけを持つ。
  - apply 時は各 preset を順に解決し、現在の active style / entrance / clear selection に適用する。
  - project 保存は今までどおり resolved snapshot 優先とし、combo preset id は任意メタデータに留める。
- 実装ステップ:
  1. combo preset schema と catalog loader を追加する。
  2. UI に combo preset dropdown と apply button を追加する。
  3. apply 時の競合解決順を `combo -> style / entrance / clear resolved snapshot` に固定する。
  4. user preset については v1.0 では read-only でもよいが、最低でも built-in combo を扱えるようにする。
- 依存:
  - `V1-01`, `V1-02`, `V1-03`
- acceptance criteria:
  - combo preset 1 回の適用で style / entrance / clear selection が切り替わる。
  - project の見た目再現は resolved snapshot に依存し、preset file 編集で壊れない。
  - developer が新しい combo preset を declarative file だけで追加できる。
- 必須 test:
  - presets_core:
    - `repository_combo_presets_resolve_style_entrance_and_clear_refs`
  - app:
    - `combo_preset_application_updates_active_style_entrance_and_clear_selection`

### `V1-05` selection / multi-select の state model と hit-test を入れる

- 優先度:
  - `P1`
- 目的:
  - object 単位の選択、複数選択、現在の selection 表示を入れ、後続の outline/group/z-order task の基礎にする。
- 根拠:
  - `.docs/02_final_spec_v1.0.0.md:306-314`
  - `.docs/03_ui_window_model.md:102-109`
  - `docs/implementation_report_v1.0.0.md:1039`
- 触る file:
  - `crates/app/src/lib.rs`
  - `crates/app/src/main.rs`
  - `crates/renderer/src/lib.rs` 必要なら selection overlay helper
  - `manual/user_guide.md`
  - `manual/developer_guide.md`
  - `docs/implementation_report_v1.0.0.md`
  - `progress.md`
- 設計方針:
  - selection state は editor-only。`HashSet<GlyphObjectId>` と `last_primary_selection` を app 側で持つ。
  - canvas hit-test は derived path の expanded bounds → 必要なら path distance の 2 段階で行う。
  - `Ctrl/Cmd` で toggle selection、plain click で単一 selection にする。
  - 既定では stroke capture と selection を同時に有効にしない。mode conflict を避けるため、selection は object がある地点の click 優先、drag drawing は空白か template/guide 文脈時のみ開始する。
- 実装ステップ:
  1. selection state と helper を app session に追加する。
  2. canvas click hit-test を追加する。
  3. selected object の簡易枠表示を overlay に追加する。
  4. inspector に「選択中 object 数」を出す。
- 依存:
  - なし
- acceptance criteria:
  - 単一 object を click で選択できる。
  - modifier 付き click で複数選択できる。
  - selection 変更が drawing の press 起点を誤って潰さない。
- 必須 test:
  - app:
    - `canvas_click_selects_single_object`
    - `modifier_click_toggles_multi_selection`
    - `drawing_on_empty_canvas_still_starts_stroke`

### `V1-06` object outline panel を tree / current / batch edit の土台まで仕上げる

- 優先度:
  - `P1`
- 目的:
  - flat list の object tab を、spec どおり run / group / glyph object / stroke の tree に引き上げる。
- 根拠:
  - `.docs/02_final_spec_v1.0.0.md:318-326`
  - `crates/app/src/main.rs:2921`
- 触る file:
  - `crates/app/src/main.rs`
  - `crates/app/src/lib.rs`
  - `manual/user_guide.md`
  - `manual/developer_guide.md`
  - `docs/implementation_report_v1.0.0.md`
  - `progress.md`
- 設計方針:
  - run は page 内の reveal lane 単位ではなく、v1.0 では「page 内の created_at batch」表示でよい。
  - group が無い object は ungrouped セクションに置く。
  - tree state は editor-only で、展開/折りたたみ状態だけ project UI state に入れてよい。
  - panel 自体は編集の single source ではなく、selection の別操作面とする。
- 実装ステップ:
  1. outline tree data builder を追加する。
  2. `egui::CollapsingHeader` などで tree を表示する。
  3. row click で selection を同期する。
  4. 現在生存中 object を強調表示する。
  5. row に `page / z / stroke count / created_at` を出す。
- 依存:
  - `V1-05`
- acceptance criteria:
  - panel が run / group / object / stroke の階層で見える。
  - row click と canvas selection が同期する。
  - 現在時刻で visible な object が分かる。
- 必須 test:
  - app:
    - `outline_tree_groups_objects_by_page_and_group`
    - `outline_selection_syncs_with_canvas_selection`

### `V1-07` group / ungroup と bulk style-edit を UI まで接続する

- 優先度:
  - `P1`
- 目的:
  - spec にある group / ungroup / 一括編集を、multi-select 後の操作として成立させる。
- 根拠:
  - `.docs/02_final_spec_v1.0.0.md:306-314`
  - `crates/domain/src/project_commands.rs:63`
  - `crates/app/src/lib.rs:665`
- 触る file:
  - `crates/domain/src/project_commands.rs`
  - `crates/app/src/lib.rs`
  - `crates/app/src/main.rs`
  - `manual/user_guide.md`
  - `manual/developer_guide.md`
  - `docs/implementation_report_v1.0.0.md`
  - `progress.md`
- 設計方針:
  - group command は selected object IDs を `Group` にまとめる。
  - ungroup は selected groups を解く。
  - bulk style edit は selected object へ同一 `StyleSnapshot` / `EntranceBehavior` を一括適用する。
  - project save は既存 schema の `groups` を使う。editor-only wrapper を増やさない。
- 実装ステップ:
  1. `CreateGroupCommand` / `DeleteGroupCommand` 相当を必要なら追加する。
  2. UI に `グループ化` / `グループ解除` ボタンを置く。
  3. selection 中は inspector の style 変更を selected set へ反映できるようにする。
  4. undo/redo を grouped command で扱う。
- 依存:
  - `V1-05`, `V1-06`
- acceptance criteria:
  - 複数選択 object を group 化できる。
  - ungroup で元の object 群へ戻る。
  - inspector の色 / 太さ / effect / entrance が selected set へ一括反映される。
- 必須 test:
  - domain/app:
    - `group_command_roundtrips_selected_objects`
    - `ungroup_restores_original_members`
    - `bulk_style_edit_updates_all_selected_objects`

### `V1-08` z-order 並べ替え UI と reorder 操作を接続する

- 優先度:
  - `P1`
- 目的:
  - domain にある `z_index` command を UI から使えるようにし、capture/reveal order と分離した編集を成立させる。
- 根拠:
  - `.docs/02_final_spec_v1.0.0.md:313-314`
  - `.docs/04_architecture.md:126-133`
  - `crates/domain/src/project_commands.rs:103`
- 触る file:
  - `crates/app/src/lib.rs`
  - `crates/app/src/main.rs`
  - `crates/domain/src/project_commands.rs`
  - `manual/user_guide.md`
  - `manual/developer_guide.md`
  - `docs/implementation_report_v1.0.0.md`
  - `progress.md`
- 設計方針:
  - v1.0 では `最前面へ`、`前へ`、`後ろへ`、`最背面へ` の 4 操作で十分。
  - reorder は selected set 全体に対して stable に適用する。
  - renderer の draw order は引き続き `z_index` 優先、同値時に created/capture order fallback とする。
- 実装ステップ:
  1. selected set の reorder helper を追加する。
  2. outline panel または inspector に z-order buttons を追加する。
  3. redraw / export が新 z-order を反映することを確認する。
- 依存:
  - `V1-05`, `V1-06`
- acceptance criteria:
  - selected object を最前面 / 最背面へ送れる。
  - capture/reveal order は変わらず、見た目だけが変わる。
  - save/load 後も z-order が保たれる。
- 必須 test:
  - domain/app:
    - `z_order_buttons_update_only_z_index`
    - `save_and_reload_preserves_reordered_z_index`

### `V1-09` visibility / lock / solo / auto-follow-current を editor-only state として入れる

- 優先度:
  - `P1`
- 目的:
  - object outline panel の実用性を上げる。
- 根拠:
  - `.docs/02_final_spec_v1.0.0.md:324-326`
  - `crates/app/src/main.rs:2921`
- 触る file:
  - `crates/app/src/main.rs`
  - `crates/app/src/lib.rs`
  - `manual/user_guide.md`
  - `manual/developer_guide.md`
  - `docs/implementation_report_v1.0.0.md`
  - `progress.md`
- 設計方針:
  - `visibility` / `lock` / `solo` は v1.0 では editor-only workspace state として扱う。
  - project export の truth を変える設定ではなく、preview/selection/filter にだけ効かせる。
  - auto-follow-current は current time で visible な run/object を panel scroll で追うだけに留める。
- 実装ステップ:
  1. editor-only object flags を app state に追加する。
  2. outline row に icon toggle を追加する。
  3. canvas hit-test と renderer preview に hidden/locked/solo を反映する。
  4. auto-follow-current toggle を bottom tab toolbar に追加する。
- 依存:
  - `V1-05`, `V1-06`
- acceptance criteria:
  - hidden object は preview で見えず、export は既定では影響を受けない。
  - locked object は selection / edit 対象から除外される。
  - solo は対象だけを preview 上で残す。
  - auto-follow-current 有効時に current object が panel 内で追従する。
- 必須 test:
  - app:
    - `hidden_objects_are_filtered_only_in_editor_preview`
    - `locked_objects_cannot_be_selected`
    - `solo_mode_filters_outline_and_canvas_consistently`

---

## 5. packaging / validation task

### `PKG-01` portable FFmpeg sidecar bundling / provenance / notices を release packaging に入れる

- 優先度:
  - `P0`
- 目的:
  - mainline spec の sidecar runtime 方針を、GitHub Release artifact まで閉じる。
- 根拠:
  - `AGENTS.md:11`
  - `.docs/07_media_runtime_and_ffmpeg.md`
  - `manual/developer_guide.md:222`
  - `docs/implementation_report_v1.0.0.md:1034-1035`
- 触る file:
  - `.github/workflows/release.yml`
  - `scripts/package_release_asset.py`
  - `crates/media/src/lib.rs`
  - `manual/developer_guide.md`
  - `README.md`
  - `docs/implementation_report_v1.0.0.md`
  - `progress.md`
  - `pauseink_data/runtime/ffmpeg/<platform>/manifest.json` のサンプル設計文書
- 設計方針:
  - app binary archive と sidecar runtime archive を混ぜない。まずは同一 release の別 asset として配布し、`manifest.json` / notices / provenance を同梱する。
  - GPL 系 codec を含む host runtime を mainline 同梱前提にしない。
  - runtime の source URL、license summary、build summary、sha256 を manifest に含める。
  - release workflow は platform ごとの runtime asset が揃った時だけ sidecar-inclusive release を publish する。
- 実装ステップ:
  1. release asset layout を決める。
  2. sidecar manifest schema と notice file の最小要件を決める。
  3. packager script に sidecar 同梱 / 別 asset 生成のモードを追加する。
  4. release workflow に platform runtime ingest を追加する。
  5. runtime diagnostics に manifest/provenance summary 表示を足す。
- 依存:
  - なし
- acceptance criteria:
  - GitHub Release に app asset と sidecar runtime asset が載る。
  - sidecar asset には `ffmpeg` / `ffprobe` / `manifest.json` / license notice が入る。
  - app は sidecar 配置後に `RuntimeOrigin::Sidecar` として認識する。
- 必須 test / validation:
  - media unit:
    - `sidecar_runtime_manifest_exposes_provenance_fields`
  - CI/local:
    - packager で sidecar archive を作り、中身を list して検証する

### `PKG-02` Windows / macOS / Linux の build / runtime / export 実検証を揃える

- 優先度:
  - `P0`
- 目的:
  - 現在 Linux host 依存の検証を、3 OS 実証へ広げる。
- 根拠:
  - `docs/implementation_report_v1.0.0.md:1036-1037`
  - `manual/user_guide.md:204`
- 触る file:
  - `docs/implementation_report_v1.0.0.md`
  - `manual/user_guide.md`
  - `manual/developer_guide.md`
  - `README.md`
  - 必要なら `.github/workflows/release.yml`
- 設計方針:
  - この task は主に verification task。必要なら最小の script 補助を足すが、先に docs を精密化する。
  - OS ごとに最低限確認する項目を固定する。
    - build
    - launch
    - import
    - free ink
    - save/load
    - composite export
    - transparent export
    - sidecar discovery
- 実装ステップ:
  1. validation matrix を文書化する。
  2. Windows 実機または runner で build と runtime discovery を確認する。
  3. macOS 実機または runner で build と runtime discovery を確認する。
  4. 各結果を implementation report に日時つきで記録する。
- 依存:
  - `PKG-01` 推奨
- acceptance criteria:
  - Windows / macOS / Linux の build 結果が report に揃う。
  - 3 OS のうち少なくとも 2 OS で import/export 実検証があり、残りは blocker が具体的に記録されている。
  - runtime discovery の配置案内が各 OS 実態と食い違っていない。
- 必須 validation:
  - 実機または runner での `cargo build --release -p pauseink-app`
  - sidecar discovery 実行
  - export 1 本ずつ

### `PKG-03` `egui` deprecation cleanup と GUI smoke 導線を整える

- 優先度:
  - `P2`
- 目的:
  - 現状の build warning と headless GUI 未検証 gap を減らす。
- 根拠:
  - `docs/implementation_report_v1.0.0.md:1046`
- 触る file:
  - `crates/app/src/main.rs`
  - `.github/workflows/ci.yml`
  - `docs/implementation_report_v1.0.0.md`
  - `progress.md`
- 設計方針:
  - `Panel::*` の deprecation は動作維持を優先しつつ新 API へ置き換える。
  - Linux CI のみでも `xvfb-run` か headless harness を使い、最低限の起動 smoke を入れる。
- 依存:
  - なし
- acceptance criteria:
  - `cargo check -p pauseink-app --all-targets` の deprecation warning が実質ゼロか、大幅減になる。
  - Linux CI で GUI 起動 smoke が 1 本追加される。

---

## 6. future work task

### `FUT-01` partial clear

- 根拠:
  - `AGENTS.md`
  - `.docs/12_future_work.md:1`
- 方針:
  - v1.0 では着手しない。page carry-forward → group 単位 clear → arbitrary partial clear の順。

### `FUT-02` real pressure support

- 根拠:
  - `AGENTS.md:7`
  - `.docs/12_future_work.md:2`
- 方針:
  - input abstraction と stroke sample schema の pressure hook を使う。

### `FUT-03` pseudo-pressure / auto taper

- 根拠:
  - `.docs/06_stroke_processing_and_future_taper.md`
  - `.docs/12_future_work.md:3`
- 方針:
  - speed / curvature / path progress を signal にする。

### `FUT-04` proxy media

- 根拠:
  - `.docs/12_future_work.md:4`
- 方針:
  - media provider を差し替えても project schema を変えない。

### `FUT-05` GPU export compositor

- 根拠:
  - `.docs/12_future_work.md:5`
- 方針:
  - CPU-safe baseline を壊さない optional path。

### `FUT-06` codec-pack helper tool

- 根拠:
  - `.docs/12_future_work.md:6`
- 方針:
  - runtime sidecar と別 manifest / notice / provenance を持つ optional 配布。

### `FUT-07` effect scripting

- 根拠:
  - `.docs/12_future_work.md:7`
- 方針:
  - hot path に直接スクリプトを入れない。安全な expression layer を別設計で検討。

---

## 7. task 実行時の共通完了条件

どの task でも、完了宣言の前に少なくとも次を満たすこと。

1. task 専用の red/green test を追加して通す。
2. `cargo test --workspace` を再通過させる。
3. `cargo check -p pauseink-app --all-targets` を再通過させる。
4. UI / 保存仕様 / export 挙動が変わる task では manual と report を同期する。
5. `progress.md` に task 番号、概算率、残 blocker を書く。

---

## 8. 現時点の推奨着手点

最初に着手するなら次の順が安全です。

1. `V1-01`
   entrance 系の dead field を閉じる。後続 preset / combo 設計にも影響するため最優先。
2. `V1-03`
   clear preset を先に入れると `V1-04` の combo preset が組みやすい。
3. `V1-05`
   panel 編集系を進めるための基礎。
4. `PKG-01`
   v1.0 mainline packaging の gap を閉じる。
