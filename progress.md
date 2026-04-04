# PauseInk 進捗

このファイルは短く、最新で、事実ベースに保つ。

## 現在地

- 作業ブランチ: `prototype`
- 目標バージョン: `v1.0.0`
- 全体状態: Phase 1 着手準備中
- 現在の即時マイルストーン: `ui` crate 分離、`MediaTime` 基盤、family/profile 二層 schema の開始
- 最新の確認事項:
  - `AGENTS.md` と `.docs/` を全件読了
  - `README.md`、`progress.md`、`manual/`、`presets/`、`samples/`、`docs/implementation_report_v1.0.0.md` を確認
  - `prototype` ブランチを作成
  - workspace を `ui` / `fonts` / `template_layout` / `media` / `renderer` / `export` を含む形へ拡張
  - `MediaTime`、portable root、family/profile 二層 schema の初期実装とテストを追加
  - `.pauseink` の lenient load / canonical save / unknown field 保持の最小実装とテストを追加
  - generic command history と bounded undo/redo の基礎実装を追加
  - `cargo test --workspace` を通過
- 現在のブロッカー:
  - ホスト環境に `ffmpeg` / `ffprobe` が未配置
  - クロスビルド target はまだ Linux ホスト分のみ

## フェーズ進行表

| Phase | 状態 | 直近ゴール | 備考 |
|---|---|---|---|
| Phase 0 | 完了 | 進行表・実装レポート初期化、最初の sub-agent review 完了 | 必読 docs 読了、review 結果取り込み方針確定 |
| Phase 1 | 実行中 | workspace / crate 骨格を実装可能な形へ拡張 | `app` を薄くし `ui` を独立境界にする |
| Phase 2 | 未着手 | domain model と clear/page 仕様を固定 | manual clear / screen-wide を厳守 |
| Phase 3 | 未着手 | `.pauseink` lenient load / normalized save | unknown field 保持を優先 |
| Phase 4 | 未着手 | portable root と設定保存 | 実行ファイル隣接ルールを検証 |
| Phase 5 | 未着手 | command model と bounded undo/redo | 既定深さ 256 |
| Phase 6 | 未着手 | preset / export profile 基盤 | 宣言的定義を優先 |
| Phase 7 | 未着手 | local font / Google Fonts 基盤 | graceful failure 必須 |
| Phase 8 | 未着手 | template layout / guide geometry | grapheme-aware を実装 |
| Phase 9 | 未着手 | FFmpeg provider / probe / capability | export review sub-agent を入れる |
| Phase 10 | 未着手 | 再生基盤 | import / seek / pause / play |
| Phase 11 | 未着手 | free ink capture と stabilization | raw points を保持 |
| Phase 12 | 未着手 | guide system | 非 export を保証 |
| Phase 13 | 未着手 | outline / groups / page events | run 表示を分離 |
| Phase 14 | 未着手 | style / entrance / clear effects | built-in effects のみ |
| Phase 15 | 未着手 | export UI と export engine | CPU-safe baseline を優先 |
| Phase 16 | 未着手 | preferences / cache manager / recovery | autosave と復旧を検証 |
| Phase 17 | 未着手 | README / manuals / tutorials / polish | 全文書を日本語へ統一 |
| Phase 18 | 未着手 | 最終 build / test / export / Windows build 試行 | done criteria を満たすまで終了しない |

## 次の具体的な一手

1. settings 永続化と env override 実利用の failing test を追加する。
2. portable config 保存と履歴深さ設定を実装する。
3. その後 local font / Google Fonts 基盤へ進む。
