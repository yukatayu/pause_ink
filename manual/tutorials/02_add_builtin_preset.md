# チュートリアル 02 — built-in style preset を追加する

## 目的

PauseInk の built-in base style preset を 1 つ追加し、右ペイン `組み込み preset` から適用できることを確認します。

## 前提

- preset ファイルは `presets/style_presets/*.json5`
- loader は `pauseink-presets-core` の `load_base_style_presets_from_dir`
- 現 UI で適用しているのは `thickness` と `color_rgba`

## 例: 新しい preset を追加する

1. `presets/style_presets/soft_blue_note.json5` を作る

```json5
{
  id: "soft_blue_note",
  display_name: "やわらか青メモ",
  base_style: {
    thickness: 8.0,
    color_rgba: [0.45, 0.72, 1.0, 0.95],
  },
  entrance: {
    kind: "instant",
    target: "group",
  },
}
```

2. loader test を通す

```bash
cargo test -p pauseink-presets-core
```

3. display が使える環境なら app を起動して右ペインから適用する

```bash
cargo run -p pauseink-app
```

確認点:

- `組み込み preset` の一覧に追加した名前が出る
- `preset を適用` を押すと `太さ` と色が変わる

headless 環境では `cargo check -p pauseink-app --all-targets` までを自動検証とし、UI 確認は表示環境で行う。

## 実装上の注意

- 現 v1.0 実装では entrance / clear / combo の宣言は将来拡張のために置けるが、UI が直接反映するのは base style の一部だけ
- project へ実際に反映されるのは、その preset 適用後に新しく描いた stroke
- 既存 stroke を一括再解決する機能はまだ入れていない

## この repository で実際に使った確認コマンド

- `cargo test -p pauseink-presets-core`
- `cargo check -p pauseink-app --all-targets`
