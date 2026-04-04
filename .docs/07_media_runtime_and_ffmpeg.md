# メディア runtime、FFmpeg provider、codec policy

## 1. mainline のランタイム方針

v1.0 の mainline は **portable sidecar runtime** を使います。

想定する配置は次のようなものです。

```text
pauseink_data/
  runtime/
    ffmpeg/
      <platform-id>/
        ffmpeg[.exe]
        ffprobe[.exe]
        manifest.json
```

repository-local な development-time layout も、明記されているなら許容します。

## 2. なぜ first-run 自動ダウンロードを mainline にしないのか

mainline のアプリを、初回起動時の FFmpeg バイナリ自動ダウンロードに依存させてはいけません。

理由は次の通りです。

- provenance / compliance の懸念
- third-party binary の入手元が不安定
- オフライン時の挙動が難しくなる
- テスト再現性が下がる
- 最も失敗しやすい経路に複雑さが増える

補助ツールを後で追加することはできますが、v1.0 の critical path にしてはいけません。

## 3. capability ベースの挙動

アプリは実行時に runtime capability を発見しなければなりません。対象は次の通りです。

- 利用可能な decoder
- 利用可能な encoder
- 利用可能な muxer
- 利用可能な pixel format
- hardware acceleration の可能性

container extension だけで広い前提を hard-code しないでください。

## 4. import の考え方

import の対応範囲は、export family より広くて構いません。

アプリは、active runtime が probe / decode できるファイルを import してよいです。

import 時には media を次のように分類します。

- supported
- supported with caveats
- unsupported

考えうる caveat の例は次の通りです。

- variable frame rate
- alpha 非対応
- 特殊な timebase
- decode はできるが seek が非効率

## 5. GPU / media acceleration の考え方

media acceleration は任意機能であり、UI preview の GPU 利用とは別に設定可能にします。

推奨アルゴリズムは次の通りです。

1. media HW acceleration が有効で、かつ可能そうなら試す
2. 無理なら、または失敗したら software に落とす
3. アプリ自体は動き続ける

hardware acceleration がないだけで、製品全体を失敗にしてはいけません。

## 6. 主な組み込み export family

mainline の組み込みは次の通りです。

- WebM / VP9 / Opus
- WebM / AV1 / Opus
- MP4 / AV1 / AAC-LC（Advanced）
- MOV / ProRes 422 HQ / PCM
- MOV / ProRes 4444 / PCM
- PNG Sequence / RGBA
- AVI / MJPEG / PCM（Legacy rescue）

## 7. optional codec-pack 領域

次のものは、意図的に optional / future codec-pack 領域として扱います。

- H.264 encode
- HEVC encode

理由は次の通りです。

- licensing / patent / compliance が複雑
- core app を MIT-friendly に保ちたい
- GPL-only の FFmpeg build を mainline 前提にしたくない

## 8. H.264 / HEVC import の注意

利用者は、H.264 素材を読むことにも licensing の懸念があるのか、という点を明示的に気にしています。

設計上の結論は次の通りです。

- codec runtime policy と app license policy を分ける
- media provider と packaging の判断を十分に文書化する
- H.264 ingest を docs 上で「法的に心配不要」とは書かない
- release packaging には別の legal review が必要になりうると明記する

## 9. Adobe 寄りの成果物

Adobe との相互運用のため、v1.0 には次を含めます。

- MOV / ProRes 422 HQ / PCM
- MOV / ProRes 4444 / PCM
- PNG Sequence / RGBA

これらは、Adobe 中心のワークフローで最も安全な編集 / intermediate 出力です。

## 10. Logging

すべての export では次を log します。

- 選ばれた export family
- 選ばれた distribution profile
- 計算された具体的 bitrate / settings
- runtime path
- hardware path を試したかどうか
- fallback が起きたかどうか
- 必要に応じた provider の stderr / stdout 要約
