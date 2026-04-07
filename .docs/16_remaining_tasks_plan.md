# v1.0 残タスク実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: `superpowers:subagent-driven-development` を推奨。各タスクは task ID 単位で着手し、task 内の設計・実装・検証・docs 更新・commit/push まで完了させること。

**Goal:** 現在の PauseInk 実装に残っている v1.0 未接続要素を、仕様準拠の順序で実装完了できる形に分解し、task ID 指定だけで最後まで進められる実行計画へ落とし込む。

**Architecture:** 既存の crate 境界は維持し、`domain` はデータモデル、`renderer` は可視化評価、`presets_core` は宣言的 catalog、`app` は editor state と GUI だけを持つ。残タスクは「preset/clear/effect の意味論」「selection/outline の編集 UI」「portable sidecar packaging / release / validation」の 3 系統へ分け、依存関係が薄いものから順に閉じる。

**Tech Stack:** Rust workspace, `eframe/egui`, JSON5 project/settings, FFmpeg provider abstraction, GitHub Actions Release

---

## 0. この計画の使い方

- この文書の task ID は実行順を兼ねます。原則として前の依存 task を終えてから次へ進めます。
- 利用者が `V1-03 を実装` や `PKG-01 を実装` のように指定した場合、その task の `スコープ`, `設計`, `変更ファイル`, `テスト`, `完了条件` を満たすまで止めない前提です。
- すべての task で共通して守ること:
  - `progress.md` を開始時 / 完了時に更新する
  - `docs/implementation_report_v1.0.0.md` に判断・コマンド・結果を追記する
  - テストを先に足すか、最低でも red/green の形で回帰を固定する
  - commit は `git -c commit.gpgsign=false commit ...` を使い、日本語メッセージにする
  - commit 後は都度 `origin/develop` へ push する

## 0.1 着手前の共通確認

- 仕様判断は最低でも次を再読する。
  - `.docs/02_final_spec_v1.0.0.md`
  - `.docs/04_architecture.md`
  - `.docs/10_testing_and_done_criteria.md`
- media / export / packaging を触る task では追加で次も確認する。
  - `.docs/07_media_runtime_and_ffmpeg.md`
  - `.docs/08_output_profiles_and_platform_presets.md`
  - `.docs/09_portable_layout_and_cache.md`
  - `.docs/13_risk_register.md`
- 既存部分を壊さないため、着手前に現行の回帰テストを読む。
  - `crates/renderer/src/lib.rs` の entrance / clear / visibility 系 test
  - `crates/app/src/main.rs` の save-restore / guide / export progress / template 系 test
  - `crates/app/src/lib.rs` の session / save-load / clear semantics 系 test
  - `crates/presets_core/src/lib.rs` の preset loader / overlay 系 test
  - `crates/media/src/lib.rs` と `crates/export/src/lib.rs` の runtime / fallback / smoke test
- task に関連する `.docs` と既存 test が薄いと判断した場合は、本実装より先に doc 追記か回帰 test 追加を行う。

## 1. 範囲の切り分け

### 1.1 この計画に含めるもの

以下は v1.0 仕様に対して未接続または最小実装のまま残っているため、この計画で扱います。

1. preset 境界の正規化と field-level reset / inherit
2. reveal-head effect
3. post-action chain
4. clear effect の実装完了と clear / combo preset UI
5. selection / multi-select / group / ungroup / z-order の編集導線
6. object outline / page events panel の強化
7. template / guide advanced controls
8. side panel scroll / overflow hardening
9. template font 切替 crash の修正
10. multiline template editor UI
11. panel 幅追従の primary controls 整理
12. template placement action row の簡素化
13. `Esc` による transient template / guide 解除
14. metrics-based template alignment
15. portable FFmpeg sidecar packaging / provenance / notices
16. GitHub Release への sidecar 統合
17. Windows / macOS / Linux の最終検証

### 1.2 この計画に含めないもの

以下は `.docs/12_future_work.md` に従い future work として維持し、ここでは実装対象にしません。

- `FUT-01`: partial clear
- `FUT-02`: pen pressure
- `FUT-03`: auto taper / pseudo-pressure の高度化
- `FUT-04`: proxy media
- `FUT-05`: GPU export compositor
- `FUT-06`: optional codec-pack 取得ツール
- `FUT-07`: arbitrary effect scripting
- `FUT-08`: object 選択時の preview/canvas ハイライト

### 1.3 Future work 参照 ID

| Task ID | 項目 | 今回含めない理由 |
|---|---|---|
| FUT-01 | partial clear | `AGENTS.md` と `.docs/02_final_spec_v1.0.0.md` が v1.0 非対象として固定 |
| FUT-02 | pen pressure | v1.0 UX の前提にしないと明記済み |
| FUT-03 | auto taper / pseudo-pressure | 将来 hook は残すが、本体機能は future work |
| FUT-04 | proxy media | media 層を塞がない設計だけ確保し、本実装は v1.0 外 |
| FUT-05 | GPU export compositor | correctness 優先で CPU-safe baseline を維持するため |
| FUT-06 | optional codec-pack 取得ツール | provenance / compliance が mainline と別問題のため |
| FUT-07 | effect scripting | v1.0 は built-in effect + declarative preset に限定 |
| FUT-08 | object 選択時の preview/canvas ハイライト | 視認性と編集導線には有用だが、誤って常時強い装飾を出すと preview のデザイン可読性を壊しやすいため |

## 2. 依存関係マップ

| Task ID | タイトル | 依存 | 目的の要点 |
|---|---|---|---|
| V1-01 | preset 境界の正規化 | なし | style / entrance / clear / combo を分け、reset / inherit を扱える editor state を入れる |
| V1-02 | reveal-head effect | V1-01 | head effect を renderer / preview / export / preset に接続する |
| V1-03 | post-action chain | V1-01 | built-in post-action を renderer / UI / project に接続する |
| V1-04 | clear effect / clear preset / combo preset | V1-01 | clear kind / ordering / granularity を UI と renderer で完結させる |
| V1-05 | selection / group / z-order foundation | なし | multi-select, group, ungroup, z-order の command と UI を入れる |
| V1-06 | object outline / page events panel 強化 | V1-04, V1-05 | tree 表示、batch edit、現在生存中表示、auto-follow を揃える |
| V1-07 | template / guide advanced controls | なし | template 詳細設定を別ポップアップへ逃がし、guide の次文字字間調整を UI に露出する |
| V1-08 | side panel scroll / overflow hardening | なし | 左右ペインを縦スクロール対応にし、項目増加でも画面外へはみ出さないようにする |
| V1-09 | template font switch crash fix | なし | template 表示中の font 切替を fail-safe にしてクラッシュを止める |
| V1-10 | multiline template editor UI | なし | 改行入力を GUI から扱えるようにし、editor 高さも調整可能にする |
| V1-11 | panel-aware wide controls | なし | seek bar や template 入力欄など primary controls を panel 幅へ自然に追従させる |
| V1-12 | template placement action row simplification | V1-11 | `前スロット/次スロット` の価値を残しつつ、左パネルの最小幅を圧迫しない配置へ整理する |
| V1-13 | `Esc` cancel for transient modes | なし | template 配置 / guide を keyboard から安全に解除できるようにする |
| V1-14 | metrics-based template alignment | V1-10 | template の縦位置と小文字揃えを font metrics ベースへ寄せつつ、横幅は kerning を壊さない shaping ベースを維持する |
| PKG-01 | portable FFmpeg sidecar packaging | なし | sidecar layout, manifest, provenance, notices を出荷形にする |
| PKG-02 | GitHub Release sidecar 統合 | PKG-01 | 既存 release workflow を sidecar / notices 同梱の完成形へ引き上げる |
| QA-01 | cross-platform validation / closeout | V1-02, V1-03, V1-04, V1-06, V1-10, V1-11, V1-12, V1-13, V1-14, PKG-01, PKG-02 | 実 build / runtime / export を OS ごとに通し、docs を確定する |

## 3. 実装対象の現状スナップショット

- `reveal-head effect` は `crates/domain/src/annotations.rs` に型だけあるが、`crates/app/src/main.rs` の inspector と `crates/renderer/src/lib.rs` の描画には未接続。
- `post-action chain` も domain に型だけあり、app / renderer / preset へ未接続。
- clear event は domain と renderer に最小 primitive があるが、app 側は `全消去 -> Instant clear 挿入` のみで、`kind / duration / granularity / ordering` の編集 UI と preset 導線が無い。
- `V1-01`, `V1-05`, `V1-07`, `V1-08` は develop で完了済み。以後の未着手はそれ以外の task に限る。
- template の横幅は `egui` shaping ベースで取れているが、縦位置は `baseline_y + font_size * scale` 近辺の簡易モデルで、`ascent / descent / x-height / cap-height` を見ていない。
- template engine 自体は `\n` を解釈できるが、左ペインの入力欄は single-line なので GUI から改行を入れられない。
- seek bar や template 文字入力欄は panel 幅へ十分に追従しておらず、横幅が余っても入力領域が伸びない箇所がある。
- `前スロット / 次スロット` は slot index を手動補正する用途で残っているが、常時 4 ボタン横並びのため左パネル最小幅を押し上げている。
- template 表示中に font family を切り替えると crash する報告があり、font 適用タイミングと placed slot 再計算の境界を見直す必要がある。
- `Esc` で template 配置や guide を解除する shortcut はまだ無い。
- FFmpeg sidecar は discovery までで、release asset への bundling / provenance / notices / CI upload が未完了。
- GitHub Release workflow は app binary archive の build/upload までは済んでいるが、sidecar / notices / manifest 同梱は未完了。

