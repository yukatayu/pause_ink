# PauseInk 進捗

このファイルは短く、最新で、事実ベースに保つ。

## 現在地

- 作業ブランチ: `prototype`
- 目標バージョン: `v1.0.0`
- 全体状態: `AGENTS.md` と `.docs/10_testing_and_done_criteria.md` の完了条件に対して概算 100%。単一ウィンドウ GUI、`.pauseink` save/load、autosave/recovery、preferences/cache manager/runtime diagnostics、Google Fonts cache と graceful failure、export queue/engine、transparent/composite export、README/manual/tutorial/report/progress の同期、preview 座標ずれと UI 日本語文字化けの修正、template underlay / guide 操作性 / transport discoverability / shortcut / panel resize、描画中ストロークのライブプレビュー、前スロット追加、object style 同期、guide 解除の stale state 解消まで反映済み。
- 完了判定: docs / code / tests / sample / tutorial の整合、host build/test/save-load/export、portable-state rule、Google Fonts graceful failure、Windows build 試行記録、final QA/docs review を再度満たした。
- 現在の即時マイルストーン: 今回の slot/style/guide 修正を report・manual・commit まで閉じる
- 最新の確認事項:
  - `AGENTS.md` と `.docs/` を全件読了
  - `README.md`、`progress.md`、`manual/`、`presets/`、`samples/`、`docs/implementation_report_v1.0.0.md` を確認
  - `prototype` ブランチで作業継続
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
  - effect 実装状況も確認し、renderer には outline / drop shadow / glow があるが、UI/preset loader と cross-stroke ordering は未完であることを整理した
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
  - GitHub Release workflow が生成する成果物は現時点では app binary archive で、FFmpeg sidecar 同梱はまだ含まれない
  - Windows cross-build は `x86_64-pc-windows-gnu` target 未導入が blocker
  - style preset は base style 適用中心で、entrance / clear / combo の UI binding は今後拡張余地がある
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
| Phase 14 | 完了 | 100% | style / entrance / clear effects | renderer built-in effect と base style preset 適用の v1.0 最小線を実装 |
| Phase 15 | 完了 | 100% | export UI と export engine | custom 編集、queue、transparent/composite smoke を確認 |
| Phase 16 | 完了 | 100% | preferences / cache manager / recovery | preferences/cache manager/runtime diagnostics/recovery を実装 |
| Phase 17 | 完了 | 100% | README / manuals / tutorials / polish | template / guide / transport / shortcut UX 差分まで docs と同期 |
| Phase 18 | 完了 | 100% | 最終 build / test / export / Windows build 試行 | 上記 polish 反映後の回帰と docs 同期を再完了 |

## 次の具体的な一手

1. display server がある Linux または実機 Windows で、template 字詰め・font dropdown・transport bar・guide 進行・live stroke preview・前後 slot 移動を目視確認する。
2. `rustup target add x86_64-pc-windows-gnu` を入れた環境で Windows build を再試行する。
3. release 用 portable sidecar runtime の bundling / provenance / notices を詰める。
