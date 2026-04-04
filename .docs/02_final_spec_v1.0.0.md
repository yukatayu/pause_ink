# 最終製品仕様 v1.0.0

## 1. コアコンセプト

PauseInk は、動画フレームに手書きのオーバーレイを載せるためのアプリです。  
オーバーレイは動画時刻に紐づき、設定可能な reveal 挙動で現れ、1 つの page 区間にわたって残り、手動の screen-wide clear event で消えます。

設計目標は次の通りです。

- ユーザー自身の手書きらしさを残す
- 読みやすさに必要なぶんだけ構造を与える
- 予期しない破壊的な正規化を避ける
- UI を理解しやすく保つ

## 2. タイムラインモデル

### 2.1 動画タイムライン

プロジェクトは、元メディアのタイムラインを主時間基準として使います。

### 2.2 ページモデル

**Page** は、2 つの clear 境界のあいだにある区間です。

- プロジェクト開始時、または直前の clear の直後
- 次の clear event もしくはプロジェクト終了まで

### 2.3 Clear event モデル

Clear event とは、次のものです。

- pause 中でも再生中でも、利用者が明示的に挿入する
- 個別 stroke ではなく page-event track が持つ
- screen-wide で、その瞬間に生存している注釈 object 全体へ影響する

v1.0 の clear 挙動は次の通りです。

- instant clear
- 書き込み順に従う ordered clear
- 逆順の ordered clear
- wipe out
- dissolve out

Clear event が持つ内容は次の通りです。

- clear kind
- duration
- effect アルゴリズム向けの target granularity
  - object
  - group
  - stroke
  - all parallel
- ordering
  - serial
  - reverse
  - parallel

clear event は screen-wide ですが、内部的な見た目の順序付けは object/group/stroke 単位で行っても構いません。

### 2.4 v1.0 では部分 clear をしない

UI に次の機能を出してはいけません。

- 選択中だけ clear
- 現在の group だけ clear
- tag で clear
- region で clear

これらは future work に残します。

## 3. 注釈 object モデル

### 3.1 ストローク

Stroke は次を保持します。

- raw input points
- timestamp 付き sample
- 派生した render path
- style snapshot
- 作成時刻の anchor

### 3.2 Glyph Object

Glyph object は、文字相当の主注釈単位です。

含みうるものは次の通りです。

- 1 本以上の stroke
- style snapshot
- entrance 挙動
- post-entrance 挙動チェーン
- geometry transform
- z-order
- capture/reveal order のメタデータ

### 3.3 Group

Group は、利用者が定義する glyph object / stroke の集合で、次の用途に使います。

- reveal 挙動の共有
- post-action の timing scope の共有
- 一括編集

### 3.4 Runs

Run は、同じ設定を共有する連続 object を outline panel 上でまとめた表示由来のグルーピングです。  
Run は利用者定義 group とは別物であり、明示的な group データの代わりにはなりません。

## 4. 入力モード

### 4.1 Free Ink

利用者は canvas に直接描きます。

- 既定モードでは guide を使わない
- 1 回の pen-down から pen-up が 1 つの stroke になる
- `Shift` で連続 stroke を 1 つの glyph object にまとめる

### 4.2 ガイドキャプチャ

通常、guide は非表示です。  
guide modifier を押しながら、利用者が参照用 glyph object を書くと次のようになります。

- その参照 object が guide geometry を決める
- 後続の書き込みに guide が現れる
- guide は editor 専用で、export されない

必要な guide の見た目は次の通りです。

- 長い横 3 線システム
- そのあいだの薄い補助線 2 本
- 次文字用の短い縦 3 線ガイド
- 次文字枠用の薄い補助線 2 本
- 調整可能な上向き slope angle
- 起動をまたいで設定が保持される

platform-default の guide modifier は次の通りです。

- Windows / Linux: `Ctrl`
- macOS: `Option`

この modifier は settings から再割り当て可能でなければなりません。

### 4.3 テンプレート配置

利用者はテキストを入力し、template underlay を設定します。

- font family
- font size
- tracking
- line height
- kana scale
- latin scale
- punctuation scale
- slope angle
- underlay mode

`Place Template` を押すと placement mode に入ります。

- underlay はポインタに追従する
- クリックで配置する
- settings の変更は preview にリアルタイム反映される
- cancel で placement mode を抜ける
- cancel または新しい placement で、placement mode の underlay は消える

template が定義するのは **slots** であり、最終見た目の glyph 置換そのものではありません。

#### Underlay mode

v1.0 が対応するのは次のものです。

- outline underlay
- faint fill underlay
- slot box only
- outline + slot box

### 4.4 テンプレートの capture 挙動

template が有効なときの既定解釈は次の通りです。

- 複数 stroke が現在の slot の glyph object に寄与する
- commit は明示的な next-slot action、または next-slot start により次 slot へ進む
- `Shift` は template mode の外で force-group 挙動に使える

## 5. 見た目設定

### 5.1 Base style

各 visible object は次の base style フィールドを持ちます。

- thickness
- color
- opacity
- outline
- drop shadow
- glow
- blend mode（最低でも normal, additive）

### 5.2 Entrance 挙動

組み込みの entrance kind は次の通りです。

- path trace
- instant
- wipe
- dissolve

Entrance パラメータには次が含まれます。