## 4. 着手前に確認する既存 docs / test

既存部分を壊さないため、各 task では実装前に最低限次を読み直し、既存 test を実行する。ここで docs と test が薄いと判定した場合は、本実装へ入る前にその task の前提 docs/test を補強する。

| Task ID | 最低限読み直す `.docs` | 既存コード / test の確認点 |
|---|---|---|
| V1-01 | `.docs/02_final_spec_v1.0.0.md`, `.docs/05_project_file_format.md`, `.docs/09_portable_layout_and_cache.md` | `crates/presets_core`, `crates/portable_fs`, `crates/app` の preset 保存/復元 test |
| V1-02 | `.docs/02_final_spec_v1.0.0.md`, `.docs/04_architecture.md` | `crates/renderer` の entrance / clear test、`crates/app` の style/entrance restore test |
| V1-03 | `.docs/02_final_spec_v1.0.0.md`, `.docs/04_architecture.md` | `crates/domain` の group/order test、`crates/renderer` の reveal sequence test |
| V1-04 | `.docs/02_final_spec_v1.0.0.md`, `.docs/03_ui_window_model.md` | `crates/domain` の clear semantics test、`crates/renderer` の clear test、`crates/app` の page event UI test |
| V1-05 | `.docs/02_final_spec_v1.0.0.md`, `.docs/03_ui_window_model.md` | `crates/domain/src/project_commands.rs` と `crates/app` の selection/undo 系 test |
| V1-06 | `.docs/02_final_spec_v1.0.0.md`, `.docs/03_ui_window_model.md` | bottom panel / object list / page event の UI test、run 導出 helper の test |
| V1-07 | `.docs/02_final_spec_v1.0.0.md`, `.docs/03_ui_window_model.md` | `crates/template_layout` と `crates/app` の template save/restore / guide geometry test |
| V1-08 | `.docs/03_ui_window_model.md`, `.docs/10_testing_and_done_criteria.md` | `crates/app/src/main.rs` の bottom panel / layout 系 test、panel 幅と canvas 安定性の test |
| V1-09 | `.docs/03_ui_window_model.md`, `.docs/13_risk_register.md` | `crates/app/src/main.rs` の template placement / font restore test、font reload 周りの helper |
| V1-10 | `.docs/03_ui_window_model.md`, `.docs/05_project_file_format.md` | template save/restore test、left panel UI test、editor UI state 保存の有無 |
| V1-11 | `.docs/03_ui_window_model.md` | transport bar / left panel layout test、`available_width` 前提の UI helper |
| V1-12 | `.docs/03_ui_window_model.md` | template slot stepper test、template placement UI test |
| V1-13 | `.docs/03_ui_window_model.md`, `.docs/02_final_spec_v1.0.0.md` | keyboard shortcut 処理、guide/template の transient state test |
| V1-14 | `.docs/02_final_spec_v1.0.0.md`, `.docs/04_architecture.md` | `crates/template_layout`, `crates/fonts`, `crates/app` の template slot / shaping / save-restore test |
| PKG-01 | `.docs/07_media_runtime_and_ffmpeg.md`, `.docs/13_risk_register.md` | `crates/media` の runtime discovery test、`scripts/package_release_asset.py` の archive test |
| PKG-02 | `.docs/07_media_runtime_and_ffmpeg.md`, `.docs/10_testing_and_done_criteria.md` | workflow YAML parse、packager script の dry-run |
| QA-01 | `.docs/10_testing_and_done_criteria.md`, `.docs/13_risk_register.md` | 現在の workspace test、host export smoke、既知制約一覧 |

## 5. Task 詳細

### V1-01: preset 境界の正規化と field-level reset / inherit

**優先度:** P0
**ひとことで言うと:** preset を「なんでも一緒盛り」から卒業させ、あとから clear や combo を足しても壊れない土台にする task。
**目的:** spec の preset category と reset semantics を実装可能な形へ整える。以後の effect / clear / combo 実装が後戻りしないよう、editor state と catalog 構造をここで固定する。

**具体的に困る場面**

- 利用者が「線の太さ preset はそのまま使いたいが、出現速度だけ project ごとに変えたい」と思っても、現状は style preset と entrance が混ざっているため、意図しない項目まで一緒に上書きされやすい。
- 開発者が clear preset や combo preset を足そうとすると、現行の style preset schema に横から項目を増やすしかなく、後から schema を分けると loader と保存互換をまとめてやり直すことになる。

**現状の問題**

- `crates/presets_core/src/lib.rs` の `BaseStylePreset` に entrance が混在しており、`style / entrance / clear / combo` が分離されていない。
- editor UI は effective snapshot を直接編集しているため、「preset から継承」「override」「preset 値に戻す」の 3 状態を表現できない。
- project 保存は resolved snapshot を持っているが、field-level binding state を持っていない。

**設計**

- `presets_core` に次の catalog を追加する。
  - `StylePreset`
  - `EntrancePreset`
  - `ClearPreset`
  - `ComboPreset`
- `ComboPreset` は値そのものを重複保持せず、`style / entrance / clear` への参照束と optional override だけを持つ。
- app 側には renderer/domain へ漏らさない editor 専用状態 `PresetBound<T>` を導入する。
  - `BindingMode::Inherited`
  - `BindingMode::Overridden`
  - `effective_value()`
  - `reset_to_preset()`
- project save は引き続き resolved snapshot + optional preset ID を canonical に保存し、editor の binding mode は `project.settings.pauseink_editor_ui` 側へ持つ。
- 既存の user style preset directory は維持しつつ、portable root 配下へ以下を追加する。
  - `pauseink_data/config/style_presets/`
  - `pauseink_data/config/entrance_presets/`
  - `pauseink_data/config/clear_presets/`
  - `pauseink_data/config/combo_presets/`
- 既存 style preset に埋まっている entrance 値は lenient load で `legacy_style_preset.entrance -> entrance preset candidate` として救済し、normalized save は新 schema へ寄せる。

**着手前に決めるべきこと**

- binding の粒度を field 単位にするか section 単位にするか。ここを後から変えると UI state と project save の互換を崩す。
- binding metadata を `project.settings.pauseink_editor_ui` へ保存するか、別キーへ逃がすか。後からキーを動かすと recovery / reopen 互換が面倒になる。
- legacy style preset の entrance 救済を「自動移行する」「読み込みだけ救済する」のどちらにするか。保存ポリシーが変わるため最初に固定する。
- combo preset の優先順位を `combo override > category preset > resolved snapshot` のどこに置くか。後から precedence を変えると project 再現性が崩れる。

**変更ファイル**

- Modify: `crates/presets_core/src/lib.rs`
- Modify: `crates/app/src/main.rs`
- Modify: `crates/app/src/lib.rs`
- Modify: `crates/portable_fs/src/lib.rs`
- Modify: `manual/user_guide.md`
- Modify: `manual/developer_guide.md`

**実装ステップ**

1. `presets_core` に 4 category の schema と loader/save helper を追加する。
2. legacy style preset の entrance 混在 schema を読み取る lenient migration path を入れる。
3. app の active editor state を `effective snapshot + binding metadata` へ置き換える。
4. inspector に `継承 / 上書き / リセット` の UI affordance を追加する。
5. user preset CRUD を category ごとに分離する。
6. project reopen / app relaunch / autosave recovery で binding state が復元されるようにする。

**必要テスト**

- `presets_core`: legacy style preset の entrance 混在を新 schema へ正規化できる
- `presets_core`: combo preset が style/entrance/clear を正しく合成する
- `app`: inherited field を reset すると preset 値へ戻る
- `app`: overridden field は project save/reopen で保持される
- `portable_fs`: 新しい preset directory 群が portable root 配下に作られる

**完了条件**

- style / entrance / clear / combo が別 catalog として読める
- UI から field 単位に inherit / override / reset ができる
- 既存 project と既存 style preset を壊さない

### V1-02: reveal-head effect

**優先度:** P0
**依存:** V1-01
**ひとことで言うと:** なぞり書きの「今どこまで進んだか」を見やすくする先頭ハイライト演出を入れる task。

