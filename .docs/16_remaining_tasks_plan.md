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
  - commit 後は都度 `origin/prototype` へ push する

## 1. 範囲の切り分け

### 1.1 この計画に含めるもの

以下は v1.0 仕様に対して未接続または最小実装のまま残っているため、この計画で扱います。

1. preset 境界の正規化と field-level reset / inherit
2. reveal-head effect
3. post-action chain
4. clear effect の実装完了と clear / combo preset UI
5. selection / multi-select / group / ungroup / z-order の編集導線
6. object outline / page events panel の強化
7. template advanced controls と slot fit
8. portable FFmpeg sidecar packaging / provenance / notices
9. GitHub Release への sidecar 統合
10. Windows / macOS / Linux の最終検証

### 1.2 この計画に含めないもの

以下は `.docs/12_future_work.md` に従い future work として維持し、ここでは実装対象にしません。

- `FUT-01`: partial clear
- `FUT-02`: pen pressure
- `FUT-03`: auto taper / pseudo-pressure の高度化
- `FUT-04`: proxy media
- `FUT-05`: GPU export compositor
- `FUT-06`: optional codec-pack 取得ツール
- `FUT-07`: arbitrary effect scripting

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

## 2. 依存関係マップ

| Task ID | タイトル | 依存 | 目的の要点 |
|---|---|---|---|
| V1-01 | preset 境界の正規化 | なし | style / entrance / clear / combo を分け、reset / inherit を扱える editor state を入れる |
| V1-02 | reveal-head effect | V1-01 | head effect を renderer / preview / export / preset に接続する |
| V1-03 | post-action chain | V1-01 | built-in post-action を renderer / UI / project に接続する |
| V1-04 | clear effect / clear preset / combo preset | V1-01 | clear kind / ordering / granularity を UI と renderer で完結させる |
| V1-05 | selection / group / z-order foundation | なし | multi-select, group, ungroup, z-order の command と UI を入れる |
| V1-06 | object outline / page events panel 強化 | V1-04, V1-05 | tree 表示、batch edit、現在生存中表示、auto-follow を揃える |
| V1-07 | template advanced controls / slot fit | なし | line height / script scale / underlay mode / fit option を UI に露出する |
| PKG-01 | portable FFmpeg sidecar packaging | なし | sidecar layout, manifest, provenance, notices を出荷形にする |
| PKG-02 | GitHub Release sidecar 統合 | PKG-01 | 3 OS build + release asset に sidecar / notices を載せる |
| QA-01 | cross-platform validation / closeout | V1-02, V1-03, V1-04, V1-06, V1-07, PKG-01, PKG-02 | 実 build / runtime / export を OS ごとに通し、docs を確定する |

## 3. 実装対象の現状スナップショット

- `reveal-head effect` は `crates/domain/src/annotations.rs` に型だけあるが、`crates/app/src/main.rs` の inspector と `crates/renderer/src/lib.rs` の描画には未接続。
- `post-action chain` も domain に型だけあり、app / renderer / preset へ未接続。
- clear event は domain と renderer に最小 primitive があるが、app 側は `全消去 -> Instant clear 挿入` のみで、`kind / duration / granularity / ordering` の編集 UI と preset 導線が無い。
- preset は現状 `style preset` に entrance まで同居しており、spec の `base style / entrance / clear / combo` 分離と `inherit / reset` UI がまだ無い。
- selection は `selected_object_id: Option<GlyphObjectId>` の単一選択だけで、multi-select / group / ungroup / z-order 操作が未整備。
- `オブジェクト一覧` は flat text list、`ページイベント` は flat list で、spec の tree / batch edit / alive highlight / auto-follow に未達。
- template では内部の `line_height / kana_scale / latin_scale / punctuation_scale / underlay_mode` はあるが UI 露出が不足し、`slot fit` は未実装。
- FFmpeg sidecar は discovery までで、release asset への bundling / provenance / notices / CI upload が未完了。

## 4. Task 詳細

### V1-01: preset 境界の正規化と field-level reset / inherit

**優先度:** P0
**目的:** spec の preset category と reset semantics を実装可能な形へ整える。以後の effect / clear / combo 実装が後戻りしないよう、editor state と catalog 構造をここで固定する。

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

**目的:** spec 5.3 の `none / solid / glow / comet-tail` を preview/export で正しく見せ、preset と project 保存に接続する。

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

**目的:** `during reveal / after stroke / after glyph object / after group / after run` に対する built-in post-action を renderer と UI へ接続する。

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

**目的:** clear event を `Instant / Ordered / ReverseOrdered / WipeOut / DissolveOut` まで UI と renderer で完結させ、clear preset と combo preset へ接続する。

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

**目的:** spec 9 の編集導線を成立させる。現在の単一 `selected_object_id` から multi-select と command 群へ拡張する。

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

