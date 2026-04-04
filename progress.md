# PauseInk 進捗

このファイルは短く、最新で、事実ベースに保つ。

## 現在地

- 作業ブランチ: `prototype`
- 目標バージョン: `v1.0.0`
- 全体状態: 全体で概算 34% 前後。Phase 2/3 の typed 基盤を広げ、次は project command と undo 接続へ進む
- 現在の即時マイルストーン: typed domain/project model を使う project command を追加し、編集操作を履歴へ載せる
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
  - sidecar/system runtime discovery と host `ffprobe` smoke test を追加
  - built-in export family catalog、profile loader、`settings_buckets` schema を追加
  - `presets/export_profiles/` を stable schema と日本語説明へ更新
  - `export` crate に bucket 解決付き concrete settings 計算と capability 判定を追加
  - stroke / glyph object / group / style snapshot などの typed domain model を追加
  - `.pauseink` の strokes / objects / groups / clear_events を typed wrapper 化
  - `cargo test --workspace` を通過
- 現在のブロッカー:
  - portable sidecar runtime 向け manifest 実体と export engine 本体が未実装
  - クロスビルド target はまだ Linux ホスト分のみ

## フェーズ進行表

| Phase | 状態 | 進捗目安 | 直近ゴール | 備考 |
|---|---|---|---|---|
| Phase 0 | 完了 | 100% | 進行表・実装レポート初期化、最初の sub-agent review 完了 | 必読 docs 読了、review 結果取り込み方針確定 |
| Phase 1 | 完了 | 100% | workspace / crate 骨格を実装可能な形へ拡張 | `app` を薄くし `ui` を独立境界にした |
| Phase 2 | 実行中 | 55% | domain model と clear/page 仕様を固定 | stroke / object / group / style / entrance の typed 基盤を追加 |
| Phase 3 | 実行中 | 50% | `.pauseink` lenient load / normalized save | typed wrapper と entity-level unknown field 保持を追加 |
| Phase 4 | 実行中 | 35% | portable root と設定保存 | 実行ファイル隣接ルールの最小実装あり |
| Phase 5 | 実行中 | 30% | command model と bounded undo/redo | 既定深さ 256 の最小実装あり |
| Phase 6 | 実行中 | 60% | preset / export profile 基盤 | built-in family catalog と profile loader を追加 |
| Phase 7 | 実行中 | 30% | local font / Google Fonts 基盤 | graceful failure の最小実装あり |
| Phase 8 | 実行中 | 35% | template layout / guide geometry | grapheme-aware 最小実装あり |
| Phase 9 | 実行中 | 75% | FFmpeg provider / probe / capability | runtime discovery と host smoke を追加 |
| Phase 10 | 未着手 | 0% | 再生基盤 | import / seek / pause / play |
| Phase 11 | 未着手 | 0% | free ink capture と stabilization | raw points を保持 |
| Phase 12 | 未着手 | 0% | guide system | 非 export を保証 |
| Phase 13 | 未着手 | 0% | outline / groups / page events | run 表示を分離 |
| Phase 14 | 未着手 | 0% | style / entrance / clear effects | built-in effects のみ |
| Phase 15 | 実行中 | 20% | export UI と export engine | bucket 解決と capability 判定の基礎を追加 |
| Phase 16 | 未着手 | 0% | preferences / cache manager / recovery | autosave と復旧を検証 |
| Phase 17 | 未着手 | 0% | README / manuals / tutorials / polish | 全文書を日本語へ統一 |
| Phase 18 | 未着手 | 0% | 最終 build / test / export / Windows build 試行 | done criteria を満たすまで終了しない |

## 次の具体的な一手

1. stroke / object / group / clear event を操作する project command を追加する。
2. typed project model と undo/redo の接続を作る。
3. command 経由で page derivation と save/load が壊れないことをテストする。