**目的:** spec 5.3 の `none / solid / glow / comet-tail` を preview/export で正しく見せ、preset と project 保存に接続する。

**具体的に困る場面**

- 利用者が path trace の書き順を見せたくて head effect を想定しても、今は線の先頭が視認しづらく、動画で「どこまで書けたか」が分かりにくい。
- 開発者目線では、head effect を static style として入れてしまうと、outline / glow / clear との前後関係を後から全部やり直すことになる。

**現状の問題**

- domain には `RevealHeadEffect` があるが、renderer は path front に head を描いていない。
- inspector に head effect editor が無い。
- preset / project restore / export smoke が head effect を検証していない。

**設計**

- head effect は static style ではなく entrance 評価の副生成物として扱う。
- renderer で timed entrance を評価したあと、`visible path front` と `progress` から head overlay を計算する。
- `solid` は塗りつぶし円/楕円、`glow` は blur 付き halo、`comet_tail` は進行方向へ減衰する trail を描く。
- `color_source` は `stroke_color`, `preset_accent`, `custom` の 3 系統で解決する。
- `persistence` は reveal 完了後の残留時間として扱い、preview/export 双方で同じ式を使う。

**着手前に決めるべきこと**

- head の形状を「path front の点」「path front の短い接線付き trail」のどこまで持つか。後から形状モデルを変えると preset parameter が増減する。
- `Instant` entrance で head effect を完全無効にするか、短時間だけ出すか。仕様差が preview/export の両方へ波及する。
- `color_source` の優先順位を `custom > preset accent > stroke color` のように固定すること。ここが曖昧だと preset 再現性が崩れる。
- head の描画順を `drop shadow / glow / outline / base / head` のどこに置くか。後から変えると見た目回帰が大きい。

**変更ファイル**

- Modify: `crates/renderer/src/lib.rs`
- Modify: `crates/app/src/main.rs`
- Modify: `crates/app/src/lib.rs`
- Modify: `crates/presets_core/src/lib.rs`
- Modify: `manual/user_guide.md`

**実装ステップ**

1. renderer に `HeadRenderState` を追加し、entrance progress から head geometry を計算する。
2. `solid / glow / comet_tail` の CPU-safe 描画 pass を追加する。
3. inspector に kind / color source / size / blur / tail length / persistence / blend mode を追加する。
4. active entrance と current object sync を head effect まで拡張する。
5. entrance preset / combo preset へ head effect を載せる。
6. export smoke で head effect が preview と矛盾しないことを確認する。

**必要テスト**

- `renderer`: `PathTrace + glow head` が先頭に追従する
- `renderer`: persistence が reveal 完了後も一定時間残る
- `renderer`: `Instant` では head effect が出ない、または spec どおり no-op になる
- `app`: head effect の設定が save/reopen と settings relaunch で戻る

**完了条件**

- inspector から head effect を編集できる
- preview / export の見え方が一致する
- preset と project save に head effect が含まれる

### V1-03: post-action chain

**優先度:** P0
**依存:** V1-01
**ひとことで言うと:** 書き終わった後の点滅、発光、色変化など「後からかかる演出」を入れる task。

**目的:** `during reveal / after stroke / after glyph object / after group / after run` に対する built-in post-action を renderer と UI へ接続する。

**具体的に困る場面**

- 利用者が「書き終わった文字だけ少し発光させたい」「trace が終わった後に色を変えたい」と思っても、現状は reveal 以後の振る舞いを付けられない。
- 開発者が group 単位の演出を入れようとしても、`after group` の定義が曖昧なまま進めると renderer と UI の時間軸が食い違う。

**現状の問題**

- domain の `PostAction` 型が死蔵されている。
- renderer は reveal 後の style change / pulse / blink を全く評価していない。
- inspector と project save/reopen に post-action 編集 UI が無い。

**設計**

- post-action は `effective style timeline` を返す evaluator として renderer 側に追加する。
- evaluator の入力は `object`, `group`, `run`, `time`, `reveal_progress`。
- v1.0 では action を次に限定する。
  - `NoOp`
  - `StyleChange`
  - `InterpolatedStyleChange`
  - `Pulse`
  - `Blink`
- `timing_scope` は `during reveal / after stroke / after glyph object / after group / after run` を実装する。`after group/run` は group/run の完了時刻導出が必要なので、その evaluator を renderer に追加する。
- UI は複雑な graph editor にせず、`追加 / 削除 / 上下移動` の配列 editor とする。

**着手前に決めるべきこと**

- v1.0 で許可する post-action の最終集合。ここを広げすぎると evaluator と UI が一気に重くなる。
- action が重なったときに「合成する」「後勝ち」「同種のみ合成」のどれにするか。後から変えると保存データの意味が変わる。
- `after group` / `after run` の完了時刻を最後の timed entrance 完了で決めるか、instant object も含めるか。時間軸の根本なので最初に固定する。
- post-action が visible style だけを変えるのか、object の persistent style snapshot まで変えたように扱うのか。export/reopen の整合に影響する。

**変更ファイル**

- Modify: `crates/renderer/src/lib.rs`
- Modify: `crates/app/src/main.rs`
- Modify: `crates/app/src/lib.rs`
- Modify: `crates/presets_core/src/lib.rs`
- Modify: `manual/user_guide.md`
- Modify: `manual/developer_guide.md`

**実装ステップ**

1. renderer に `evaluate_post_actions` を追加する。
2. group/run 完了時刻の導出 helper を renderer 内へ追加する。
3. pulse / blink / interpolated style change を base style へ重ねる式を固定する。
4. inspector に配列 editor を追加する。
5. entrance preset / combo preset に post-action を保存できるようにする。
6. export / preview 回帰を追加する。

**必要テスト**

- `renderer`: `after glyph object` の style change が reveal 完了後に効く
- `renderer`: `after group` が group 内最後の timed object 完了後にだけ発火する
- `renderer`: `pulse` と `blink` が clear 境界を越えない
- `app`: post-action chain の add/remove/reorder が save/reopen で保持される

**完了条件**

- post-action chain を UI から編集できる
- preview / export / save/load が一貫する
- group / run scope の timing が test で固定されている

### V1-04: clear effect の実装完了と clear / combo preset UI

**優先度:** P0
**依存:** V1-01
**ひとことで言うと:** `全消去` を「ただ消すだけ」から、順番付きや dissolve 付きで編集できる機能へ広げる task。

**目的:** clear event を `Instant / Ordered / ReverseOrdered / WipeOut / DissolveOut` まで UI と renderer で完結させ、clear preset と combo preset へ接続する。

**具体的に困る場面**

- 利用者が「このページだけ dissolve で消したい」「書いた順に消したい」と思っても、現状は `全消去` が instant clear 固定で、挿入後に直せない。
- clear event を誤った時刻に打った場合も、page events 側で編集や複製ができないため、何度も打ち直しが必要になる。
- 開発者視点では、ordering key を `capture_order` にするか `reveal_order` にするか曖昧なまま実装すると、後から clear 演出が全部ひっくり返る。

**現状の問題**

- app の `全消去` は `Instant` 挿入しかできない。
- renderer の clear は `Ordered / ReverseOrdered / granularity` をほぼ使っていない。
- page events tab は flat list だけで、select/edit/delete/duplicate が無い。
- clear preset / combo preset の catalog と GUI が無い。

**設計**

- clear event の編集主体は下部 `ページイベント` タブに置く。
- transport 近辺には `全消去` の quick action を残しつつ、その時点の `選択中 clear preset` を使って insert する。
- renderer は clear evaluator を `scope-aware serial scheduler` にする。
  - `granularity`: all parallel / group / glyph / stroke
  - `ordering`: serial / reverse / parallel
- combo preset は `style + entrance + clear` の束として挿入・適用できるようにする。
- clear preset の resolved snapshot は project に保存し、preset file の変更で過去 project が崩れないようにする。

**着手前に決めるべきこと**

- ordered clear の順序キーを `capture_order`, `reveal_order`, `z-order` のどれにするか。後から変えると export 見た目が変わる。
- `granularity=group` のとき loose stroke をどう扱うか。group model の意味に直結する。
- quick clear が「現在選択中 clear preset」を使うのか、「最後に使った clear 設定」を使うのか。UI 期待値が変わる。
- combo preset が clear preset を参照保持するか resolved 値を抱えるか。project 再現性と loader 複雑度に効く。

**変更ファイル**

- Modify: `crates/renderer/src/lib.rs`
- Modify: `crates/app/src/main.rs`
- Modify: `crates/app/src/lib.rs`
- Modify: `crates/presets_core/src/lib.rs`
- Modify: `manual/user_guide.md`
- Modify: `README.md`

**実装ステップ**

