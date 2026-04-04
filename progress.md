# PauseInk 進捗

このファイルは短く、最新で、事実ベースに保つ。

## 現在地

- 作業ブランチ: `prototype`
- 目標バージョン: `v1.0.0`
- 全体状態: `AGENTS.md` と `.docs/10_testing_and_done_criteria.md` の完了条件に対して概算 100%。単一ウィンドウ GUI、`.pauseink` save/load、autosave/recovery、preferences/cache manager/runtime diagnostics、Google Fonts cache と graceful failure、export queue/engine、transparent/composite export、README/manual/tutorial/report/progress の同期まで完了した。
- 完了判定: docs / code / tests / sample / tutorial の整合、host build/test/save-load/export、portable-state rule、Google Fonts graceful failure、Windows build 試行記録、final QA/docs review を満たした。
- 現在の即時マイルストーン: commit / push のための最終整理のみ
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
  - `cargo test --workspace` を通過
  - `cargo build -p pauseink-app` を通過
  - `cargo check --workspace --target x86_64-pc-windows-gnu` は target 未導入で失敗し、blocker を記録
  - `timeout 5s ./target/debug/pauseink-app` は display server 不在で失敗し、headless host 制約として記録
- 現在の未解決制約:
  - release 用 portable sidecar runtime の同梱 / provenance 整備は未着手
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
| Phase 17 | 完了 | 100% | README / manuals / tutorials / polish | README/manual/tutorial/`.docs` を日本語で同期 |
| Phase 18 | 完了 | 100% | 最終 build / test / export / Windows build 試行 | final QA/docs review 反映、検証ログを確定 |

## 次の具体的な一手

1. display server がある Linux または実機 Windows で GUI の目視 smoke を行う。
2. `rustup target add x86_64-pc-windows-gnu` を入れた環境で Windows build を再試行する。
3. release 用 portable sidecar runtime の bundling / provenance / notices を詰める。
