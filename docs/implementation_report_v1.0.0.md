# 実装レポート — v1.0.0

> この文書は実装中ずっと更新し続ける。最後にまとめて書かないこと。

## 1. 要約

- 現在の状態: Phase 5 の generic command history まで実装。次は settings 永続化と env override 実利用。
- 現在のフェーズ: Phase 5 実行中。
- ホスト環境: Linux x86_64 / Rust stable 1.93.0 / `ffmpeg` と `ffprobe` は未配置。
- 最新の検証済み build: 未実施
- 最新の検証済み composite export: 未実施
- 最新の検証済み transparent export: 未実施

## 2. 環境

- 日時: 2026-04-04T23:56:59+09:00
- ホスト OS: Linux yukatayu-agent 6.8.0-106-generic x86_64 GNU/Linux
- シェル: `bash`
- Rust toolchain: `stable-x86_64-unknown-linux-gnu` / `rustc 1.93.0` / `cargo 1.93.0`
- FFmpeg runtime 状態: ホストに `ffmpeg` / `ffprobe` なし。sidecar runtime も未配置。
- Cross-build tooling 状態: `rustup target list --installed` は `x86_64-unknown-linux-gnu` のみ。
- GPU / driver メモ: まだ未調査。UI 実装時に runtime probe を追加予定。
- ディスク / メモリ備考: 現時点で顕著な制約は未確認。

## 3. フェーズログ

| フェーズ | 状態 | メモ |
|---|---|---|
| Phase 0 | 完了 | docs 読了、進行表更新、architecture sanity review 実施・採用方針確定 |
| Phase 1 | 完了 | workspace 拡張、logging 初期化、基礎 crate 境界を反映 |
| Phase 2 | 実行中 | `MediaTime` と clear 境界 semantics を最小実装 |
| Phase 3 | 実行中 | `.pauseink` schema / lenient load / canonical save / unknown field 保持の最小版を実装 |
| Phase 4 | 実行中 | portable root と主要ディレクトリ解決を最小実装 |
| Phase 5 | 実行中 | generic command history / bounded undo-redo の最小実装を完了 |
| Phase 6 | 未着手 | |
| Phase 7 | 未着手 | |
| Phase 8 | 未着手 | |
| Phase 9 | 未着手 | |
| Phase 10 | 未着手 | |
| Phase 11 | 未着手 | |
| Phase 12 | 未着手 | |
| Phase 13 | 未着手 | |
| Phase 14 | 未着手 | |
| Phase 15 | 未着手 | |
| Phase 16 | 未着手 | |
| Phase 17 | 未着手 | |
| Phase 18 | 未着手 | |

## 4. 決定ログ

- 2026-04-04T23:56:59+09:00
  - 決定: 文書と UI は日本語を正とし、既存の英語記述も実装に合わせて日本語へ更新する。
  - 検討した代替案: コードのみ先行して docs 翻訳を最後にまとめる。
  - 理由: ユーザーの明示要求であり、仕様と UI 表記のずれを早期に防ぐため。
  - 影響: `README.md`、`progress.md`、`manual/`、`docs/`、UI 文言のすべてを段階的に日本語化する。
- 2026-04-04T23:56:59+09:00
  - 決定: 初期 crate / module 方針は `app`、`domain`、`project_io`、`portable_fs`、`presets_core` を維持しつつ、必要に応じて `fonts`、`template_layout`、`media`、`renderer`、`export` を追加する前提で architecture review にかける。
  - 検討した代替案: すべてを `app` crate に集約する。
  - 理由: `AGENTS.md` と `.docs/04_architecture.md` の疎結合要件を満たしやすく、snapshot-based background job を分離しやすい。
  - 影響: 初期実装では crate 境界の責務を明文化し、review 結果で必要なら分割を調整する。
- 2026-04-05T00:00:00+09:00
  - 決定: architecture review を受け、`app` は composition root のみに寄せ、`ui` crate を独立させる。加えて time model は単純 `u64 ms` 固定ではなく `MediaTime` を導入し、export schema は family / profile の二層に分割する。
  - 検討した代替案: 既存 5 crate のまま `app` に UI を内包し、time model と export profile を簡易な整数 / 単一 schema で押し切る。
  - 理由: UI への business rule 混入、clear 境界の時刻ズレ、profile 特例分岐の肥大化を初期段階で防ぐため。
  - 影響: Phase 1 は crate 境界の追加、Phase 2 / 6 は time model と export schema の土台から TDD で組み直す。