1. renderer の clear evaluator を `granularity + ordering` 対応へ拡張する。
2. page events tab に一覧選択、詳細編集、削除、duplicate を追加する。
3. quick clear button を `current clear preset` ベースにする。
4. clear preset / combo preset の built-in + user catalog と CRUD UI を追加する。
5. project save に resolved clear snapshot と optional combo preset binding を追加する。
6. export smoke で wipe/dissolve/ordered clear を確認する。

**必要テスト**

- `renderer`: ordered clear が capture/reveal order に従って段階的に消える
- `renderer`: reverse clear が逆順に消える
- `renderer`: granularity=`group` が group を unit として消す
- `app`: clear event editor の変更が save/reopen で保持される
- `presets_core`: combo preset が clear preset を正しく解決する

**完了条件**

- clear kind / duration / granularity / ordering を GUI から編集できる
- quick clear と page events track が同じ preset を共有する
- clear/combo preset を built-in / user ともに扱える

### V1-05: selection / multi-select / group / ungroup / z-order foundation

**優先度:** P0
**依存:** なし
**ひとことで言うと:** 複数選択、グループ化、前面/背面のような「編集の土台」を作る task。

**目的:** spec 9 の編集導線を成立させる。現在の単一 `selected_object_id` から multi-select と command 群へ拡張する。

**具体的に困る場面**

- 利用者が 3 文字まとめて色を変えたい、前面へ出したい、group 化して一緒に扱いたい、と思っても今は 1 object ずつしか触れない。
- object outline を tree 化しても、selection の source of truth が単一選択のままだと batch edit と undo/redo がすぐ破綻する。

**現状の問題**

- selection は単一 object しか持てない。
- `InsertGroupCommand` はあるが、ungroup / membership 更新 / batch z-order command が足りない。
- batch style edit の適用先モデルがない。

**設計**

- app session に `SelectionState` を追加する。
  - selected glyph object IDs
  - selected group IDs
  - active focus ID
- command 層へ次を追加する。
  - `InsertGroupCommand` の app 接続
  - `RemoveGroupCommand`
  - `UpdateGroupMembershipCommand`
  - `BatchSetGlyphObjectStyleCommand`
  - `BatchSetGlyphObjectEntranceCommand`
- `NormalizeZOrderCommand`
- renderer 側には selection の概念を持ち込まない。
- UI 操作はまず outline panel 起点にし、canvas direct-manipulation は後回しにする。
- v1.0 では group の入れ子を禁止する。
- z-order 操作は「選択中 object の相対順を保ったまま前後へ動かす」を基本とする。

**着手前に決めるべきこと**

- selection の source of truth は app session に一本化する。
- group の入れ子は v1.0 で禁止する。
- z-order は selected objects を前後移動し、必要に応じて全体正規化できる方針で進める。
- canvas 直接選択は今回 scope に入れず、outline 起点に絞る。

**変更ファイル**

- Modify: `crates/domain/src/project_commands.rs`
- Modify: `crates/app/src/lib.rs`
- Modify: `crates/app/src/main.rs`
- Modify: `manual/user_guide.md`

**実装ステップ**

1. `SelectionState` と command 群を追加する。
2. outline panel から複数選択できる土台を入れる。
3. `グループ化`, `グループ解除`, `前面へ`, `背面へ`, `一つ前`, `一つ後ろ` を app action として実装する。
4. batch style / entrance edit が selection 全体へ効くようにする。
5. undo/redo と grouped command で巻き戻せるようにする。

**必要テスト**

- `domain`: group/un-group が history で往復する
- `domain`: z-order normalize が capture/reveal order を壊さない
- `app`: multi-select へ style を適用すると全 object に反映される
- `app`: group 化後の undo/redo で selection が壊れない

**完了条件**

- 複数選択、group/ungroup、z-order 操作が GUI から可能
- undo/redo と project save/load で崩れない

### V1-06: object outline / page events panel の強化

**優先度:** P1
**依存:** V1-04, V1-05
**ひとことで言うと:** 下の一覧パネルを「ただの文字列」から、探しやすく編集しやすい実用品へする task。

**目的:** spec 10 の panel 要件へ近づける。flat list を tree / batch-edit 可能な panel に引き上げる。

**具体的に困る場面**

- 利用者が object 数の多い project を開くと、今の flat list では「どの stroke がどの group / run に属しているか」「今どの object が生きているか」がほぼ追えない。
- clear event を多数打った project でも、page events が plain text だけだと時刻修正や比較がしづらい。
- 開発者も tree / run 導出ルールが曖昧なままだと、selection や auto-follow を足すたびに UI 表示が揺れる。

**現状の問題**

- `オブジェクト一覧` は `object id / stroke count / page / z` の文字列羅列だけ。
- `run / group / glyph / stroke` の tree が無い。
- `visibility / lock / solo / auto-follow-current / alive highlight` が無い。
- `ページイベント` も timeline track ではなく plain text list。

**設計**

- 下部 panel は引き続き単一 window 内に保ち、tree widget を自作せず `egui::CollapsingHeader` と small action row の組み合わせで構成する。
- outline tree の top level は `run`、その下に `group`、その下に `glyph object`、leaf に `stroke` を置く。
- run は `style/entrance/preset/page` が同じ連続 object から導出する view model とし、persistent data にはしない。
- `alive` 判定は current preview time と clear/page interval から導く。
- `lock / solo / visibility` は v1.0 では editor-only state とし、project へは保存しない。

**着手前に決めるべきこと**

- run 導出のキーを `page + preset + style + entrance` のどこまで含めるか。後から run 分割条件を変えると outline 操作感が変わる。
- `lock / solo / visibility` を editor-only 一時 state にするか、relaunch まで戻すか。保存境界を最初に固定する。
- auto-follow を再生中だけにするか、seek/selection change にも反応させるか。後から変えると UX が大きく変わる。
- page events タブを「一覧 + 詳細編集」か「簡易 timeline 風」かのどちらへ寄せるか。実装コスト差が大きい。

**変更ファイル**

- Modify: `crates/app/src/main.rs`
- Modify: `crates/app/src/lib.rs`
- Modify: `manual/user_guide.md`
- Modify: `manual/developer_guide.md`

**実装ステップ**

1. outline tree 用の view model を app 側に追加する。
2. run/group/glyph/stroke の階層表示を実装する。
3. multi-select / batch action / auto-follow-current を接続する。
4. alive highlight と page filter を追加する。
5. page events tab を timeline track 風の表示へ近づける。

**必要テスト**

- `app`: run 導出が group と混同しない
- `app`: current time 更新で alive highlight が切り替わる
- `app`: auto-follow が current object を view 内へ保つ
- `app`: visibility / solo / lock が editor preview にだけ効き、export へ漏れない

**完了条件**

- outline panel が tree で使える
- page events tab から clear を編集しやすい
- batch edit と auto-follow が spec に沿う

### V1-07: template / guide advanced controls

**優先度:** P1
**依存:** なし
**ひとことで言うと:** 左ペインで詰まり気味の template / guide 設定を整理し、実用上ほしい細かな調整を足す task。

**目的:** spec 4.3 / 7 の template settings を別ポップアップで編集可能にし、guide の次文字字間調整を v1.0 範囲で入れる。

**具体的に困る場面**

- 利用者がかな/英字/句読点の混在したテンプレートを書いても、現状は advanced controls が無いため「英字だけ少し小さく」「行間を狭く」といった実運用上の調整ができない。
- guide を使って連続で文字を書くとき、文字の右端と次の縦線セットが近すぎたり遠すぎたりしても、今は gap を調整できない。たとえば横に寝た字形や払いの長い字の直後で、次のマスを少し右へ逃がしたい場面に対応できない。
- 開発者が guide gap を後付けすると、`cell_width` 由来の固定幅・`next_cell_origin_x` の送り方・project/settings 保存のどこに属する値かが後からずれやすい。

**現状の問題**

- `line_height / kana_scale / latin_scale / punctuation_scale / underlay_mode` は内部 state にあるが UI に出ていない。
- guide の次文字送りは直前文字全体の union bounds までは実装済みだが、その右端からさらに空ける `gap` を持っていない。

**設計**

- template 詳細設定は左ペインに詰め込まず、`テンプレート詳細` ポップアップへ逃がす。
- ポップアップで変更した値は即時に preview / placed slot へ反映する。
- guide 設定は左ペインに残し、`ガイド傾き` の直下に `次文字字間` スライダーを置く。
- guide gap は既存 guide overlay にも即時反映し、スライダーを動かした瞬間に縦線位置が更新されるようにする。
- guide gap は `guide_next_gap_ratio: f32` として持ち、`next_cell_origin_x = previous_character_max_x + cell_width * guide_next_gap_ratio` の形で適用する。縦線セット幅そのものは変えない。
- gap の単位は `cell_width` 比で固定する。`0.0` が「右端ぴったり」、`0.25` が「1 マス幅の 25% だけ右へ空ける」、`-0.20` が「20% だけ食い込ませる」を意味する。
- gap は負値を許可する。ただし fallback advance の歩幅が 0 以下にならないよう、UI 範囲は `-0.50..=1.50` を既定とし、`次送りのみの連打` では `cell_width * (1.0 + guide_next_gap_ratio)` を 1 ステップとして使う。
- gap の見た目は slope と independent に保存し、傾けても「文字間の横方向余白」を調整するための値として扱う。
- 保存先は guide slope と同じ経路に揃える。つまり live 値は `Settings` に持ち、project save/reopen と app relaunch の復元は `ProjectEditorUiState` を介して `project.settings.pauseink_editor_ui` と `settings.editor_ui_state` の両方へ流す。

