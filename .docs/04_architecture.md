# アーキテクチャ

## 1. アーキテクチャの優先順位

1. 正しさ
2. ポータビリティ
3. 予測可能な fallback 挙動
4. 疎結合
5. 何もかも書き直さずに将来拡張できること

## 2. 想定する workspace / module 分割

想定する crate / module は次の通りです。

- `app` - binary entrypoint と app wiring
- `domain` - 中核の project model と commands
- `project_io` - `.pauseink` の parse / normalize / save
- `portable_fs` - executable-local の state、cache、logs、autosave
- `presets_core` - preset schema と profile resolution
- 実装の途中で追加する future module:
  - `ui`
  - `renderer`
  - `media`
  - `export`
  - `fonts`
  - `template_layout`

crate の正確な分割は変わっても構いませんが、境界は維持しなければなりません。

## 3. データ所有権

### 3.1 単一 writer モデル

project state の変更は、UI / app thread だけで行います。

### 3.2 スナップショットベースの background job

background worker は次のために不変の snapshot を受け取ります。

- export
- probe
- thumbnail 生成
- cache cleanup 候補の走査

worker が live project state を直接変更してはいけません。

### 3.3 イベントの戻り道

worker は message / event / result を通じて UI thread に返します。

## 4. Rendering モデル

### 4.1 Preview

- GPU 優先の経路
- 起動時または最初の canvas 生成時に backend / runtime を probe する
- 利用できない場合はきれいに fallback する

### 4.2 最終合成

v1.0 の baseline は、正しさと再現性を重視した CPU 安全な合成経路です。

将来的に GPU export compositor を追加しても構いませんが、v1.0 の足を引っ張ってはいけません。

### 4.3 ストローク描画

stroke rendering pipeline は次の層で組みます。

- raw sample
- stabilized sample
- derived path / mesh
- effect の適用
- composite

## 5. メディアアーキテクチャ

### 5.1 Provider abstraction

media access は provider interface の後ろに置き、責務は次の通りです。

- probing
- capability discovery
- frame access / playback 支援
- export 呼び出し
- diagnostics

### 5.2 Runtime の置き場

開発時は portable root の下、または repository-local の runtime layout に置く sidecar runtime を想定します。

### 5.3 Capability model

アプリは runtime が実際に何をできるかを問い合わせるべきであり、勝手に決めつけてはいけません。

- decoder support
- encoder support
- muxer support
- hardware acceleration の可用性

## 6. エクスポートアーキテクチャ

### 6.1 責務の分離

export は次を組み合わせて構成します。

- project snapshot
- 選ばれた container / codec family
- 選ばれた distribution profile
- 計算済みの concrete settings
- provider capability の結果
- software / hardware path の選択

### 6.2 Fallback の順序

推奨挙動は次の通りです。

1. target settings を計算する
2. media HW accel が許可され、かつ capability 的に可能そうなら hardware path を試す
3. hardware path が失敗したら software path で 1 回だけやり直す
4. どの path を使ったかと理由を log する

## 7. プロジェクトモデルの注意点

domain model は次を区別しなければなりません。

- z-order
- capture order
- reveal order
- page clear event
- object / group の関係
- preset reference と resolved snapshot

## 8. 将来拡張の hook

次のための hook を残します。

- ペン圧
- pseudo-pressure / taper
- partial clear
- proxy media
- user effect scripting
- GPU export compositor
- codec-pack の導入補助

これらは、半端な UI ではなく拡張点として存在させます。
