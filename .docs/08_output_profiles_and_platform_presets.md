# 出力 profile と platform preset

## 1. export の判断を 2 層に分ける

### レイヤーA - container/codec family

例:

- WebM / VP9 / Opus
- WebM / AV1 / Opus
- MP4 / AV1 / AAC-LC
- MOV / ProRes 422 HQ / PCM
- MOV / ProRes 4444 / PCM
- PNG Sequence / RGBA
- AVI / MJPEG / PCM

### レイヤーB - distribution/profile preset

例:

- Low
- Medium
- High
- YouTube
- X
- Instagram
- Adobe Edit
- Adobe Alpha
- Custom

この分割によって system は拡張しやすくなり、codec logic を書き直さずに platform preset を追加できます。

## 2. UI の挙動

- 利用者は family と profile を選びます。
- アプリは具体的な数値設定を計算します。
- Custom 以外では、数値入力欄に計算済み値を表示しつつ編集不可にします。
- Custom では数値欄を編集可能にします。

数値欄には少なくとも次を含めます。

- target video bitrate
- 必要なら max video bitrate
- audio bitrate
- GOP / keyframe interval
- sample rate
- 必要に応じて CRF / CQ など、該当 codec family で使う quality target

## 3. 公式ソースと app-authored ソース

### 3.1 YouTube

直接使えるなら、公式の公開 encoding guidance を使います。

### 3.2 X

直接使えるなら、公式の公開 upload guidance を使います。

### 3.3 Instagram

使える範囲では公式の公開制約を使います。  
正確な bitrate ladder が同じ形で公開されていない場合は、app-authored の「safe defaults」を使い、そのことを正直にラベルします。

### 3.4 Adobe

Adobe と互換性のある family を元に、app-authored の intermediate / editing preset を使います。

## 4. 組み込み preset の期待値

### 4.1 Web / social 向けの既定 family

- WebM VP9 + Opus: 主な open default
- WebM AV1 + Opus: 高圧縮の default
- MP4 AV1 + AAC-LC: 1 ファイル互換性を高めた advanced option

### 4.2 Adobe / editing 向け family

- MOV ProRes 422 HQ + PCM: editing master / intermediate
- MOV ProRes 4444 + PCM: alpha / intermediate
- PNG sequence RGBA: 透明度互換性が最も高い出力

### 4.3 Legacy rescue

- AVI MJPEG + PCM

## 5. データ駆動の profile file

profile 定義は `presets/export_profiles/` 配下の宣言ファイルに置きます。

想定する責務は次の通りです。

- platform / profile 名
- 想定する family 互換性
- bitrate ladder
- frame-rate 調整規則
- audio の既定
- 補足メモ
- 参照 URL

アプリは UI コードに rule を hard-code するのではなく、安定した schema で読み込みます。

## 6. 解像度を考慮した計算の方向

計算 engine は少なくとも次を考慮するべきです。

- 出力 width / height
- frame rate bucket
- family capability
- platform / profile の優先
- alpha の必要性
- audio の有無

## 7. 公式ガイダンスの参照例

Codex は実装中に次のような公式ページから、実際に採用した値を確認して記録してください。

- YouTube recommended upload encoding settings
- X media upload / media studio guidance
- Instagram Reels / public constraints
- Adobe Media Encoder の import / export 対応形式ページ

最終的に採用した URL と数値は `docs/implementation_report_v1.0.0.md` に保存します。

## 8. Profile 拡張のルール

新しい platform preset を追加するのは、開発者にとって簡単でなければなりません。

1. 新しい profile file を作る
2. 必要なら profile catalog に登録する
3. test を追加 / 更新する
4. developer guide に記載する

## 9. Audio policy の補足

- **PCM** は、editing / master 出力向けの非圧縮・ロスレス系 intermediate です。
- social / web 配布 preset では、選んだ family / platform に応じて **AAC-LC** か **Opus** を優先します。
- Adobe 向け intermediate preset では、ファイルサイズより編集しやすさと忠実性を重視するため PCM を使って構いません。