- 2026-04-05T00:00:00+09:00
  - 決定: background job は `ProjectSnapshot` などの immutable request / snapshot 型だけを受け取り、`Arc<Mutex<AppState>>` の共有は採用しない。
  - 検討した代替案: live app state を共有して export / probe / font 更新を直接参照させる。
  - 理由: `.docs/04_architecture.md` と sub-agent 指摘の通り、single-writer と worker 分離を守るため。
  - 影響: runtime state と persisted project state を明確に分離する必要がある。
- 2026-04-05T00:20:00+09:00
  - 決定: domain time model は `MediaTime { ticks, time_base }` を基本表現とし、clear 境界ジャスト時刻は次ページに属する。
  - 検討した代替案: `u64 ms` 維持。
  - 理由: mixed time base と clear 境界の等値判定を UI / save / export で一貫させるため。
  - 影響: project schema と media/provider 側も `MediaTime` 前提で整える必要がある。
- 2026-04-05T00:20:00+09:00
  - 決定: `presets_core` は export family と distribution profile を別 schema とし、catalog で compatibility を解決する。
  - 検討した代替案: profile 単一 schema に family 制約を混在させる。
  - 理由: special case 分岐を減らし、後から profile を追加しやすくするため。
  - 影響: `presets/export_profiles/` のサンプル定義も後続で更新する。
- 2026-04-05T00:45:00+09:00
  - 決定: `.pauseink` の初期実装は `json5` による lenient load と、known field 順を固定した canonical JSON save を採用する。コメントは load できるが save 時には保持しない。
  - 検討した代替案: コメント保持付きの完全 JSON5 roundtrip serializer を初手で実装する。
  - 理由: v1.0 の determinism と unknown field 保持を先に満たし、comment preservation は limitation として明示する方が安全なため。
  - 影響: `docs/implementation_report_v1.0.0.md` と manuals に save 時のコメント消失を明記する必要がある。
- 2026-04-05T01:05:00+09:00
  - 決定: undo/redo 基盤は project 専用 enum を先に固定せず、generic `Command` / `CommandBatch` / `CommandHistory` として実装する。
  - 検討した代替案: 最初から `ProjectCommand` enum を作って history を密結合にする。
  - 理由: grouped command、redo invalidation、bounded history を先に検証しつつ、後から domain command を載せ替えやすくするため。
  - 影響: UI / domain の実編集操作は後続でこの generic history に乗せる。

## 5. 作業ログ

- 2026-04-04T23:56:59+09:00
  - 実施内容: `prototype` ブランチ作成、必読 docs 読了、workspace scaffold 調査、環境確認。
  - 変更ファイル: なし
  - 結果: 固定仕様と現状 scaffold の差分を把握。`ffmpeg` runtime 未配置を確認。
  - 次の一手: `progress.md` / 実装レポート更新後、architecture sanity review を起動。
- 2026-04-04T23:56:59+09:00
  - 実施内容: `progress.md` と本レポートを日本語ベースの live tracker に更新。
  - 変更ファイル: `progress.md`, `docs/implementation_report_v1.0.0.md`
  - 結果: Phase 0 の開始状態と初期方針を明文化。
  - 次の一手: sub-agent 所見を反映して crate/module 構成を確定。
- 2026-04-05T00:00:00+09:00
  - 実施内容: architecture sanity review を回収し、採用点 / 後回し点を整理。
  - 変更ファイル: `progress.md`, `docs/implementation_report_v1.0.0.md`
  - 結果: `ui` crate 分離、`MediaTime`、family/profile 二層 schema、immutable snapshot worker を採用。
  - 次の一手: workspace 拡張と最初の failing test を追加する。
