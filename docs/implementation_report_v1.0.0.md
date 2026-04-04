# 実装レポート — v1.0.0

> この文書は実装中ずっと更新し続ける。最後にまとめて書かないこと。

## 1. 要約

- 現在の状態: `media` に runtime origin / raw probe / capability parser、`presets_core` に runtime tier / source kind を追加済み。host では Ubuntu apt の `ffmpeg` / `ffprobe` が利用可能。
- 現在のフェーズ: Phase 9 実行中。
- ホスト環境: Linux x86_64 / Rust stable 1.93.0 / host に Ubuntu apt `ffmpeg 6.1.1-3ubuntu5` と `ffprobe 6.1.1-3ubuntu5` がある。portable sidecar runtime は未配置。
- 最新の検証済み build: 未実施
- 最新の検証済み composite export: 未実施
- 最新の検証済み transparent export: 未実施

## 2. 環境

- 日時: 2026-04-04T23:56:59+09:00
- ホスト OS: Linux yukatayu-agent 6.8.0-106-generic x86_64 GNU/Linux
- シェル: `bash`
- Rust toolchain: `stable-x86_64-unknown-linux-gnu` / `rustc 1.93.0` / `cargo 1.93.0`
- FFmpeg runtime 状態: host に `ffmpeg 6.1.1-3ubuntu5` / `ffprobe 6.1.1-3ubuntu5` があり、検証に利用可能。sidecar runtime は未配置。
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
| Phase 4 | 実行中 | portable root / env override / settings.json5 の最小実装を完了 |
| Phase 5 | 実行中 | generic command history / bounded undo-redo の最小実装を完了 |
| Phase 6 | 実行中 | family/profile 二層 schema の最小実装を完了 |
| Phase 7 | 実行中 | local font family 列挙、Google Fonts CSS2 URL / cache path の最小実装 |
| Phase 8 | 実行中 | template layout / guide geometry の最小実装を完了 |
| Phase 9 | 実行中 | runtime provenance、raw probe、capability parser、schema 補強を実装済み |
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
- 2026-04-05T01:20:00+09:00
  - 決定: settings は `settings.json5` を lenient load / canonical JSON save で扱い、既定値は sample に合わせて `history_depth=256`、GPU preview / media HW accel は有効、Google Fonts は有効とする。
  - 検討した代替案: settings 保存を後回しにして app 起動時のハードコード設定だけで進める。
  - 理由: portable-state ルールと履歴深さ設定を早い段階で実コードに落とし込むため。
  - 影響: fonts / UI / export は以後この settings 構造を参照できる。
- 2026-04-05T01:35:00+09:00
  - 決定: Google Fonts は official CSS2 API を使って family ごとの CSS URL を構築し、キャッシュファイル名は portable root 配下で family 名を slug 化して管理する。
  - 検討した代替案: Google Fonts の全メタデータ API や重いブラウザ依存取得に寄せる。
  - 理由: v1.0 では configured family を軽量に取得できれば十分であり、broken entry を UI 全体から切り離しやすいため。
  - 影響: 実ダウンロード段階では CSS から最初の `url(...)` を抽出し、失敗時はその family だけを非表示扱いにできる。
- 2026-04-05T01:50:00+09:00
  - 決定: host に入った Ubuntu apt 版 `ffmpeg` / `ffprobe` は検証用 runtime として使うが、mainline 実装方針は引き続き portable sidecar provider として維持する。
  - 検討した代替案: system `ffmpeg` を mainline 依存とみなす。
  - 理由: apt build は `--enable-gpl` を含み、release packaging 方針と切り分けて扱う必要があるため。
  - 影響: 実装レポートと packaging/licensing notes に host 検証 runtime と sidecar mainline 方針の差を明記する。
- 2026-04-05T00:32:01+09:00
  - 決定: export 実装前に `MediaRuntime` へ runtime origin / manifest / provenance を持たせ、`presets_core` には family ごとの codec/muxer/alpha/audio requirement と profile ごとの concrete rule data を追加する。
  - 検討した代替案: 現在の `PathBuf` 2 本と `compatibility` だけの最小 schema のまま export orchestration で補う。
  - 理由: mainline sidecar と host 検証 runtime を区別できないと packaging/licensing の境界が崩れ、family/profile 側の schema が薄すぎると UI/export 実装へ hard-code が漏れるため。
  - 影響: Phase 9-15 の前に media/provider と presets schema の補強テストを追加する必要がある。
