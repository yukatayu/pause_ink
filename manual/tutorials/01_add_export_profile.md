# チュートリアル 01 — export profile を追加する

## 目的

PauseInk の export UI に、新しい distribution profile を追加します。  
family 側の codec/container ロジックは変えず、宣言ファイルだけで profile を増やす流れを確認します。

## 前提

- profile ファイルは `presets/export_profiles/*.json5`
- loader は `pauseink-presets-core` の `ExportCatalog::load_builtin_from_dir`
- UI は起動時に `presets/export_profiles/` を走査して読み込みます

## 例: `my_delivery.json5` を追加する

1. `presets/export_profiles/` に新しい `.json5` を作る
2. `id`、`display_name`、`compatibility`、`settings_buckets` を埋める

例:

```json5
{
  id: "my_delivery",
  display_name: "社内共有",
  source_kind: "app_authored",
  compatibility: ["webm_vp9_opus", "mp4_av1_aac"],
  notes: "社内レビュー向けの軽量 preset。",
  settings_buckets: {
    "1080p": {
      target_video_bitrate_kbps: 6000,
      max_video_bitrate_kbps: 8000,
      audio_bitrate_kbps: 128,
      sample_rate_hz: 48000,
      keyframe_interval_seconds: 2,
      preferred_audio_codecs: ["aac", "libopus"],
    },
  },
}
```

## 検証

1. catalog loader を確認する

```bash
cargo test -p pauseink-presets-core
```

2. app 側の export UI まで含めて compile する

```bash
cargo check -p pauseink-app --all-targets
```

3. display が使える環境ならアプリを起動し、右ペイン `書き出し` の `配布 profile` に新 profile が出ることを確認する

```bash
cargo run -p pauseink-app
```

headless 環境では `cargo check -p pauseink-app --all-targets` までを自動検証とし、UI 確認は表示環境で行う。

## 実装上の注意

- family は増やさず、profile だけを増やしたい時は `compatibility` で対象 family を限定する
- `カスタム` と違い、非 custom profile の数値欄は UI から編集されない
- exact resolution bucket が必要な場合は `"1080x1920"` のような bucket を追加する

## この repository で実際に使った確認コマンド

- `cargo test -p pauseink-presets-core`
- `cargo check -p pauseink-app --all-targets`
- `cargo check -p pauseink-app --all-targets`
