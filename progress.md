# PauseInk 進捗

このファイルは短く、最新で、事実ベースに保つ。

## 現在地

- 作業ブランチ: `develop`
- 目標バージョン: `v1.0.0`
- 全体状態: `AGENTS.md` と `.docs/10_testing_and_done_criteria.md` に対して概算 99%。単一ウィンドウ GUI、`.pauseink` save/load、autosave/recovery、preferences/cache manager/runtime diagnostics、Google Fonts cache と graceful failure、export queue/engine、transparent/composite export、README/manual/tutorial/report/progress の同期、preview 座標ずれと UI 日本語文字化けの修正、template underlay / guide 操作性 / transport discoverability / shortcut / panel resize、描画中ストロークのライブプレビュー、前スロット追加、object style 同期、guide 解除の stale state 解消、multi-stroke effect の backend 合成順補正、FFmpeg runtime の手動再検出と Windows/macOS/Linux の system path 探索強化、project ごとの style/entrance/template/guide 状態保存、portable user preset CRUD、effect editor、出現速度 editor、paused batch preview semantics、cross-object effect order、起動時ワークスペース復元、再生中入力禁止、左右ペインの固定ヘッダ付き縦スクロール、template 詳細 popup、guide 次文字字間調整、outline 起点の複数選択 / group / ungroup / z-order foundation、`Esc` による popup 優先 close と template/guide cancel まで反映済み。
- 完了判定: host build/test/save-load/export、portable-state rule、Google Fonts graceful failure、Windows build 試行記録、final QA/docs review 相当の主要項目は通過済み。ただし `.docs/11_implementation_plan.md` ベースでは reveal-head effect、post-action chain、clear/combo preset の専用 UI が残っているため 100% から巻き戻して管理する。
- 現在の即時マイルストーン: `V1-13 Esc cancel for transient modes` を完了。次候補は `V1-14 metrics-based template alignment`。
- 最新の確認事項:
  - `AGENTS.md` と `.docs/` を全件読了
  - `README.md`、`progress.md`、`manual/`、`presets/`、`samples/`、`docs/implementation_report_v1.0.0.md` を確認
  - `develop` ブランチで作業継続
  - `V1-05` の前提として、selection の source of truth を `AppSession` に一本化し、group 入れ子禁止・outline 起点・selected objects の相対順を保つ z-order 移動で進める方針を再確認した
  - `V1-05` では domain command と app selection state の両方を TDD で進め、複数選択・group/ungroup・batch style/entrance・z-order を同一履歴モデルへ載せる方針を確定した
  - `V1-05` を完了し、`SelectionState`、`RemoveGroupCommand`、`BatchSetGlyphObjectStyleCommand`、`BatchSetGlyphObjectEntranceCommand`、`NormalizeZOrderCommand` を追加した
  - 下部 `オブジェクト一覧` から object / group の複数選択、group / ungroup、`背面へ` / `一つ後ろ` / `一つ前` / `前面へ` を実行できるようにした
  - style / entrance inspector は直接 mutate をやめ、選択対象全体へ history 経由で適用されるようにした
  - `cargo test -p pauseink-domain`、`cargo test -p pauseink-app --lib --bins`、`cargo check -p pauseink-app --all-targets`、`cargo test --workspace` を再通過した
  - `V1-01` に着手し、現行 `style preset` へ entrance が混在している loader / save / restore / CRUD / UI を、後続の clear/combo 実装で手戻りしない形へ分離する作業を開始した
  - `V1-01` では built-in/user preset の読み込み互換を保ちつつ、style / entrance / clear / combo の category と portable directory を分離する方針で進める
  - `V1-01` の binding metadata は project/settings の editor UI state 側へ保持し、resolved snapshot は project `presets` 側へ残す方針で実装を始める
  - `V1-01` を完了し、style / entrance / clear / combo の preset catalog を `presets_core` へ分離、portable root に `entrance_presets / clear_presets / combo_presets` を追加した
  - built-in / user の legacy style preset file に埋まっていた `entrance` は lenient load で entrance preset candidate として救済し、style preset の normalized save は entrance を含めない形へ寄せた
  - app 側では style preset と entrance preset を別 picker / 別 CRUD / 別 binding state に分離し、field-level の `preset 継承中 / 上書き中 / presetへ戻す` を最小 UI で接続した
  - `settings.json5` と `project.settings.pauseink_editor_ui` へ style/entrance の binding state を保存し、`project.presets.base_style` と `project.presets.entrance` の resolved snapshot / preset ID と組み合わせて reopen / relaunch 復元できるようにした
  - `cargo test -p pauseink-presets-core -p pauseink-portable-fs -p pauseink-app --lib --bins` を再通過し、style / entrance user preset CRUD、binding state 復元、legacy entrance rescue を回帰固定した
  - `V1-13` を完了し、`Esc` は `復旧 -> テンプレート詳細 -> 設定 -> キャッシュ管理 -> ランタイム診断` の順で window を閉じ、window が無ければ template preview / guide overlay を解除するようにした
  - text edit focus 中は global `Esc` cancel を奪わないことを test で固定し、`cargo test -p pauseink-app --bin pauseink-app escape_ -- --nocapture` を green 化した
  - 仕上げ確認として `cargo fmt --all`、`cargo test --workspace`、`cargo check -p pauseink-app --all-targets`、`git diff --check` を再通過した
  - `.docs/16_remaining_tasks_plan.md` の `V1-07` を `template / guide advanced controls` へ絞り直し、slot fit を計画から外した
  - guide 字間は `cell_width` 比、負値許可、guide slope と同じ保存経路で固定した
  - template 詳細は左ペインへ詰め込まず別ポップアップ前提にし、変更がリアルタイムで preview / placed slot へ反映される設計へ更新した
  - 各残 task に `ひとことで言うと` を追加し、利用者目線での意味が追いやすい計画書へ整えた
  - `PKG-02` は「未着手」ではなく「release workflow 自体は実装済みで、sidecar/notices 同梱が未完了」という状態に計画書を修正した
  - `V1-05` は app session 一本化 / group 入れ子禁止 / outline 起点 / z-order 前後移動を前提として固定し、早めに着手しても大きな手戻りが出にくい形にした
  - `V1-08` として左右ペインの縦スクロール対応を追加し、先に片づける軽量 task として計画へ入れた
  - `.pauseink` lenient load / canonical save / unknown field 保持を実装
  - bounded undo/redo と history depth 設定を実装
  - portable settings、cache/autosave/runtime path、cache cleanup helper を実装
  - local font discovery、Google Fonts CSS/cache helper、実 fetch と graceful failure を実装
  - template slot / guide geometry を実装
  - FFmpeg runtime discovery / probe / capability parser / preview frame を実装
  - export family/profile catalog と base style preset loader を実装
  - export engine、custom profile 編集、HW try / software fallback、transparent/composite smoke export を実装
  - `app` binary に preferences / cache manager / runtime diagnostics / export queue を接続
  - built-in base style preset の読み込みと適用 UI を接続
  - `.docs/` 配下を日本語へ統一し、`.docs/05_project_file_format.md` を現行 schema に同期
  - `samples/minimal_project.pauseink` を現行 `.pauseink` schema へ更新
  - integration / smoke として `create -> save -> reopen -> compare`、`import -> annotate -> clear -> save` を追加し通過
  - tutorial validation を一時 preset/profile 追加で実施し、loader / app compile を通過
  - `.github/workflows/ci.yml` を追加し、`main` push と `pull_request` で `cargo check` / `cargo test --workspace` を走らせる構成を追加
  - `.github/workflows/release.yml` を追加し、tag push または tag 付き commit の `main` 流入時に Linux / macOS / Windows build を作って GitHub Release へ添付する構成を追加
  - `scripts/package_release_asset.py` を追加し、release asset の archive 化を workflow から再利用できる形にした
  - preview overlay を source frame 座標から target texture 座標へ縮尺して描くよう修正し、マウス描画時の見かけの大きな位置ずれを解消
  - canvas pointer helper の roundtrip / letterbox test を追加し、preview 座標変換の再発防止を強化
  - `egui` 起動時に system / portable font から日本語 UI fallback font を登録し、Windows 環境での豆腐化を回避する構成にした
  - bugfix sanity review sub-agent でも、preview の source/target 座標不一致と `egui` 日本語 font 未登録が主因であることを再確認した
  - bugfix 反映 commit `217d1ae` を `origin/prototype` へ push 済み
  - 現在の追加修正バッチでは、template underlay の字幅/字詰め/傾き/フォント選択、object list と log の drag resize、transport bar の明示、Ctrl-Z / Ctrl-Shift-Z / Ctrl-Y、Ctrl タップでの次文字縦ガイド進行、template 配置中の stroke 抑止を進めている
  - template 字詰めは単文字固定幅ではなく、実フォント shaping と kerning を使う形へ更新した
  - architecture / UI 観点の sub-agent 2 件を回収済みで、panel resize・shortcut・runtime help・template slot 幅の根本原因を確認した
  - transport bar を上部直下へ追加し、再生 / 一時停止 / seek の導線を分離した
  - template font dropdown は読み込み済み family を列挙し、選択 family を egui へ lazy 登録する形で反映した
  - Ctrl タップでの次文字縦ガイド送りと、template 配置待ち中の stroke 抑止を実装した
  - Ctrl guide capture は modifier 押下中の複数 stroke を同一 reference glyph に寄せ、modifier release で確定する挙動へ更新した
  - 描画中の stroke を `AppSession` の draft から stabilized preview として取り出し、committed overlay の上に live 表示するよう更新した
  - live preview sanity review sub-agent でも、draft は editor-only overlay として app painter で描くのが最小で安全という結論を確認した
  - `前スロット` を追加し、template slot の前後移動を underflow / overflow しない helper へ寄せた
  - 既存 object へ stroke を append する際、object style も最新の active style へ同期するよう修正し、基本スタイル変更が template / guide の継続描画で反映されるようにした
  - `ガイド解除` は overlay だけでなく capture 文脈、modifier 状態、last committed bounds もまとめて捨てるようにした
  - renderer の effect 合成を object 単位の multi-pass へ寄せ、後続 stroke の outline が先行 stroke 本体を不自然に覆いにくい順序へ補正した
  - effect 実装状況も整理し、renderer backend は整ったが、UI/preset loader は引き続き thickness / color 中心であることを明記した
  - FFmpeg runtime 再検出を `機能情報更新` / `診断を再取得` に接続し、起動後に sidecar や host runtime を配置した場合でもその場で再 discovery できるようにした
  - runtime 未検出時の最後の discovery error を診断 UI に保持し、原因が見えなくなる問題を減らした
  - Windows の `WinGet Links` / `WinGet Packages` / `WindowsApps` / Scoop、macOS の Homebrew / MacPorts、Linux の system path / user bin / Linuxbrew を system runtime 探索対象に追加した
  - 配置済み template は `placed_origin` を保持し、文字列 / フォント / フォントサイズ / 字間 / 傾きの変更時に slot box を再計算するようにした
  - template underlay は committed stroke と live stroke の下へ回し、input を preview 描画より先に処理して描き始めのラグを減らした
  - canvas input は `drag_started` 依存を避け、current frame の `PointerButton` press 座標を最初の sample として優先的に取り込むようにした
  - press frame の duplicate sample を抑止し、1 点目が zero-length line になって消えるケースを防いだ
  - guide の横線は current frame の左右端まで伸ばして描くようにし、表示領域いっぱいで基準線を見られるようにした
  - live preview の線幅は renderer と同じ downscale 比率へ合わせ、ペンを離す前だけ不自然に太く見える差を減らした
  - `project.settings.pauseink_editor_ui` と `project.presets.base_style` に、template text/font/layout、guide 傾き、resolved base style snapshot、選択 preset ID を保存するようにした
  - built-in preset に加えて `pauseink_data/config/style_presets/*.json5` の user preset overlay を読み込むようにし、GUI から追加保存 / 上書き保存 / 削除できるようにした
  - user preset は built-in と同じ `id` を使うと overlay として優先され、削除すると built-in 側へ自然に戻る
  - 下部タブは `内容幅` を持つ固定高さ scroll region に整理し、object list / logs が増えても panel 自体の縦サイズが揺れないようにした
  - export 実行中は `実行中:` の下と `書き出しキュー` の両方に stage 名付き progress bar を表示するようにした
  - 基本スタイルの色 picker は RGB のみに絞り、不透明度は単一の `不透明度` スライダーへ統一した
  - inspector に outline / drop shadow / glow / blend mode / 出現方式 / 時間モード / 時間 / 出現速度を追加し、preset / project save / renderer まで接続した
  - `project.presets.entrance` に resolved entrance snapshot を保存し、reopen 後も出現設定を復元するようにした
  - `presets/style_presets/*.json5` と `pauseink_data/config/style_presets/*.json5` は base style に加えて entrance を読み書きできるようにした
  - `cargo test -p pauseink-renderer fixed_duration_speed_scalar_changes_reveal_progress -- --nocapture`、`cargo test -p pauseink-app style_preset_application_updates_effect_fields_and_persists_entrance_state -- --nocapture`、`cargo test -p pauseink-presets-core user_style_presets_overlay_builtins_and_roundtrip_disk_edits -- --nocapture`、`cargo test -p pauseink-app save_and_reopen_project_restores_style_template_and_guide_state -- --nocapture` を通過
  - `cargo fmt --all`、`cargo test --workspace`、`cargo check -p pauseink-app --all-targets` を再通過
  - `.docs/11_implementation_plan.md` を再確認し、Phase 14 の残 gap は reveal-head effect、post-action chain、clear/combo preset 専用 UI であることを棚卸しした
  - 動画 export の 92% 固定は ffmpeg 実行中の progress 未更新が原因だったため、`-progress pipe:1` を使って encode 中も進捗が進むように修正した
  - hardware fallback で encode 経路が切り替わっても progress bar が逆走しないよう、pending progress は単調増加で保持するようにした
  - `progress=end` は即「完了」扱いにせず「最終処理中」表示へ切り替え、`3/3 一時ファイルを整理中` と説明文を出すようにして、99% / 100% 表示のまま何待ちか分からない状態を解消した
  - page 内の entrance sequencing を見直し、`Instant` は即表示のまま通しつつ、PathTrace / Wipe / Dissolve のような timed entrance は `reveal_order` 順に直列化するよう修正した
  - `cargo test -p pauseink-renderer timed_entrance_waits_for_previous_timed_reveal_even_with_instant_between -- --nocapture`、`cargo test -p pauseink-renderer timed_entrance_on_next_page_does_not_wait_for_previous_page_reveal -- --nocapture`、`cargo test -p pauseink-renderer dissolve_entrance_waits_for_previous_path_trace_reveal -- --nocapture`、`cargo test -p pauseink-renderer`、`cargo test --workspace`、`cargo check -p pauseink-app --all-targets` を再通過
  - `cargo test -p pauseink-export -- --nocapture`、`cargo test -p pauseink-app --lib --bins`、`cargo check -p pauseink-app --all-targets` を再通過
  - guide の次文字縦線は、直前文字の幅で再スケールせず、位置だけ直前文字の右端へ送るように修正した
  - `Ctrl+Z` / `Ctrl+Shift+Z` / `Ctrl+Y` を consume した release では guide の次文字送りが発火しないよう、modifier tap 抑止を追加した
  - paused batch の renderer は page 全体 1 本の queue ではなく、`created_at` ごとの paused batch lane ごとに timed entrance を直列化するように見直した
  - 一時停止中 preview は current paused batch だけ `fully visible` override を掛け、既存 batch は現在時刻ベースの reveal のまま見せるようにした
  - outline / drop shadow / glow / base の合成順は object-first から layer-first へ切り替え、後から描いた object の outer effect が先の object body を潰さないようにした
  - 再生中に canvas input が来た場合は stroke draft を開始せず、既存 drag も cancel して free-ink を無効化するようにした
  - `settings.json5` に editor UI / base style / entrance の resolved snapshot を保存し、次回起動時に style / effect / font / template / guide が戻るようにした
  - 上記 bugfix バッチについて `cargo test --workspace` と `cargo check -p pauseink-app --all-targets` を再通過し、manual / report / progress も同期した
  - `.docs/16_remaining_tasks_plan.md` を新設し、未実装の v1.0 残項目と future work を task 番号つきで整理した
  - `.docs/16_remaining_tasks_plan.md` に task ごとの具体的な困り方、不可逆寄りの先決事項、共通の doc/test 読み順を追記し、大域計画の最終見直し版へ更新した
  - `.docs/16_remaining_tasks_plan.md` をさらに更新し、`V1-09 template font switch crash fix`、`V1-10 multiline template editor UI`、`V1-11 panel-aware wide controls`、`V1-12 template slot UI removal`、`V1-13 Esc cancel for transient modes`、`V1-14 metrics-based template alignment` を追加した
  - その後 `V1-12` は再検討し、slot direct selection も採らず、`前スロット/次スロット`、current slot 状態表示、current slot 強調表示を template UI から外す task へ上書きした
  - `FUT-08 object 選択時の preview/canvas ハイライト` は future work のまま detail task 化し、`Esc` 解除は ready task として future から本体計画へ昇格した
  - template 幅は shaping ベース維持、縦揃えだけ metrics ベースへ寄せる方針を計画へ固定し、`VA` などの kerning を壊さない設計にした
  - multiline template engine 自体は既に `\\n` 対応済みだが、GUI は single-line のため `V1-10` として分離した
  - template font 切替 crash は `未 bind family` を `FontFamily::Name` で引く panic 経路が最有力と切り分け、`V1-09` の設計へ反映した
  - Windows の release binary だけ二重にコンソールが開く問題について、原因を `windows_subsystem` 未宣言と切り分け、`crates/app/src/main.rs` に release 専用 GUI subsystem 宣言を追加した
  - 上記の回帰として `windows_release_build_declares_gui_subsystem` を追加し、targeted test / `cargo test -p pauseink-app --lib --bins` / `cargo check -p pauseink-app --all-targets` を通した
  - `cargo test -p pauseink-renderer later_paused_batch_starts_in_parallel_with_first_timed_object_of_page -- --nocapture`、`cargo test -p pauseink-renderer paused_preview_forces_current_batch_fully_visible_without_releasing_previous_batch_queue -- --nocapture`、`cargo test -p pauseink-renderer later_object_outline_and_shadow_stay_behind_earlier_object_body -- --nocapture`、`cargo test -p pauseink-app save_and_relaunch_restores_style_template_and_effect_state_from_settings_file -- --nocapture`、`cargo test -p pauseink-app canvas_input_is_ignored_while_playback_is_running -- --nocapture` を red/green で通した
  - `cargo test -p pauseink-app guide_overlay_state_keeps_vertical_width_constant_and_anchors_to_previous_right_edge -- --nocapture`、`cargo test -p pauseink-app guide_overlay_state_can_advance_vertical_guides_without_moving_horizontal_origin -- --nocapture`、`cargo test -p pauseink-template-layout guide_geometry_can_move_only_the_next_character_vertical_set -- --nocapture`、`cargo test --workspace`、`cargo check -p pauseink-app --all-targets` を再通過
  - bottom panel 固定化の原因調査では sub-agent が `ScrollArea::both()` + `auto_shrink([false, false])` + 独立した内容幅 state を推奨し、その方針を採用した
  - `cargo test -p pauseink-export`、`cargo test -p pauseink-app --lib --bins`、`cargo test --workspace`、`cargo check -p pauseink-app --all-targets` を再通過
  - preset/save-state 追加後も `cargo test -p pauseink-presets-core user_style_presets_overlay_builtins_and_roundtrip_disk_edits -- --nocapture`、`cargo test -p pauseink-portable-fs -- --nocapture`、`cargo test -p pauseink-app save_and_reopen_project_restores_style_template_and_guide_state -- --nocapture`、`cargo test -p pauseink-app desktop_app_loads_user_style_presets_from_portable_root_and_overrides_builtin_ids -- --nocapture`、`cargo test -p pauseink-app user_style_preset_save_overwrite_and_delete_roundtrip_updates_catalog -- --nocapture` を通過
  - 追加の regression test として `stroke_starts_on_pointer_press_before_drag_threshold`、`committed_stroke_keeps_press_origin_as_first_raw_sample`、`same_frame_move_keeps_pointer_button_press_as_first_preview_point`、`horizontal_guide_line_extends_to_frame_edges`、`live_preview_width_matches_downscaled_overlay_scale` を追加した
  - `cargo test -p pauseink-media`、`cargo test -p pauseink-app --lib --bins`、`cargo test --workspace`、`cargo check -p pauseink-app --all-targets` を再通過
  - Windows release build の binary は `windows_subsystem = "windows"` を宣言し、debug の `cargo run` を維持したまま配布 exe だけ余計なコンソールを開かない構成へ修正した
  - 上記の副作用として、配布 exe から起動される `ffprobe` / `ffmpeg` が Windows で個別に console window を出す問題を確認し、media/export の production command を Windows 専用 no-window helper 経由へ統一した
  - startup の runtime version / capability query、import / probe、preview 再生、書き出しの ffmpeg 実行がすべて同 helper を通るようにし、source 上でも `windows_media_commands_use_hidden_process_helper` / `windows_export_commands_use_hidden_process_helper` で固定した
  - 保存済み `.pauseink` の open では `project.media.source_path` を見て media を再読込するよう修正し、relative path は `.pauseink` 自体の親ディレクトリ基準で解決するようにした
  - media restore は runtime 用の `imported_media` / `playback` だけを復元し、保存済み `source_path` 文字列は上書きせず、`dirty` も立てない
  - 今回の回帰として `restore_media_from_hint_resolves_relative_path_from_project_file` と `open_project_attempts_to_restore_saved_media_hint` を追加し、`cargo test --workspace` と `cargo check -p pauseink-app --all-targets` を再通過した
  - guide の次文字送りは、直近 1 画ではなく「前回 guide 確定/送り以降に commit された文字全体」の union bounds を基準にするよう修正した
  - 再現条件 `Ctrl を押しながら 1 文字目を書く -> Ctrl を離す -> 2 文字目を書く -> Ctrl を短く押す` を `guide_modifier_tap_advances_from_union_of_strokes_written_since_last_anchor` で固定し、多画文字でも最後の画の右端ではなく文字全体の右端へ送られることを確認した
  - 追加で、送りに使った pending bounds が 1 回で消費されること、`clear_guide_state` で stale な guide 文字 bounds も落ちることを test で固定した
  - app 側で built-in style preset / export profile の探索先を `current_exe()` の親ディレクトリ配下 `presets/` 優先、repo fallback ありに変更し、CI 配布 archive でも `style preset` と `書き出し` 欄が欠けないようにした
  - `scripts/package_release_asset.py` は `presets/` ツリーも release archive へ同梱するように更新し、`scripts/package_release_asset_test.py` で stage/zip 両方を回帰固定した
  - `V1-08` に着手し、左右ペインを `fixed header + vertical ScrollArea body` へ分けるために、既存の `bottom panel` 安定性 test と `left/right panel` 実装の境界を再確認した
  - `V1-08` では panel 幅リサイズは現状維持、ヘッダは常時表示、本文だけを縦スクロールに閉じる方針で進める
  - `V1-08` を完了し、left/right panel の outer ID と resize を保ったまま、本文だけを `show_side_panel_scroll_body()` へ逃がして低い画面でも `書き出し` / `Google Fonts 設定` まで辿れるようにした
  - side panel scroll の回帰として `side_panel_scroll_body_reports_overflow_when_contents_exceed_viewport` と `side_panel_scroll_body_uses_full_available_width` を追加し、body overflow と width 安定性を固定した
  - `V1-07` に着手し、template advanced settings を popup 化しつつ、guide 次文字字間を `cell_width` 比で保存・反映するために `TemplateSettings` / `GuideOverlayState` / `ProjectEditorUiState` / `Settings` の境界を再確認した
  - `V1-07` では state と geometry の回帰を先に固定し、その後で popup UI と slider UI を接続する方針で進める
  - `V1-07` を完了し、`テンプレート詳細` window から行間 / かな倍率 / 英字倍率 / 句読点倍率 / 下敷き表示を即時反映できるようにした
  - guide は `次文字字間` を `cell_width` 比で保存し、負値を許可したまま、縦線セット幅を変えず位置だけ動かすようにした
  - `guide_next_gap_ratio` は guide slope と同じ reopen / relaunch 経路へ保存され、project / settings の両方で復元される
  - `V1-09` を完了し、template font dropdown / restore / frame-start の 3 経路に安全化を入れて、未発見 family を持つ project/settings があっても preview / reflow で panic せず `システム既定` へ fallback するようにした
  - template 表示中に無効な font family へ切り替えようとしても現在の family を維持し、log に理由を残すようにした
  - `cargo test -p pauseink-app missing_template_font_ -- --nocapture`、`cargo test -p pauseink-app invalid_template_font_selection_keeps_previous_family -- --nocapture`、`cargo check -p pauseink-app --all-targets` を通過
  - `V1-10` を完了し、template text input を multiline 化して `Enter` 改行を有効化し、右下ドラッグで高さを変えられる editor へ置き換えた
  - template text editor の高さは `settings.json5` の app-only `editor_ui_state` にだけ保存し、`.pauseink` reopen では漏れず、workspace relaunch では復元されるようにした
  - `cargo test -p pauseink-app --bin pauseink-app save_ -- --nocapture`、`cargo test -p pauseink-app --bin pauseink-app restore_app_ui_state_accepts_legacy_editor_ui_payload_without_height -- --nocapture`、`cargo check -p pauseink-app --all-targets` を通過
  - `V1-11` を完了し、seek bar と preset ID/名 text field に `inline_wide_control_width()` を適用して、panel 幅の増減に合わせて usable width が自然に伸びるようにした
  - template text editor も同じ helper へ寄せ、広い panel では入力領域が広がり、ボタン列や短い numeric field は fixed のままにした
  - `cargo fmt --all`、`cargo test -p pauseink-app --bin pauseink-app inline_wide_control_ -- --nocapture`、`cargo test -p pauseink-app --bin pauseink-app side_panel_scroll_body_ -- --nocapture`、`cargo check -p pauseink-app --all-targets` を通過
  - `V1-12` を完了し、template UI から `前スロット/次スロット`、current slot 強調、`スロット x/y` 状態表示を削除した
  - template の内部 `current_slot_index` は commit 対象決定のためだけに残し、guide/grid 側の仕様や geometry には影響を与えないようにした
  - `cargo fmt --all`、`cargo test -p pauseink-app --bin pauseink-app template_ -- --nocapture`、`cargo check -p pauseink-app --all-targets` を通過
  - 今回の確認として `cargo test -p pauseink-media windows_media_commands_use_hidden_process_helper -- --nocapture`、`cargo test -p pauseink-export windows_export_commands_use_hidden_process_helper -- --nocapture`、`cargo test --workspace`、`cargo check -p pauseink-app --all-targets`、`python3 -m unittest scripts/package_release_asset_test.py`、`rg -n "Command::new\\(" crates/media/src/lib.rs crates/export/src/lib.rs`、`python3 scripts/package_release_asset.py --binary target/debug/pauseink-app --platform linux-x86_64 --version dev-smoke --format tar.gz --output-dir <temp>`、`tar -tzf <artifact>` を通し、production 側の child process spawn が helper へ集約され、archive 内に `README.md` と `presets/style_presets` / `presets/export_profiles` が入ることを確認した
  - Linux host では `/usr/bin/ffmpeg`、`/usr/bin/ffprobe`、`ffmpeg 6.1.1-3ubuntu5` を実確認した
  - `cargo test -p pauseink-template-layout`、`cargo test -p pauseink-app --lib --bins`、`cargo test --workspace`、`cargo check -p pauseink-app --all-targets`、`cargo build -p pauseink-app` を通過
  - live preview 追加後も `cargo fmt --all`、`cargo test -p pauseink-app --lib --bins`、`cargo test --workspace`、`cargo check -p pauseink-app --all-targets` を通過
  - slot/style/guide 修正後も `cargo fmt --all`、`cargo test --workspace`、`cargo check -p pauseink-app --all-targets` を通過
  - workflow YAML parse、packager `py_compile`、release archive 生成のローカル検証を通過
  - `cargo test --workspace` を通過
  - `cargo check -p pauseink-app --all-targets` を通過
  - `cargo build -p pauseink-app` を通過
  - `cargo build --release -p pauseink-app` を通過
  - `cargo check --workspace --target x86_64-pc-windows-gnu` は target 未導入で失敗し、blocker を記録
  - `timeout 5s ./target/debug/pauseink-app` は display server 不在で失敗し、headless host 制約として記録
