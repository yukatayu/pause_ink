# PauseInk 進捗

このファイルは短く、最新で、事実ベースに保つ。

## 現在地

- 作業ブランチ: `prototype`
- 目標バージョン: `v1.0.0`
- 全体状態: Phase 9 実行中、runtime provenance と export schema の補強を反映済み
- 現在の即時マイルストーン: host `ffprobe` smoke と export settings data loader の着手
- 最新の確認事項:
  - `AGENTS.md` と `.docs/` を全件読了
  - `README.md`、`progress.md`、`manual/`、`presets/`、`samples/`、`docs/implementation_report_v1.0.0.md` を確認
  - `prototype` ブランチを作成
  - workspace を `ui` / `fonts` / `template_layout` / `media` / `renderer` / `export` を含む形へ拡張
  - `MediaTime`、portable root、family/profile 二層 schema の初期実装とテストを追加
  - `.pauseink` の lenient load / canonical save / unknown field 保持の最小実装とテストを追加
  - generic command history と bounded undo/redo の基礎実装を追加
  - portable settings と env override の最小実装を追加
  - local font family 列挙と Google Fonts CSS2 URL / cache path の基礎実装を追加
  - template slot / guide geometry の最小実装を追加
  - host 検証用 `ffmpeg` / `ffprobe` 利用可能を確認
  - media runtime origin / raw probe / capability parser の最小実装を追加
  - export family/profile schema を runtime tier / source kind 付きに補強
  - `cargo test --workspace` を通過
- 現在のブロッカー:
  - portable sidecar runtime 向け manifest 実体と export setting loader が未実装
  - クロスビルド target はまだ Linux ホスト分のみ

## フェーズ進行表

| Phase | 状態 | 直近ゴール | 備考 |
|---|---|---|---|
| Phase 0 | 完了 | 進行表・実装レポート初期化、最初の sub-agent review 完了 | 必読 docs 読了、review 結果取り込み方針確定 |
| Phase 1 | 完了 | workspace / crate 骨格を実装可能な形へ拡張 | `app` を薄くし `ui` を独立境界にした |
| Phase 2 | 実行中 | domain model と clear/page 仕様を固定 | `MediaTime` と clear/page semantics の最小実装あり |
| Phase 3 | 実行中 | `.pauseink` lenient load / normalized save | unknown field 保持の最小実装あり |
| Phase 4 | 実行中 | portable root と設定保存 | 実行ファイル隣接ルールの最小実装あり |
| Phase 5 | 実行中 | command model と bounded undo/redo | 既定深さ 256 の最小実装あり |
| Phase 6 | 実行中 | preset / export profile 基盤 | family/profile 二層 schema の最小実装あり |
| Phase 7 | 実行中 | local font / Google Fonts 基盤 | graceful failure の最小実装あり |
| Phase 8 | 実行中 | template layout / guide geometry | grapheme-aware 最小実装あり |
| Phase 9 | 実行中 | FFmpeg provider / probe / capability | media/export/licensing sanity review を実施済み |
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

1. host `ffprobe` を使った実 probe smoke を追加する。
2. `presets/export_profiles/` を新 schema に合わせて読み込む loader を実装する。
3. export concrete setting 計算と runtime capability 判定へ進む。
