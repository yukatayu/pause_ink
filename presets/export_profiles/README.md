# Export Profile 定義

このディレクトリには、container/codec family とは別レイヤーの distribution profile を置く。

目的:

- platform 固有の推奨値を UI から切り離す
- family ごとの codec ロジックを profile 側へ漏らさない
- 開発者が profile を追加しやすくする

現在の基本 schema:

- `id`: 安定 ID
- `display_name`: UI 表示名
- `source_kind`: `official` または `app_authored`
- `source_urls`: 参照した公開情報 URL 一覧
- `compatibility`: `"any"` または family ID 配列
- `notes`: UI/開発者向け補足
- `public_constraints`: 公開制約の最小表現
- `settings_buckets`: 解像度や用途別の具体設定テンプレート

`settings_buckets` で使う主な項目:

- `target_video_bitrate_kbps`
- `max_video_bitrate_kbps`
- `audio_bitrate_kbps`
- `sample_rate_hz`
- `keyframe_interval_seconds`
- `preferred_audio_codecs`

開発者が新しい profile を追加する手順:

1. 新しい `.json5` ファイルをこのディレクトリへ追加する
2. `compatibility` と `settings_buckets` を埋める
3. `cargo test -p pauseink-presets-core` で loader と catalog を確認する
4. `manual/developer_guide.md` に必要な補足を追記する
