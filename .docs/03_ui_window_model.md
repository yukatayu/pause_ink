# 画面とウィンドウモデル

## 1. ウィンドウ方針

v1.0 は **単一のメインウィンドウ** を使います。

理由は次の通りです。

- クロスプラットフォームの複雑さを下げる
- 分離した window の同期バグを避ける
- 永続化 / 復元を単純にする
- Codex 実装時のリスクを下げる

v1.0 では detached な floating tool window は不要です。

## 2. メインウィンドウのレイアウト

推奨レイアウトは次の通りです。

```text
+----------------------------------------------------------------------------------+
| メニュー / project / transport / page 情報 / status                            |
+----------------------+------------------------------------+----------------------+
| 左レール             | 中央 canvas                         | Inspector            |
| - Media              | - 動画 preview                      | - Selection          |
| - Template           | - Overlay preview                  | - Style              |
| - Fonts              | - Capture interaction              | - Entrance           |
| - Presets            |                                    | - Post actions       |
|                      |                                    | - Template settings  |
+----------------------+------------------------------------+----------------------+
| 下部タブ: Object Outline | Page Events | Export Queue | Logs                    |
+----------------------------------------------------------------------------------+
```

## 3. 必要な panel

### 3.1 左レール

区分は次の通りです。

- **Media**: file import、metadata 概要、runtime diagnostics
- **Template**: テキスト入力、underlay mode、slot 関連操作
- **Fonts**: local fonts、Google Fonts、refresh、壊れた entry の表示切替
- **Presets**: built-in / user preset の閲覧

### 3.2 Inspector

文脈依存です。

想定する区分は次の通りです。

- Selection summary
- Base style
- Entrance
- Reveal-head effect
- Post-actions
- Group 情報
- Transform
- template mode 中の template placement 設定

### 3.3 下部タブ

- **Object Outline**
- **Page Events**
- **Export Queue**
- **Logs**

## 4. モーダルダイアログ

v1.0 で許可するのは次の通りです。

- Project を開く
- Save As
- Media を import
- Export
- Preferences
- Font manager / font refresh
- Cache manager
- runtime / codec provider 不足情報
- recovery prompt
- Error dialog

モーダルを増やしすぎないでください。

## 5. Transport controls

必要な操作は次の通りです。

- play
- pause
- seek bar
- current time
- 可能なら frame / short-step 操作
- clear 挿入

Insert Clear は pause 中でも再生中でも動作しなければなりません。

## 6. 見えているべき状態

UI では次をすぐに見分けられるようにします。

- 現在の page boundary
- 現在生存している object
- 現在の selection
- 現在の template mode
- GPU preview が有効か、無効か、fallback か
- media HW accel が有効か、利用不可か、無効か
- 現在の export path が hardware-assisted か software fallback か

## 7. v1.0 で避ける複雑さ

次のものは追加しません。

- scripting editor
- floating property window
- partial-clear target UI
- ネストした multitrack media timeline
- object ごとの exit track editor
