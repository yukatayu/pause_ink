# Style preset 定義

このディレクトリには、PauseInk が起動時に読み込む built-in base style preset を置きます。

現 v1.0 実装で読み込む主項目:

- `id`
- `display_name`
- `base_style.thickness`
- `base_style.color_rgba`

現時点では `entrance` / `clear` / `combo` の宣言をファイルへ置くことはできますが、UI が直接適用しているのは base style の一部です。  
将来の preset 拡張時にも file layout を崩しにくいよう、example file では関連 block を残しています。

開発者向け手順:

1. `.json5` を追加する
2. `cargo test -p pauseink-presets-core` を実行する
3. `cargo run -p pauseink-app` で右ペイン `組み込み preset` に現れることを確認する