- 2026-04-05T02:20:00+09:00
  - 決定: `MediaRuntime` は `RuntimeOrigin`、`manifest_path`、`build_summary`、`license_summary` を持ち、probe では `avg_frame_rate_raw`、`r_frame_rate_raw`、`pix_fmt`、`has_alpha`、`has_audio` を保持する。
  - 検討した代替案: human-readable summary のみで raw probe 情報を捨てる。
  - 理由: VFR / alpha / optional codec pack 判定に raw metadata が必要だから。
  - 影響: import caveat と export capability 判定を provider 側で持てる。
- 2026-04-05T02:20:00+09:00
  - 決定: `ExportFamily` は `RuntimeTier` と codec/muxer requirement を持ち、`DistributionProfile` は `ProfileSourceKind` と source URL / notes を持つ。
  - 検討した代替案: runtime tier 判定を export orchestration 側の if 文で行う。
  - 理由: H.264 / HEVC optional codec pack を schema で mainline から切り離すため。
  - 影響: JSON5 preset file もこの schema へ寄せる必要がある。

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
- 2026-04-05T01:20:00+09:00
  - 実施内容: portable settings と env override の failing test を追加し、`Settings` と `settings.json5` 保存基盤を実装。
  - 変更ファイル: `crates/portable_fs/Cargo.toml`, `crates/portable_fs/src/lib.rs`
  - 結果: env override 付き root 解決、settings ファイル位置、既定値 roundtrip をテストで固定した。
  - 次の一手: local fonts / Google Fonts catalog と graceful failure を実装する。
- 2026-04-05T01:35:00+09:00
  - 実施内容: fonts crate に failing test を追加し、Google Fonts CSS2 URL / cache path / CSS parser と local font family 列挙を実装。
  - 変更ファイル: `crates/fonts/Cargo.toml`, `crates/fonts/src/lib.rs`
  - 結果: broken CSS は `None` で握りつぶせる形になり、missing extra dirs も無視できるようにした。
  - 次の一手: template layout と guide geometry を実装する。
- 2026-04-05T01:50:00+09:00
  - 実施内容: template layout の failing test を追加し、grapheme-aware slot / scale / slope / guide geometry を実装。
  - 変更ファイル: `crates/template_layout/Cargo.toml`, `crates/template_layout/src/lib.rs`
  - 結果: grapheme cluster 単位の slot と 5 本構成の guide line がテストで固定された。
  - 次の一手: host `ffprobe` を使った media provider の入口を実装する。
- 2026-04-05T00:32:01+09:00
  - 実施内容: media/export/licensing sanity review を実施し、`AGENTS.md`、`.docs/07`、`.docs/08`、`.docs/10`、`.docs/11`、`progress.md`、実装レポート、`crates/media/src/lib.rs`、`crates/presets_core/src/lib.rs` を再確認した。
  - 変更ファイル: `progress.md`, `docs/implementation_report_v1.0.0.md`
  - 結果: runtime provenance 欠落、probe 情報不足、family/profile schema の薄さ、host 検証 runtime と release packaging の記録不足を Phase 9 の主要論点として確定した。
  - 次の一手: runtime origin / manifest / capability と export family/profile schema の failing test を追加する。
- 2026-04-05T02:20:00+09:00
  - 実施内容: `media` に runtime origin / raw probe / capability parser を追加し、`presets_core` に runtime tier / source kind を追加。
  - 変更ファイル: `crates/media/src/lib.rs`, `crates/presets_core/src/lib.rs`
  - 結果: host runtime と mainline sidecar の切り分け、alpha/VFR などの raw probe 情報保持、optional codec pack tier の区別が code/schema 上に現れた。
  - 次の一手: host `ffprobe` 実 probe と JSON5 export profile loader を実装する。

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
- `cargo test -p pauseink-portable-fs`
  - 結果: exit 101。`from_override_or_executable_dir` / `Settings` / settings load-save API 未定義で red。
- `cargo test -p pauseink-portable-fs`
  - 結果: exit 0。4 tests passed。