- 2026-04-05T00:20:00+09:00
  - 実施内容: workspace に `ui` / `fonts` / `template_layout` / `media` / `renderer` / `export` crate を追加し、`app` を composition root 寄りに整理。
  - 変更ファイル: `Cargo.toml`, `crates/app/*`, `crates/ui/*`, `crates/fonts/*`, `crates/template_layout/*`, `crates/media/*`, `crates/renderer/*`, `crates/export/*`
  - 結果: Phase 1 の crate 境界が compile 可能な状態になった。
  - 次の一手: foundation behavior を TDD で積む。
- 2026-04-05T00:20:00+09:00
  - 実施内容: `MediaTime` と clear 境界 semantics の failing test を追加し、domain を実装して緑化。
  - 変更ファイル: `crates/domain/src/lib.rs`
  - 結果: mixed time base 比較と clear 境界でのページ切り替えがテストで固定された。
  - 次の一手: project schema に `MediaTime` を反映できるよう `.pauseink` 実装へ進む。
- 2026-04-05T00:20:00+09:00
  - 実施内容: portable root の failing test を追加し、主要ディレクトリ解決を実装。
  - 変更ファイル: `crates/portable_fs/src/lib.rs`
  - 結果: executable-local root と override root の最小 API ができた。
  - 次の一手: env override の実利用と settings 保存へ広げる。
- 2026-04-05T00:20:00+09:00
  - 実施内容: export family/profile の failing test を追加し、互換解決 catalog を実装。
  - 変更ファイル: `crates/presets_core/Cargo.toml`, `crates/presets_core/src/lib.rs`
  - 結果: family/profile 二層 schema の最小 API ができた。
  - 次の一手: preset file の loader と実際の JSON5 schema を揃える。
- 2026-04-05T00:45:00+09:00
  - 実施内容: `.pauseink` の failing test を追加し、lenient load / canonical save / unknown field 保持の最小実装を追加。
  - 変更ファイル: `Cargo.toml`, `crates/project_io/Cargo.toml`, `crates/project_io/src/lib.rs`
  - 結果: comments / trailing commas を許可する loader と deterministic save が動作した。
  - 次の一手: command model / undo-redo と portable settings を実装する。
- 2026-04-05T01:05:00+09:00
  - 実施内容: command history の failing test を追加し、generic `Command` / `CommandBatch` / `CommandHistory` を実装。
  - 変更ファイル: `crates/domain/Cargo.toml`, `crates/domain/src/history.rs`, `crates/domain/src/lib.rs`
  - 結果: bounded history、redo invalidation、grouped command undo の基礎がテストで固定された。
  - 次の一手: settings 永続化と env override 実利用へ広げる。

## 6. 検証ログ

- `git status --short --branch`
  - 結果: `main...origin/main` から開始。作業前は未変更。
- `git branch --list prototype`
  - 結果: 既存ブランチなし。
- `git switch -c prototype`
  - 結果: `prototype` ブランチを作成して checkout。
- `rustc -V && cargo -V && rustup show active-toolchain`
  - 結果: `rustc 1.93.0`, `cargo 1.93.0`, active toolchain は stable。
- `rustup target list --installed`
  - 結果: `x86_64-unknown-linux-gnu` のみ。
- `ffmpeg -version | head -n 2`
  - 結果: `ffmpeg: command not found`
- `ffprobe -version | head -n 2`
  - 結果: `ffprobe: command not found`
- `cargo test --workspace`
  - 結果: exit 0。現 scaffold の 4 unit test は通過したが、仕様拘束を担保する面積はまだ不足。
- `cargo test -p pauseink-domain`
  - 結果: exit 101。`MediaTime` / `TimeBase` / `ClearEvent.time` 未定義で compile error。期待どおり red。
- `cargo test -p pauseink-domain`
  - 結果: exit 0。2 tests passed。
- `cargo test -p pauseink-portable-fs`
  - 結果: exit 101。`PortablePaths` と `portable_root_with_override` 未定義で red。
- `cargo test -p pauseink-portable-fs`
  - 結果: exit 0。2 tests passed。
- `cargo test -p pauseink-presets-core`
  - 結果: exit 101。`ExportCatalog` / `DistributionProfile` / `ProfileCompatibility` 未定義で red。