**着手前に決めるべきこと**

- mixed-script line height の基準を font size 基準に固定するか、最大 glyph height 基準にするか。slot 生成式の根本になる。
- template 詳細ポップアップをモーダルにするか、非モーダルの小ウィンドウにするか。操作中に canvas を見ながら微調整できるかが変わる。
- guide gap の UI 表現を slider only にするか、slider + numeric 表示にするか。値の discoverability と細かい調整性に影響する。

**変更ファイル**

- Modify: `crates/template_layout/src/lib.rs`
- Modify: `crates/app/src/main.rs`
- Modify: `crates/app/src/lib.rs`
- Modify: `crates/portable_fs/src/lib.rs`
- Modify: `manual/user_guide.md`
- Modify: `manual/developer_guide.md`

**実装ステップ**

1. template 詳細を開くポップアップと、その中の advanced controls を追加する。
2. `UnderlayMode` の dropdown と script scale / line height controls をポップアップへ移す。
3. `Settings` と `ProjectEditorUiState` に `guide_next_gap_ratio` を追加し、guide slope と同じ保存経路へ乗せる。
4. 左ペイン guide section に `次文字字間` スライダーを追加し、guide geometry 生成式へ接続する。
5. `next_cell_origin_x` の更新式と fallback advance を `guide_next_gap_ratio` 対応に直す。
6. template 詳細ポップアップの変更が placed slot と preview に即時反映されることを確認する。
7. guide gap が save/reopen と app relaunch の両方で戻ることを確認する。

**必要テスト**

- `template_layout`: script scale / line height / underlay mode の UI 値が slot 計算へ反映される
- `app` / `guide`: gap を変えると次文字縦線の開始 x だけが `cell_width * ratio` 分だけ変わり、縦線セット幅は変わらない
- `app` / `guide`: 負の gap でも次文字送りが動作し、fallback advance が 0 以下にならない
- `app`: save/reopen で advanced template settings が戻る
- `app`: save/reopen と settings relaunch の両方で guide gap が guide slope と同じ経路で戻る
- `app`: template 詳細ポップアップの値変更が preview / placed slot に即時反映される

**完了条件**

- template advanced settings が GUI から編集できる
- guide の次文字字間が GUI から編集できる
- template 詳細と guide gap がリアルタイムで反映される

### V1-08: side panel scroll / overflow hardening

**優先度:** P1
**依存:** なし
**ひとことで言うと:** 左右ペインの中身が増えても画面外へ逃げず、低い画面でも操作不能にならないようにする task。

**目的:** 左右ペインの縦方向 overflow を吸収し、今後 UI 項目が増えても main canvas と下部 panel のレイアウトを安定させる。

**具体的に困る場面**

- 左ペインや右ペインに設定項目が増えると、下の項目が画面外へ押し出されて触れなくなる。
- 画面高さが低い環境や高 DPI 環境では、effect や export の controls に辿り着くためにウィンドウ自体を大きくし続ける必要がある。
- これから `V1-05` や `V1-07` で項目が増えると、縦方向 overflow が実害に変わりやすい。

**現状の問題**

- 左右ペインは `egui::Panel::left/right` の中へそのまま controls を積んでおり、縦スクロール領域で包んでいない。
- 下部 panel は固定高さ + scroll だが、左右ペインには同等の overflow 保護がない。

**設計**

- 左右ペインとも `固定ヘッダ + ScrollArea body` に分ける。
- スクロール対象は panel 全体ではなく body 部分に限定し、width resize 挙動は現状維持する。
- runtime status / title / preset 選択など上部で頻繁に触る要素は固定側へ残し、長い controls 群だけを scroll 側へ送る。
- 既存の export panel や style editor の責務分割は触らず、まずはレイアウト層で overflow を解消する。

**着手前に決めるべきこと**

- 左右ペインをそれぞれ 1 本の大きい scroll にするか、section ごとに個別 scroll にするか。後から変えると操作感がかなり変わる。
- ヘッダ行に何を固定し、何を scroll 側へ送るか。頻繁に触る操作の見え方に影響する。
- スクロール導入後も中央 canvas と下部 panel の高さを絶対に揺らさない前提でいくか。レイアウト調整方針の根本になる。

**変更ファイル**

- Modify: `crates/app/src/main.rs`
- Modify: `manual/user_guide.md`
- Modify: `manual/developer_guide.md`

**実装ステップ**

1. 左ペインを `固定ヘッダ + ScrollArea body` に分ける。
2. 右ペインを `固定ヘッダ + ScrollArea body` に分ける。
3. 既存の width resize と bottom panel 安定性を壊さないことを確認する。
4. 項目数が増えても canvas 高さが揺れないことを確認する。

**必要テスト**

- `app`: 左ペイン overflow 時に body だけが scroll し、panel width resize は維持される
- `app`: 右ペイン overflow 時に export panel まで到達できる
- `app`: 左右ペインへ縦 scroll を入れても中央 canvas と下部 panel の高さが不必要に揺れない

**完了条件**

- 左右ペインが縦スクロール対応する
- 低い画面でも主要 controls へ到達できる
- 既存の width resize と bottom panel の安定性を壊さない

### V1-09: template font switch crash fix

**優先度:** P0
**依存:** なし
**ひとことで言うと:** template 表示中の font family 切替を fail-safe にして、配置済み slot があっても落ちないようにする task。

**目的:** reported crash を止める。font family の変更を「即値書き換え」ではなく、検証済みの apply pipeline に通してから preview / placed slot へ反映する。

**具体的に困る場面**

- 利用者が template を表示したまま font を変えると app が落ち、保存前の作業を失う。
- 開発者が font 適用タイミングを曖昧なまま修正すると、今度は crash は止まっても「font dropdown だけ変わって実際の slot は旧 font のまま」になる。

**現状の問題**

- 左ペインの font dropdown 変更時に `font_family` を即時更新し、その場で `maybe_apply_egui_fonts()` と layout refresh を混在させている。
- `template_font_choices` は未発見の選択中 family も choices に残す一方、`template_font_id()` は `システム既定` 以外を常に `FontFamily::Name` で引くため、未 bind family を preview/reflow が踏む panic 経路がある。
- placed slot の再計算は `slot_object_ids.resize(...)` に留まるため、font 切替で slot 並びが変わった場合の index ずれも検討が必要。

**設計**

- `requested_template_font_family` と `applied_template_font_family` を分けず、既存 state は維持しつつ「apply 前 validation」を 1 箇所へ寄せる。
- font 変更時は即座に `self.template.font_family` を採用せず、次の順で処理する。
  1. 候補 family が local / Google cache / system から解決できるか検証する
  2. 解決できる場合だけ `font_config_dirty = true`
  3. `maybe_apply_egui_fonts()` を通して font registry を更新する
  4. 更新成功後に placed slot reflow と preview invalidation を行う
- 解決できない場合は previous family を維持し、log に理由を出す。
- placed slot がある状態の font 切替は `reset_template_slots()` ではなく `refresh_placed_template_slots()` による reflow を基本とし、slot index と origin は保持する。

**着手前に決めるべきこと**

- font 適用失敗時に「前の font を保持」するか「システム既定へ戻す」か。UX が変わる。
- `maybe_apply_egui_fonts()` の失敗を recoverable log に留めるか、UI toast 的な明示エラーにするか。通知量が変わる。
- font 変更中に template details popup が開いている場合も同じ apply pipeline に統一するか。入口が複数あると再発しやすい。

**変更ファイル**

- Modify: `crates/app/src/main.rs`
- Modify: `crates/fonts/src/lib.rs` （必要なら validation helper を追加）
- Modify: `manual/user_guide.md`

**実装ステップ**

1. template font 変更の apply helper を 1 箇所へ集約する。
2. candidate family の解決可否と fallback を明示化する。
3. placed slot がある状態の reflow を安全に通す。
4. font 切替失敗時の log を追加する。

**必要テスト**

- `app`: placed slot がある状態で font family を切り替えても panic / crash しない
- `app`: 無効な family を選んだとき previous family を維持する
- `app`: font family 切替後も slot origin と current slot index が保持される

**完了条件**