- `cargo test --workspace`
  - 結果: exit 0。portable settings 追加後の回帰確認を完了。
- `cargo search fontdb --limit 1`
  - 結果: `fontdb = "0.23.0"` を確認。
- Google Fonts CSS2 API 公式ページ
  - URL: https://developers.google.com/fonts/docs/css2
  - メモ: base URL は `https://fonts.googleapis.com/css2`、`family=` を複数指定可能、`display=swap` を利用可能。最終閲覧時のページ更新日は 2024-07-23 UTC。
- `cargo test -p pauseink-fonts`
  - 結果: exit 101。Google Fonts URL / cache path / CSS parser API 未定義で red。
- `cargo test -p pauseink-fonts`
  - 結果: exit 101。`fontdb` API の iterator 想定違いで失敗。実 API に合わせて修正。
- `cargo test -p pauseink-fonts`
  - 結果: exit 0。4 tests passed。
- `cargo test --workspace`
  - 結果: exit 0。fonts 追加後の回帰確認を完了。
- `cargo test -p pauseink-template-layout`
  - 結果: exit 101。template layout / guide geometry API 未定義で red。
- `cargo test -p pauseink-template-layout`
  - 結果: exit 0。3 tests passed。
- `ffmpeg -version | head -n 3`
  - 結果: `ffmpeg version 6.1.1-3ubuntu5`。Ubuntu apt build、`--enable-gpl` を含む。
- `ffprobe -version | head -n 3`
  - 結果: `ffprobe version 6.1.1-3ubuntu5`。Ubuntu apt build、`--enable-gpl` を含む。
- `cargo test --workspace`
  - 結果: exit 0。template layout 追加後の回帰確認を完了。
- `cargo test -p pauseink-media -p pauseink-presets-core`
  - 結果: exit 0。`pauseink-media` 3 tests、`pauseink-presets-core` 2 tests が通過。
- `cargo test -p pauseink-media`
  - 結果: exit 101。`RuntimeOrigin` / raw probe fields 未定義で red。
- `cargo test -p pauseink-presets-core`
  - 結果: exit 101。`RuntimeTier` / `OutputKind` / `ProfileSourceKind` 未定義で red。
- `cargo test -p pauseink-media`
  - 結果: exit 101。`RuntimeCapabilities` 未定義で red。
- `cargo test -p pauseink-media`
  - 結果: exit 0。5 tests passed。
- `cargo test -p pauseink-presets-core`
  - 結果: exit 0。3 tests passed。
- `cargo test --workspace`
  - 結果: exit 0。runtime/schema 補強後の回帰確認を完了。
- `ffmpeg -version | sed -n '1,12p'`
  - 結果: Ubuntu apt build `6.1.1-3ubuntu5`。`--enable-gpl`、`--enable-libaom`、`--enable-libopus`、`--enable-libsvtav1`、`--enable-libvpx`、`--enable-libx264`、`--enable-libx265` を確認。
- `ffprobe -version | sed -n '1,12p'`
  - 結果: Ubuntu apt build `6.1.1-3ubuntu5`。`--enable-gpl` を含む host 検証 runtime であることを再確認。
- `ffmpeg -encoders | rg 'libx264|libx265|libaom-av1|libsvtav1|libvpx-vp9|prores|mjpeg|aac|libopus'`
  - 結果: host runtime では `libaom-av1`、`libsvtav1`、`libvpx-vp9`、`prores[_ks]`、`mjpeg`、`aac`、`libopus`、`libx264`、`libx265` が見えている。release packaging の既定サポートとはみなさない。
- `ffmpeg -muxers | rg 'webm|mp4|mov|avi|image2'`
  - 結果: host runtime では `webm`、`mp4`、`mov`、`avi`、`image2` muxer を確認。
- `ffmpeg -hwaccels`
  - 結果: host runtime では `vdpau`、`cuda`、`vaapi`、`qsv`、`drm`、`opencl`、`vulkan` が列挙された。実使用可否は別途 runtime probe が必要。

## 7. 失敗と修正

- 2026-04-04T23:56:59+09:00
  - 事象: ホストに `ffmpeg` / `ffprobe` が存在しない。
  - 影響: import / export の実検証は sidecar runtime を用意するまで着手不可。
  - 暫定対応: provider abstraction は sidecar 前提で設計し、後続フェーズで取得方法と provenance を整理する。