- target scope: stroke / glyph object / group / run
- order: serial / reverse / parallel
- duration mode:
  - stroke 長に比例
  - 固定 total duration
- speed scalar

### 5.3 Reveal-head effect

Entrance には任意で head effect を付けられます。

- none
- solid head
- glow head
- comet / tail head

Head effect のパラメータは次の通りです。

- color source: preset accent / stroke color / custom
- size multiplier
- blur radius
- tail length
- persistence
- blend mode

### 5.4 Post-action

Post-action は、reveal の後または途中で起きる状態変更チェーンです。  
チェーンの各要素は次を指定します。

- timing scope:
  - during reveal
  - after stroke
  - after glyph object
  - after group
  - after run
- action:
  - no-op
  - style change
  - interpolated style change
  - pulse
  - blink

v1.0 では組み込みだけで十分であり、任意スクリプトは不要です。

## 6. Preset

### 6.1 Preset category

v1.0 に含めるのは次のものです。

- base style presets
- entrance presets
- clear presets
- combo presets

### 6.2 built-in と user preset の違い

- built-in preset は読み取り専用
- user preset は編集可能で、portable root に保存する
- project には **resolved snapshot** と任意の preset ID を保存する
- 古い見た目を再現するために、mutable な live preset file に project が依存してはいけない

### 6.3 reset 挙動

各編集可能フィールドは次のいずれかにできます。

- preset から継承
- 上書き
- preset 値へリセット

## 7. テキストレイアウトと spacing

template system は次をサポートしなければなりません。

- grapheme-aware な slot 生成
- kana scale
- latin scale
- punctuation scale
- tracking
- line height
- slope
- mixed-script layout

手書き出力は slot に配置されますが、利用者が gentle fitting を選ばない限り、強制的には形を変えません。

### 7.1 Slot fit オプション

v1.0 が対応するのは次の通りです。

- Off
- Move only
- Weak uniform scale

既定値は **Off** です。

## 8. smoothing と stroke stabilization

v1.0 では、次のルールで調整可能な stroke stabilization を提供します。

- raw point を保持する
- render path は派生させる
- corner はなるべく残す
- smoothing は 1 つの強さスライダーで調整できる

設計目標は次の通りです。

- adaptive One Euro 風フィルタリング
- corner guard / curvature-aware な smoothing 抑制
- 将来の pseudo-pressure / taper 用の明示的な hook

## 9. 選択・順序・編集

利用者は次を行えます。

- object を選択する
- 複数選択する
- group 化する
- ungroup する
- z-order を並べ替える
- style / effect を一括編集する

アプリは **capture/reveal order** と **z-order** を概念上きちんと分離しなければなりません。

## 10. パネル

### 10.1 Object Outline

次のものを表示する tree 形式の panel です。

- run
- group
- glyph object
- stroke

次をサポートしなければなりません。

- 展開 / 折りたたみ
- 複数選択
- 一括編集
- 並べ替え
- visibility / lock / solo
- 現在生存中の強調表示
- 任意の auto-follow-current

### 10.2 ページイベント

clear event のみを扱う独立した timeline track です。

### 10.3 Export キュー

export job の簡単な queue / status 表示です。

### 10.4 Logs

トラブルシュート用に、最近の log 出力をアプリ内で見られると望ましいです。

## 11. save/load 挙動

### 11.1 プロジェクトファイル

- 拡張子: `.pauseink`
- encoding: UTF-8
- 形式: JSON5 風テキスト
- load 時にコメントと trailing comma を許可する
- write 時には canonical に正規化して保存する

### 11.2 未知フィールド

手編集と forward compatibility を支えるため、未知フィールドは可能な範囲で保持します。

### 11.3 Autosave

autosave は必須です。

### 11.4 Crash recovery

直近 autosave からの recovery が必須です。

## 12. Export 挙動

### 12.1 Composite export

元動画と注釈を合わせて出力します。

### 12.2 Transparent export

注釈だけを出力します。

必要な transparent family は次の通りです。

- PNG Sequence RGBA
- MOV / ProRes 4444 / PCM（または、音声を含めないなら silent）

### 12.3 Export profile

UI は次の 2 層を分けて見せなければなりません。

- **container/codec family**
- **distribution preset**

distribution preset は次の通りです。

- Low
- Medium
- High
- YouTube
- X
- Instagram
- Adobe Edit
- Adobe Alpha
- Custom

非 Custom preset では次を行います。

- 計算済みの数値を表示する
- 数値入力 widget は無効にする

Custom では次を行います。

- 直接数値を編集できる

## 13. Import 挙動

入力メディアの対応範囲は export family に限定されません。  
Import は、アクティブな FFmpeg runtime が probe / decode できるものを受け付けます。

アプリは runtime probe の結果に基づいて、import を次のように分類します。

- supported
- supported with caveats
- unsupported

## 14. Preferences

最低でも次を含めます。

- undo history depth
- portable root override（developer/test 専用。hidden / advanced でよい）
- guide modifier override
- guide slope angle
- guide persistence options
- GPU preview toggle
- media hardware acceleration toggle
- autosave cadence
- cache size の目安表示または cleanup 操作
- Google Fonts の configured families
- local font directories（追加の任意 root）

## 15. v1.0 の対象外

- partial clear
- scene-cut を使った自動 clear 挿入
- ペン圧
- 任意の effect scripting
- 自動 proxy 生成
- NLE 級の完全な media 管理