- template 表示中の font 切替で落ちない
- 正常系では preview と placed slot が新 font で reflow する
- 失敗系では安全に前状態へ留まる

### V1-10: multiline template editor UI

**優先度:** P1
**依存:** なし
**ひとことで言うと:** template text を GUI から改行付きで入力できるようにし、入力欄の高さもユーザが調整できるようにする task。

**目的:** engine 側にある `\n` 対応を GUI から使えるようにする。2 行初期表示と resizable editor を用意し、template details の `行間` が実際の multiline template で意味を持つ状態へ持っていく。

**具体的に困る場面**

- 利用者が 2 行以上の template を置きたくても、今は single-line input のため GUI から改行を入れられない。
- 左ペインが狭い環境では、長い template を編集すると全文が見えず、誤字修正や改行位置の調整がしづらい。

**現状の問題**

- template text input は `text_edit_singleline` 固定。
- editor の高さは固定で、複数行の確認に向かない。

**設計**

- 左ペインの template text input を `TextEdit::multiline` へ置き換える。
- 初期表示は 2 行分の高さとし、右下ドラッグで高さを変えられる `egui::Resize` コンテナで包む。
- 横幅は panel の available width に追従させるが、ボタン列や見出しの余白までは塗りつぶさない。
- editor height は project ではなく app/editor UI state として保存する。template 本文そのものは既存どおり project に保存する。
- line break の正規化は既存 project save の canonical 形式へ従い、`\r\n` 読み込みは lenient のまま、save は `\n` に寄せる。

**着手前に決めるべきこと**

- editor height を app relaunch まで戻すか、project ごとに持つか。UI state の保存先が変わる。
- multiline editor を常時表示にするか、折りたたみ可能にするか。左ペイン密度に影響する。
- Enter キーの扱いを「そのまま改行」にするか、特殊 shortcut と衝突させないか。操作感が変わる。

**変更ファイル**

- Modify: `crates/app/src/main.rs`
- Modify: `crates/app/src/lib.rs`
- Modify: `manual/user_guide.md`
- Modify: `manual/developer_guide.md`

**実装ステップ**

1. template text input を multiline 化する。
2. `egui::Resize` で高さ可変にする。
3. editor height を UI state へ保存し、relaunch で戻す。
4. 改行が placed slot reflow に即時反映されることを確認する。

**必要テスト**

- `app`: multiline template text が save/reopen で保持される
- `app`: `\n` を含む template 編集が即時 reflow される
- `app`: editor height state が relaunch で戻る

**完了条件**

- GUI から改行入り template を編集できる
- 初期高さは 2 行分で、右下ドラッグで高さ変更できる
- 改行と行間設定が preview / placed slot に反映される

### V1-11: panel-aware wide controls

**優先度:** P1
**依存:** なし
**ひとことで言うと:** seek bar や template 入力欄など、広い方が使いやすい primary controls だけを panel 幅へ自然に追従させる task。

**目的:** 利用者が panel 幅を広げたとき、入力や transport の主要部品もそれに見合って広く使えるようにする。一方で、余白やボタンまで不必要に引き伸ばさない。

**具体的に困る場面**

- シークバーが短いままで、panel を広げても seek 精度が上がらない。
- template 入力欄が短いままだと、長い文字列や改行入り text の編集性が上がらない。

**現状の問題**

- seek bar 自体は `available_width()` を見る実装が入っているが、container 側の割付と他の text field の扱いが統一されていない。
- どの widget を stretch し、どの widget を fixed にするかのルールがまだ無い。

**設計**

- 対象を「連続量を編集する primary control」に限定する。
  - seek bar
  - template text editor
  - 必要なら preset 名や ID の長文 text field
- stretch 方針は `available_width() - reserved_inline_width` の残りだけを使う。
- ボタン列、短い numeric field、status badge は fixed のまま維持する。
- 実装は共通 helper を 1 つ置き、場当たり的な `add_sized` の乱立を避ける。

**着手前に決めるべきこと**

- wide control の対象集合をどこまで広げるか。広げすぎると UI が間延びする。
- seek bar の最小幅をいくつに固定するか。狭い画面での折り返し挙動が変わる。
- multiline template editor と同時にやる前提か。layout 調整が重複しやすい。

**変更ファイル**

- Modify: `crates/app/src/main.rs`
- Modify: `manual/user_guide.md`

**実装ステップ**

1. wide control 用 helper を追加する。
2. transport bar の seek slider 割付を監査し、必要なら container 側も含めて調整する。
3. template text editor を panel 幅追従にする。
4. 余白やボタン列が不自然に伸びないことを確認する。

**必要テスト**

- `app`: panel 幅が広いと seek bar の usable width が伸びる
- `app`: template text editor が panel 幅へ追従する
- `app`: narrow width でも主要ボタン行が崩れない

**完了条件**

- seek bar と template input が panel 幅に応じて広がる
- ボタンや余白は必要以上に広がらない
- 狭い幅でも UI が壊れない

### V1-12: template placement action row simplification

**優先度:** P1
**依存:** V1-11
**ひとことで言うと:** `前スロット/次スロット` の機能は残しつつ、常時 4 ボタン横並びをやめて左パネルの最低幅を下げる task。

**目的:** `前スロット/次スロット` の価値を整理し、必要なら secondary action へ格下げする。template placement の主操作を分かりやすくしつつ、panel 幅を圧迫しないようにする。

**具体的に困る場面**

- 左パネルが狭いと `テンプレート配置 / 前スロット / 次スロット / テンプレート解除` の 4 連ボタンが最小幅を押し上げる。
- 利用者から見ると `前スロット/次スロット` の用途が見えにくく、「自動で次へ進むのに、なぜ必要なのか」が分かりづらい。

**現状の問題**

- `前スロット/次スロット` は placement 補正のための secondary action なのに、常時 primary row に置かれている。
- slot navigation の価値説明が UI 上に無い。

**設計**

- capability 自体は残す。用途は次の 3 つに限定して説明する。
  - 自動 advance を戻して書き直す
  - 1 slot 飛ばして次へ進む
  - 非連続な位置へ手動補正する
- 左ペインの primary row は `テンプレート配置` と `テンプレート解除` を主にし、slot navigation は placement active 時だけ出す compact secondary row へ移す。
- secondary row は `◀` / `▶` の小ボタン + `3 / 8` の現在位置表示を基本にし、文言ボタンを常時置かない。
- もし実機確認で利用頻度が極端に低ければ、最終的に `テンプレート詳細` へ退避する余地を残す。

**着手前に決めるべきこと**

- slot navigation を完全削除せず残すかどうか。あとから戻すと shortcut / test をやり直す。
- compact row を常時表示するか、placement active 時だけ出すか。左パネル密度が変わる。
- keyboard shortcut を同時導入するか。説明コストと衝突が増える。

**変更ファイル**

- Modify: `crates/app/src/main.rs`
- Modify: `manual/user_guide.md`

**実装ステップ**

1. template action row を primary / secondary に分ける。
2. slot navigation を compact 表示へ移す。
3. 現在 slot index 表示を追加する。
4. panel 最小幅と wrapping の改善を確認する。

**必要テスト**

- `app`: template slot stepper が compact UI でも前後に動く
- `app`: placement inactive 時は slot navigation が出ない、または無効化される
- `app`: narrow panel でも template action row が崩れにくい

**完了条件**

- 左パネルの template action row が圧迫しにくくなる
- `前スロット/次スロット` の価値を失わず secondary action 化できる
- 現在 slot の位置が UI で分かる

### V1-13: `Esc` cancel for transient modes

**優先度:** P1
**依存:** なし
**ひとことで言うと:** template 配置や guide を `Esc` で解除できるようにし、mouse 主体の操作から安全に抜けられるようにする task。

**目的:** transient editor mode を keyboard で即座に抜けられるようにする。template placement / guide / pending overlay を `Esc` で安全に閉じる。

**具体的に困る場面**

- 利用者が template placement 中や guide 表示中に「やめたい」と思っても、今は mouse で解除ボタンまで戻る必要がある。
- shortcut 導入時に優先順位を曖昧にすると、text edit の `Esc`、popup close、template cancel が衝突する。

**現状の問題**

- `Esc` による transient mode cancel が無い。
- keyboard shortcut の優先順位表が無い。

**設計**

- `Esc` の優先順位を明示する。
  1. 開いている popup/window を閉じる
  2. template placement を解除する
  3. guide overlay / capture 待ちを解除する
  4. それ以外は no-op
- text editor に keyboard focus がある間は、その widget 側の既定挙動を優先し、global cancel は発火させない。
- template cancel は `placement_armed = false` と placed slot reset、guide cancel は overlay state と capture 関連 state の reset を 1 helper へまとめる。

**着手前に決めるべきこと**