- 2026-04-05T00:32:01+09:00
  - 事象: `progress.md` と本レポートの一部に host `ffmpeg` / `ffprobe` 未配置という古い記述が残っていた。
  - 影響: host 検証 runtime と mainline sidecar 方針の区別が読み取りづらく、review 証跡として不正確。
  - 暫定対応: host の Ubuntu apt runtime 利用可能を明記しつつ、release packaging とは分離して扱う方針に統一した。

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
- Pass 2 — media/export/licensing sanity review
  - 目的: runtime/provider 分離、export family/profile 分離、host 検証 runtime と release packaging の境界、Adobe/Web/optional codec pack の扱いを洗う。
  - 要約:
    - `MediaRuntime` が `PathBuf` 2 本だけでは provenance / manifest / diagnostics を持てず、system runtime を mainline に混入させやすい
    - `parse_ffprobe_output` は duration / fps を `f64` に落としており、time base・VFR caveat・alpha/audio の分類に必要な情報が不足している
    - `ExportFamily` / `DistributionProfile` は二層分離自体は正しいが、codec/muxer/audio/alpha/settings/source metadata を保持できず schema が薄い
    - host Ubuntu apt build は review / local validation に使えるが、`--enable-gpl` と `libx264` / `libx265` を含むため mainline release assumption にしてはいけない
  - 採用した変更:
    - Phase 9 以降で runtime origin / manifest / capability parsing を TDD で補強する
    - Phase 6/15 の前に family/profile schema を concrete rule data まで広げる
    - packaging/licensing notes では host 検証 runtime と release sidecar を別項目で記録する
    - `MediaRuntime` に `RuntimeOrigin` と provenance field を追加する
    - `MediaProbe` に raw frame-rate / alpha/audio 情報を追加する
    - `presets_core` に `RuntimeTier` / `ProfileSourceKind` を追加する
  - 見送った提案:
    - host Ubuntu apt build の capability を既定サポート表として docs に固定する。理由: mainline runtime と法務境界が崩れるため

## 9. Export / profile メモ

- 公式ページの再確認は未実施。
- YouTube / X / Instagram / Adobe の最終値は export 実装前に official source を見直して記録する。

## 10. パッケージング / ライセンスメモ

- mainline 方針: FFmpeg sidecar runtime provider。
- 現時点では optional codec pack 未実装。
- H.264 / HEVC は mainline 前提にしない。
- host 検証では Ubuntu apt の `ffmpeg 6.1.1-3ubuntu5` を利用可能。これは `--enable-gpl` を含むため、release packaging とは切り分けて扱う。
- host 検証 runtime の encoder/muxer 列挙結果は local validation 証跡としてのみ使い、mainline release の feature guarantee には転用しない。
- optional codec pack を導入する場合は mainline sidecar とは別 manifest / provenance / notices を持たせ、既定では無効にする。
- release packaging 用の provenance / notice 整理は後続フェーズで記録する。

## 11. 開発者チュートリアル

- 対象チュートリアル: 未確定
- 実行 / 検証コマンド: 未実施
- 結果: 未実施

## 12. 既知の問題 / 制約

- 現在の repository は最小 scaffold のみで、実アプリ機能は未実装。
- portable sidecar runtime が未配置のため、mainline packaging 前提の import/export 実検証はまだできない。
- Windows cross-build 環境は未整備。
- `.pauseink` parse/save、undo/redo、実 UI、media provider、renderer、export はまだ本実装前。
- command history は generic 基盤のみ実装済みで、実 project editing command はまだ未接続。
- settings は最小実装で、ファイル I/O やディレクトリ作成、cache cleanup policy まではまだ未接続。
- Google Fonts は URL / cache path / CSS parser までで、実ダウンロードと UI 連携はまだ未接続。
- media provider / export / 実 UI はまだ未実装で、host `ffmpeg` は probe/capability 設計検証にのみ使っている。
- `.pauseink` save は現時点でコメント保持を行わない。load は許可、save は canonical JSON に正規化する。
- `presets/export_profiles/` の JSON5 実ファイルはまだ新 schema と同期していない。

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