**目的:** spec 10 の panel 要件へ近づける。flat list を tree / batch-edit 可能な panel に引き上げる。

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

### V1-07: template advanced controls と slot fit

**優先度:** P1
**依存:** なし

**目的:** spec 4.3 / 7 の template settings を UI へ露出し、slot fit を v1.0 範囲で入れる。

**現状の問題**

- `line_height / kana_scale / latin_scale / punctuation_scale / underlay_mode` は内部 state にあるが UI に出ていない。
- `slot fit` の `Off / Move only / Weak uniform scale` が未実装。

**設計**

- template editor は左ペインのまま維持し、advanced section を `詳細` collapsible にまとめる。
- slot fit は object capture 後の幾何補正ではなく、slot commit 時の object transform として適用する。
  - `Off`: 現状維持
  - `MoveOnly`: slot center へ平行移動だけ
  - `WeakUniformScale`: bounding box 比から `1.0..=1.15` 程度の弱い uniform scale を掛ける
- stroke の最終表示はユーザー手書き主体を守るため、非一様スケールや強制歪みは入れない。

**変更ファイル**

- Modify: `crates/template_layout/src/lib.rs`
- Modify: `crates/app/src/main.rs`
- Modify: `crates/app/src/lib.rs`
- Modify: `manual/user_guide.md`

**実装ステップ**

1. 左ペイン template section に advanced controls を追加する。
2. `UnderlayMode` の dropdown を追加する。
3. slot fit mode を editor state と project/settings restore に追加する。
4. slot commit 時に `MoveOnly / WeakUniformScale` を object transform へ適用する。
5. preview と export が同じ transform を使うことを確認する。

**必要テスト**

- `template_layout`: script scale / line height / underlay mode の UI 値が slot 計算へ反映される
- `app`: save/reopen で advanced template settings と fit mode が戻る
- `renderer/app`: `WeakUniformScale` が上限を超えず、stroke 形状を過度に歪めない

**完了条件**

- template advanced settings が GUI から編集できる
- slot fit が spec の 3 モードで動く
- 手書き主体の見た目を壊さない

### PKG-01: portable FFmpeg sidecar packaging / provenance / notices

**優先度:** P0
**依存:** なし

**目的:** mainline runtime 方針を host `ffmpeg` 依存から脱し、portable sidecar runtime を release-ready にする。

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

**目的:** tag が `main` に入った時に 3 OS の app + sidecar + notices を GitHub Release へ確実に上げる。

**現状の問題**

- 現 release workflow は app binary archive しか載せない。
- Windows / macOS / Linux で sidecar 同梱方針が workflow に落ちていない。

**設計**

- workflow は各 OS job で app build -> sidecar assemble -> archive -> release upload の順に統一する。
- asset 名は `pauseink-vX.Y.Z-<os>-<arch>.zip|tar.gz` を固定し、中身は共通 layout にする。
- CI 側では unit tests と release packaging を分離し、release job では `cargo test --workspace` の再実行を避けず、artifact の一貫性を優先する。

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

**目的:** done criteria を最後に閉じる。Linux / Windows / macOS で build / import / save-load / composite export / transparent export を確認し、残制限を確定する。

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

## 5. 推奨実行順

1. `V1-01` preset 境界正規化
2. `V1-02` reveal-head effect
3. `V1-03` post-action chain
4. `V1-04` clear / clear preset / combo preset
5. `V1-05` selection / group / z-order foundation
6. `V1-06` outline / page events panel 強化
7. `V1-07` template advanced controls / slot fit
8. `PKG-01` portable sidecar packaging
9. `PKG-02` release workflow sidecar 統合
10. `QA-01` cross-platform validation / closeout

## 6. task 指定時の返答ルール

利用者から `V1-03` や `PKG-01` のように task ID が指定されたら、開始時に次を短く共有してから着手すること。

1. 対象 task の依存が満たされているか
2. 今回触るファイル
3. 最初に追加する failing test
4. 完了時に回す verification command

## 7. 先に確認しておくべき地雷

- `V1-01` を飛ばして `V1-04` へ行くと preset schema の後戻りが起きやすい。
- `V1-05` を飛ばして `V1-06` を進めると outline tree の UI だけ出来て command が空洞になる。
- `PKG-01` を飛ばして `PKG-02` を進めると release workflow が host runtime 前提に戻りやすい。
- `QA-01` は単なる docs 更新ではなく、実 build / export / validation の証跡が必須。

## 8. 受け入れの最終ライン

この計画の task をすべて閉じた時点で、PauseInk v1.0 の残差は以下に限定されているべきです。

- `.docs/12_future_work.md` に明示した future work
- optional codec pack の別 tier 検討
- deprecation warning のような非機能系の軽微な改善

それ以外が残る場合は、対応する task を追加してこの文書を更新すること。