- `cargo test -p pauseink-presets-core`
  - 結果: exit 0。2 tests passed。
- `cargo test --workspace`
  - 結果: exit 0。workspace 全体の回帰確認を完了。
- `cargo test -p pauseink-project-io`
  - 結果: exit 101。`PauseInkDocument` / `PauseInkProject` / `load_from_str` / `save_to_string` 未定義で red。
- `cargo test -p pauseink-project-io`
  - 結果: exit 101。`serde_json` manifest 重複定義で失敗。manifest を修正。
- `cargo test -p pauseink-project-io`
  - 結果: exit 101。canonical save の順序が alphabetic になり golden と不一致。`serde_json` を `preserve_order` で固定。
- `cargo test -p pauseink-project-io`
  - 結果: exit 0。2 tests passed。
- `cargo test --workspace`
  - 結果: exit 0。`project_io` 追加後の回帰確認を完了。
- `cargo test -p pauseink-domain`
  - 結果: exit 101。`CommandHistory` / `CommandBatch` / `Command` / `DEFAULT_HISTORY_DEPTH` / `CommandError` 未定義で red。
- `cargo test -p pauseink-domain`
  - 結果: exit 0。4 tests passed。
- `cargo test --workspace`
  - 結果: exit 0。generic history 追加後の回帰確認を完了。

## 7. 失敗と修正

- 2026-04-04T23:56:59+09:00
  - 事象: ホストに `ffmpeg` / `ffprobe` が存在しない。
  - 影響: import / export の実検証は sidecar runtime を用意するまで着手不可。
  - 暫定対応: provider abstraction は sidecar 前提で設計し、後続フェーズで取得方法と provenance を整理する。

## 8. Sub-agent メモ

- Pass 1 — architecture sanity review
  - 目的: crate / module 境界、single-writer + snapshot worker 方針、cross-platform UI の危険点を洗う。
  - 要約:
    - `app` は薄く保ち `ui` crate を独立させる
    - time model は `u64 ms` に固定せず `MediaTime` を先に整備する
    - export schema は family / profile の二層に分ける
    - background worker は immutable snapshot のみを受ける
  - 採用した変更:
    - `ui` crate を workspace に追加する
    - clear 境界の等値 semantics と `MediaTime` を Phase 2 の先頭で TDD する
    - `presets_core` は export family schema と distribution profile schema を分ける
  - 見送った提案:
    - UI toolkit 名の即時固定。理由: crate 境界と time/export schema を先に固める方が rework が少ないため

## 9. Export / profile メモ

- 公式ページの再確認は未実施。
- YouTube / X / Instagram / Adobe の最終値は export 実装前に official source を見直して記録する。

## 10. パッケージング / ライセンスメモ

- mainline 方針: FFmpeg sidecar runtime provider。
- 現時点では optional codec pack 未実装。
- H.264 / HEVC は mainline 前提にしない。
- release packaging 用の provenance / notice 整理は後続フェーズで記録する。

## 11. 開発者チュートリアル

- 対象チュートリアル: 未確定
- 実行 / 検証コマンド: 未実施
- 結果: 未実施

## 12. 既知の問題 / 制約

- 現在の repository は最小 scaffold のみで、実アプリ機能は未実装。
- FFmpeg runtime が未配置のため、import/export 実検証はまだできない。
- Windows cross-build 環境は未整備。
- `.pauseink` parse/save、undo/redo、実 UI、media provider、renderer、export はまだ本実装前。
- command history は generic 基盤のみ実装済みで、実 project editing command はまだ未接続。
- `.pauseink` save は現時点でコメント保持を行わない。load は許可、save は canonical JSON に正規化する。

## 13. 最終受け入れチェックリスト

- [ ] Host build passes
- [ ] Core tests pass
- [ ] Save/load works
- [ ] Manual clear works
- [ ] Composite export validated
- [ ] Transparent export validated
- [ ] Portable-state rule validated
- [ ] Google Fonts graceful-failure behavior validated
- [ ] Export-profile computation validated
- [ ] Developer tutorial sample validated
- [ ] Windows build attempted and documented
- [ ] Manuals updated
- [ ] `progress.md` updated