- 現在の未解決制約:
  - release 用 portable sidecar runtime の同梱 / provenance 整備は未着手
  - GitHub Release workflow が生成する成果物は現時点では app binary + `README.md` + `presets/` を含む archive までで、FFmpeg sidecar 同梱はまだ含まれない
  - Windows cross-build は `x86_64-pc-windows-gnu` target 未導入が blocker
  - Windows / macOS の runtime 実行確認はこの Linux host では行えず、現時点では探索ロジックの unit test と Linux 実機検証まで
  - Windows 実機での console 点滅解消確認と macOS 実機での runtime 実行確認は、この Linux host では未実施
  - GUI 実機での「保存済み project open 後に media が即復元されるか」の目視確認は、この Linux host では未実施
  - reveal-head effect と post-action chain は domain 型までで、renderer / inspector UI は未接続
  - clear / combo preset の専用 UI は未接続
  - headless host では GUI 実表示 smoke を実行できない

## フェーズ進行表

| Phase | 状態 | 進捗目安 | 直近ゴール | 備考 |
|---|---|---|---|---|
| Phase 0 | 完了 | 100% | 進行表・実装レポート初期化、最初の sub-agent review 完了 | 必読 docs 読了、review 結果取り込み済み |
| Phase 1 | 完了 | 100% | workspace / crate 骨格 | app/domain/project_io/portable_fs/presets_core/fonts/template/media/renderer/export/ui を整理 |
| Phase 2 | 完了 | 100% | domain model と clear/page 仕様 | typed stroke/object/group/style/clear と clear semantics を固定 |
| Phase 3 | 完了 | 100% | `.pauseink` lenient load / normalized save | typed wrapper、entity extra 維持、sample roundtrip を確認 |
| Phase 4 | 完了 | 100% | portable root と設定保存 | env override、autosave/cache/runtime/temp/helper、cleanup API を実装 |
| Phase 5 | 完了 | 100% | command model と bounded undo/redo | history limit 設定と app session 接続、smoke test 追加 |
| Phase 6 | 完了 | 100% | preset / export profile 基盤 | export profile / base style preset loader、family/profile accessors を実装 |
| Phase 7 | 完了 | 100% | local font / Google Fonts 基盤 | fetch/caching/graceful failure を unit test 付きで実装 |
| Phase 8 | 完了 | 100% | template layout / guide geometry | preview と slot 生成を v1.0 範囲で固定 |
| Phase 9 | 完了 | 100% | FFmpeg provider / probe / capability | preview frame と diagnostics、host smoke を実装 |
| Phase 10 | 完了 | 100% | 再生基盤 | preview canvas と transport 接続、import/playback smoke を実装 |
| Phase 11 | 完了 | 100% | free ink capture と stabilization | GUI 経由の free ink / grouping / undo-redo と smoke を実装 |
| Phase 12 | 完了 | 100% | guide system | guide capture と editor-only guide 表示を v1.0 範囲で実装 |
| Phase 13 | 完了 | 100% | outline / groups / page events | object/page event track を v1.0 最小線で実装 |
| Phase 14 | 進行中 | 92% | style / entrance / clear effects | outline / drop shadow / glow / entrance UI と preset/save は接続済み。reveal-head effect と post-action chain が残る |
| Phase 15 | 完了 | 100% | export UI と export engine | custom 編集、queue、transparent/composite smoke を確認 |
| Phase 16 | 完了 | 100% | preferences / cache manager / recovery | preferences/cache manager/runtime diagnostics/recovery を実装 |
| Phase 17 | 進行中 | 99% | README / manuals / tutorials / polish | 残 task 計画を `.docs/16_remaining_tasks_plan.md` へ整理し、具体例と先決事項まで反映済み |
| Phase 18 | 完了 | 100% | `V1-05` の選択・グループ・前後移動基盤を閉じる | outline 起点の複数選択、group / ungroup、batch style / entrance、z-order foundation を history 付きで接続 |

## 次の具体的な一手

1. display server がある Linux または実機 Windows で、outline 起点の複数選択 / group / z-order、effect editor、出現速度 editor、template 字詰め、guide 進行を目視確認する。
2. `.docs/16_remaining_tasks_plan.md` に従って `V1-09` から `V1-14` までの template/UI 系バッチを順に進める。
3. その後 `V1-02 reveal-head effect`、`V1-04 clear/combo preset`、`V1-06`、`V1-03 post-action chain`、packaging / QA の順で閉じる。