- popup close と mode cancel のどちらを優先するか。UI 期待値が変わる。
- `Esc` を text input focus 中に奪うかどうか。編集体験が変わる。
- guide cancel で `last_committed_object_bounds` まで消すか。次の guide 生成位置に影響する。

**変更ファイル**

- Modify: `crates/app/src/main.rs`
- Modify: `manual/user_guide.md`

**実装ステップ**

1. global `Esc` handler を 1 箇所へ集約する。
2. template cancel helper と guide cancel helper を分ける。
3. popup open/focus との優先順位を固定する。
4. undo/redo や guide capture と干渉しないことを確認する。

**必要テスト**

- `app`: template placement 中の `Esc` で slot と armed state が解除される
- `app`: guide 表示中の `Esc` で overlay と capture state が解除される
- `app`: text editor focus 中は global `Esc` cancel が発火しない

**完了条件**

- `Esc` で template placement と guide を解除できる
- popup / text edit / global cancel の優先順位が一貫する
- guide 再生成位置が壊れない

### V1-14: metrics-based template alignment

**優先度:** P1
**依存:** V1-10
**ひとことで言うと:** template の縦位置を font metrics ベースへ寄せ、小さくした英字や句読点の揃い方を改善する task。

**目的:** `x` と `y`、小さめ英字、句読点などの縦揃えを改善する。ただし横幅まで単純な font metrics へ置き換えて kerning を壊さないよう、「縦は metrics、横は shaping」の責務分離を固定する。

**具体的に困る場面**

- 英字や句読点を縮小した template で、下端や baseline が不自然に浮いたり沈んだりする。
- mixed-script の multiline template で、行ごとの見た目は reflow しても、小文字の収まりが揃わず下敷きとして使いにくい。

**現状の問題**

- template の横幅は `egui` shaping 由来の glyph width を使っているが、縦位置は `font_size * scale` 前提の簡易モデル。
- `pauseink-fonts` は family 解決には使っているが、ascent/descent/x-height/cap-height の抽出 API はまだ無い。

**設計**

- 横方向は引き続き shaping / layout engine ベースにする。理由は、`VA` のような kerning や ligature に相当する advance 調整を壊さないため。
- 縦方向だけ metrics ベースへ寄せる。
  - 第一候補: font の `ascent / descent`
  - 取れる場合は `x-height / cap-height`
  - 取れない場合は「縮小文字は下揃え」の fallback
- `pauseink-fonts` に metrics 抽出 helper を追加し、template に使う family から line metrics を引けるようにする。
- slot には `baseline_offset_y` 相当の内部計算を導入するが、保存 format は大きく変えず、resolved slot geometry のみ再計算で吸収する。
- `line_height` は line box 間距離として維持し、per-slot vertical alignment だけを差し替える。

**着手前に決めるべきこと**

- どの metrics source を採用するか。`ttf-parser` 等の parser 追加可否を最初に決める。
- Latin/Kana/Punctuation を script 別 baseline に分けるか、単一の alphabetic-like baseline + fallback にするか。後から変えると slot 見た目が大きく変わる。
- `x-height` 不在時の fallback を「下揃え」で固定するか、bbox 近似へ寄せるか。見た目互換に効く。

**変更ファイル**

- Modify: `crates/fonts/Cargo.toml`
- Modify: `crates/fonts/src/lib.rs`
- Modify: `crates/template_layout/src/lib.rs`
- Modify: `crates/app/src/main.rs`
- Modify: `manual/user_guide.md`
- Modify: `manual/developer_guide.md`

**実装ステップ**

1. font metrics 抽出 helper を `pauseink-fonts` へ追加する。
2. template slot 計算へ vertical metrics を導入する。
3. shaping ベースの horizontal width と組み合わせる。
4. fallback 時の下揃えロジックを入れる。
5. mixed-script / multiline の回帰 test を追加する。

**必要テスト**

- `template_layout`: scaled latin / punctuation が baseline 付近で自然に揃う
- `template_layout`: metrics 不在 fallback でも縮小文字が下揃えになる
- `app`: font 切替後も metrics-based slot reflow が安定する
- `app`: `VA` 相当の width が naive fixed advance より shaping に近いことを回帰で固定する

**完了条件**

- template の縦揃えが metrics ベースで改善される
- kerning を壊さず横幅計算を維持できる
- metrics 不在時も縮小文字は下揃えで破綻しない

### PKG-01: portable FFmpeg sidecar packaging / provenance / notices

**優先度:** P0
**依存:** なし
**ひとことで言うと:** 配布 zip を展開しただけで動くように、FFmpeg 同梱の出荷形を作る task。

**目的:** mainline runtime 方針を host `ffmpeg` 依存から脱し、portable sidecar runtime を release-ready にする。

**具体的に困る場面**

- 利用者が release archive を展開しても sidecar が入っていなければ、Windows では `winget` や `PATH` 設定が必要になり、v1.0 の「portable mainline」前提から外れる。
- 配布した runtime の provenance や license summary が無いと、後から「この ffmpeg はどこ由来か」「GPL なのか」が追えず、配布判断で止まる。
- 開発者が runner ごとに適当な layout で sidecar を詰めると、discovery / diagnostics / release packaging が全部ずれる。

**現状の問題**

- runtime discovery と manifest 読み込みはあるが、実際の sidecar を repository / release asset へ載せる手順が未整備。
- provenance / notices / runtime source policy が release artifacts に反映されていない。

**設計**

- sidecar 自体は repository commit へ直接 vendor しない。取得済み runtime を CI/release packaging stage で assembly する。
- manifest は最低限次を持つ。
  - runtime version
  - source URL / source label
  - license summary
  - build summary
  - supported families
- release asset には app binary と sidecar runtime、notice file、manifest を同梱する。
- optional codec pack は mainline asset と分離し、この task では触らない。

**着手前に決めるべきこと**

- mainline asset に sidecar を同梱するか、app archive と sidecar archive を別配布にするか。後から変えると release workflow も diagnostics も変わる。
- manifest schema の必須項目を何にするか。後から field を足すと tooling 互換が揺れる。
- notices を platform ごとに分けるか、runtime ごとに 1 つにまとめるか。asset layout に直結する。
- runtime の取得元と更新手順を人手管理にするか、CI input artifact にするか。release 運用が変わる。

**変更ファイル**

- Modify: `scripts/package_release_asset.py`
- Modify: `.github/workflows/release.yml`
- Create or Modify: `manual/runtime_packaging.md` または `manual/developer_guide.md` の packaging 章
- Create: `docs/runtime_sidecar_manifest_schema.md` もしくは `.docs/` 下の補足資料

**実装ステップ**

1. release asset の directory layout を確定する。
2. manifest schema と notices の最小必須項目を定義する。
3. packager script に sidecar assembly を追加する。
4. runtime 不在時の fail-fast と log を整える。
5. packaging 手順を docs に残す。

**必要テスト**

- packager script: sidecar / manifest / notices が archive に入る
- media: sidecar manifest 不備時に明確な error を返す
- release workflow dry-run: asset 名が OS ごとに安定する

**完了条件**

- release asset が mainline sidecar 付きで組める
- provenance / notices を人が確認できる
- app が host runtime に暗黙依存しない

### PKG-02: GitHub Release への sidecar 統合

**優先度:** P0
**依存:** PKG-01
**ひとことで言うと:** GitHub Release に 3 OS 分の「完成した配布物」を自動で正しく載せる task。

**目的:** tag が `main` に入った時に 3 OS の app + sidecar + notices を GitHub Release へ確実に上げる。

**具体的に困る場面**

- release asset 名や中身が OS ごとに揺れると、利用者はどれを取ればよいか分からず、検品側も「不足 asset があるのか、命名が違うだけか」を毎回確認する羽目になる。
- Linux/Windows は成功したのに macOS だけ落ちた場合、release 全体を止めるのか一部だけ出すのかが決まっていないと、workflow の failure policy が毎回ぶれる。

**現状の問題**

- 現 release workflow は app binary archive の build / upload までは既にできているが、sidecar / notices / manifest の同梱はまだ無い。
- Windows / macOS / Linux で sidecar 同梱方針が workflow に落ちていない。

**設計**

- workflow は各 OS job で app build -> sidecar assemble -> archive -> release upload の順に統一する。
- asset 名は `pauseink-vX.Y.Z-<os>-<arch>.zip|tar.gz` を固定し、中身は共通 layout にする。
- CI 側では unit tests と release packaging を分離し、release job では `cargo test --workspace` の再実行を避けず、artifact の一貫性を優先する。

**着手前に決めるべきこと**

- macOS lane を mainline 必須にするか、Windows/Linux 必須・macOS 任意にするか。failure policy と release completeness 判定が変わる。
- release 完成条件を「全 asset 必須」にするか、「Windows/Linux 必須 + macOS 任意」にするか。`discover-release-targets` の期待 asset 集合に影響する。
- sidecar assembly の source を workflow 内ダウンロードにするか、事前配置 artifact にするか。運用コストが大きく変わる。
- partial success 時に release を作らない方針にするか、suffix を付けて暫定公開するか。後から変えると automation の契約が崩れる。

**変更ファイル**

- Modify: `.github/workflows/release.yml`
- Modify: `.github/workflows/ci.yml` （必要なら reusable workflow 化）
- Modify: `scripts/package_release_asset.py`
- Modify: `manual/developer_guide.md`

**実装ステップ**

1. OS ごとの sidecar input path を workflow で決める。
2. packager script を release workflow から呼ぶ。
3. asset contents チェックを workflow step へ入れる。
4. release body に runtime provenance / notices の説明を差し込む。

**必要テスト**

- workflow YAML parse
- packager unit/dry-run
- 手元で Linux archive 生成
- `act` 等が使えるならローカル dry-run、無理なら exact blocker を docs に記録

**完了条件**

- release asset が 3 OS で同じ規約で作られる
- Release ページから notices / manifest / runtime source が追える

### QA-01: cross-platform validation / closeout

**優先度:** P0
**依存:** V1-02, V1-03, V1-04, V1-06, V1-07, PKG-01, PKG-02
**ひとことで言うと:** 最後に各 OS で本当に使えるかを確認し、完了判定を閉じる task。

**目的:** done criteria を最後に閉じる。Linux / Windows / macOS で build / import / save-load / composite export / transparent export を確認し、残制限を確定する。

**具体的に困る場面**

- 利用者が release を受け取っても、どの OS でどこまで確認済みか分からなければ、実質的に毎回手探りで同じ検証を繰り返すことになる。
- export や recovery のような重い機能は、unit test だけ通っていても OS 差分で壊れやすく、最後の実機検証を飛ばすと mainline 判定が不正確になる。

**現状の問題**

- Linux では host runtime 検証があるが、Windows / macOS は runtime discovery unit test 中心で、実 export validation が不足している。
- headless host のため GUI 目視起動が一部未確認。

**設計**

- 検証は OS ごとに同じチェックリストで回す。
  - build
  - launch
  - import media
  - free ink / guide / template
  - save -> reopen
  - composite export
  - transparent export
  - runtime diagnostics
- 実施不能項目は「なぜ不可か」「次に必要な環境」を exact に記録する。

**着手前に決めるべきこと**

- どの OS を「必須通過」、どれを「blocker 記録で可」とするか。done criteria の判定に直接関わる。
- smoke 用 media / sample project を何で固定するか。後から素材が変わると比較不能になる。
- report に残す証跡の粒度を「コマンド + exit code + 生成物 path」まで含めるかどうか。後から追跡できるかが変わる。
- GUI 目視確認ができない環境でどこまでを local verify とみなすか。曖昧にすると完了判定がぶれる。

**変更ファイル**

- Modify: `docs/implementation_report_v1.0.0.md`
- Modify: `progress.md`
- Modify: `README.md`
- Modify: `manual/user_guide.md`
- Modify: `manual/developer_guide.md`

**実装ステップ**

1. OS 別 validation matrix を report へ追加する。
2. Linux 実検証を更新する。
3. Windows 実 build/runtime/export を実施する。無理なら blocker を exact に書く。
4. macOS 実 build/runtime/export を実施する。無理なら blocker を exact に書く。
5. done criteria を再確認し、未達があれば該当 task へ差し戻す。

**必要テスト / 実行コマンド**

- `cargo fmt --all --check`
- `cargo test --workspace`
- `cargo check -p pauseink-app --all-targets`
- OS ごとの release build
- 実 export 1 件以上ずつ

**完了条件**

- `.docs/10_testing_and_done_criteria.md` の項目を正直にすべて判定できる
- `progress.md` が最終状態になっている
- `docs/implementation_report_v1.0.0.md` がコマンドと結果を含んで完結している

### FUT-08: object 選択時の preview/canvas ハイライト

**優先度:** Future
**依存:** V1-06
**ひとことで言うと:** 選択中 object を canvas 側でも追えるようにしつつ、preview の見た目を壊さないハイライト表現を定める future work。

**目的:** object outline で選んだ object が preview 上のどれかを分かりやすくする。ただし annotation の完成見た目を強い装飾で壊さない。

**具体的に困る場面**

- object 数が多い project で、outline から object を選んでも preview 上の対応物がどれか即座に分からない。
- 強すぎる selection overlay を入れると、実際の線色や effect の確認がしづらくなる。

**現状の問題**

- selection の source of truth 自体は `V1-05` で app session にあるが、canvas 側へ可視化していない。
- どの visual language が preview を壊さないかの UX 検討がまだ無い。

**設計**

- base stroke を直接塗り替えず、editor-only overlay で示す。
- 候補は次の 3 つまでに絞って比較する。
  - subtle outline halo
  - low-alpha bounding box
  - active object のみ faint pulse
- export には絶対に漏らさない。
- multi-select 時は「focus object を強く、他 selected を弱く」の 2 段階表現を基本にする。

**着手前に決めるべきこと**

- 選択ハイライトを bbox ベースにするか stroke path ベースにするか。見た目と実装コストが大きく変わる。
- focus object と selected set の見分け方をどうするか。selection UX の根本になる。
- preview correctness 優先時にハイライトをどの程度 suppress するか。effect 編集時の視認性に影響する。

**変更ファイル**

- Modify: `crates/app/src/main.rs`
- Modify: `crates/renderer/src/lib.rs` または app overlay 描画部
- Modify: `manual/user_guide.md`

**必要テスト**

- `app`: selected/focused object の overlay state が切り替わる
- `app`: export snapshot に selection highlight が漏れない
- `app`: multi-select 時に focus と non-focus が区別される

**完了条件**

- preview 上で選択対象を見失いにくくなる
- 完成見た目の確認を阻害しない
- export へ editor-only overlay が漏れない

## 6. 着手おすすめ順

ここでは **未着手 task のみ** を、依存だけでなく「後戻りしにくい順」「reported bug を早く止める順」で並べます。

1. `V1-09` template font switch crash fix
   - crash は最優先で止める価値が高く、局所修正で済む可能性が高い。
2. `V1-10` multiline template editor UI
   - engine 既存機能の GUI 開放で、仕様変更が小さい。
3. `V1-11` panel-aware wide controls
   - layout 層の改善で独立性が高く、後続 UI task の土台になる。
4. `V1-12` template placement action row simplification
   - `V1-11` 後なら panel 幅と action row を同時に整理しやすい。
5. `V1-13` `Esc` cancel for transient modes
   - keyboard 導線の改善で独立性が高いが、popup/focus 優先順位だけは先に固定する。
6. `V1-14` metrics-based template alignment
   - 設計は固められるが、font metrics source の追加を伴うため一段重い。
7. `V1-02` reveal-head effect
   - `V1-01` 後なら preset 境界が固まり、renderer への局所変更で進めやすい。
8. `V1-04` clear / clear preset / combo preset
   - 利用者が直接困る操作 gap が大きく、page-event track の source of truth をここで確定できる。
9. `V1-06` outline / page events panel 強化
   - `V1-05` と `V1-04` が揃ってから入ると UI だけ空洞になるのを避けられる。
10. `V1-03` post-action chain
    - 最も timing が複雑で、group/run の基盤と outline 可視化がある方が安全。
11. `PKG-01` portable sidecar packaging
    - productization 側の大きな未完了。release asset 設計をここで固定する。
12. `PKG-02` release workflow sidecar 統合
    - workflow 自体はあるので、PKG-01 完了後は sidecar 統合へ絞って進められる。
13. `QA-01` cross-platform validation / closeout
    - 最後に OS ごとの build/import/export/diagnostics を実機証跡で閉じる。

## 7. task 指定時の返答ルール

利用者から `V1-03` や `PKG-01` のように task ID が指定されたら、開始時に次を短く共有してから着手すること。

1. 対象 task の依存が満たされているか
2. 今回触るファイル
3. 最初に追加する failing test
4. 完了時に回す verification command

## 8. 先に確認しておくべき地雷

- `V1-01` を飛ばして `V1-04` へ行くと preset schema の後戻りが起きやすい。
- `V1-05` を飛ばして `V1-06` を進めると outline tree の UI だけ出来て command が空洞になる。
- `PKG-01` を飛ばして `PKG-02` を進めると release workflow が host runtime 前提に戻りやすい。
- `QA-01` は単なる docs 更新ではなく、実 build / export / validation の証跡が必須。

## 9. 受け入れの最終ライン

この計画の task をすべて閉じた時点で、PauseInk v1.0 の残差は以下に限定されているべきです。

- `.docs/12_future_work.md` に明示した future work
- optional codec pack の別 tier 検討
- deprecation warning のような非機能系の軽微な改善

それ以外が残る場合は、対応する task を追加してこの文書を更新すること。
