# PauseInk ユーザーガイド

## 1. アプリの目的

PauseInk は、動画を止めながら手書きで注釈を書き、そのまま再生・保存・書き出しまで行うための single-window アプリです。

v1.0 の前提:

- clear は manual のみ
- clear は screen-wide
- partial clear はなし
- 最終表示はフォントではなくユーザーの stroke data が主体

## 2. 画面の見方

メイン画面は 1 ウィンドウです。

- 上部ツールバー
  - `開く` / `保存` / `別名保存`
  - `メディア読込`
  - `元に戻す` / `やり直す`
  - `再生` / `一時停止`
  - `全消去`
  - 現在位置スライダー
- 左ペイン
  - メディア情報
  - テンプレート入力と配置操作
  - ローカルフォント一覧
  - Google Fonts の cache 状態
- 右ペイン
  - タイトル
  - built-in style preset 適用
  - 基本スタイル
  - ガイド
  - 書き出し
- 下部タブ
  - `オブジェクト一覧`
  - `ページイベント`
  - `書き出しキュー`
  - `ログ`
- 中央
  - 動画 preview と注釈 overlay を重ねたキャンバス

## 3. 基本ワークフロー

### 3.1 新規作業

1. `メディア読込` で動画を開く
2. 必要なら再生 / 一時停止で位置を合わせる
3. 中央キャンバスへそのまま描く
4. ページを切りたい位置で `全消去` を押す
5. `保存` で `.pauseink` を保存する

### 3.2 free ink

- キャンバス上でドラッグすると stroke を記録します
- 直前の object へ stroke を継ぎ足す処理は app session 側にありますが、現 UI では個別の切替ボタンは未露出です
- 太さ、不透明度、手ブレ補正は右ペイン `基本スタイル` で調整します

### 3.3 ガイド capture

- ガイド修飾キーを押しながら 1 文字ぶん描くと、その bounding box を元に guide が生成されます
- 既定の修飾キーは Linux / Windows では `Ctrl`、macOS では `Alt`
- 修飾キーは `設定` ウィンドウで上書きできます
- `ガイド解除` で非表示にできます

### 3.4 テンプレート配置

1. 左ペイン `テンプレート` に文字列を入れる
2. フォントサイズ、字間、傾きを調整する
3. `テンプレート配置` を押す
4. キャンバス上で配置位置をクリックする
5. 表示された slot box をなぞるように手書きする
6. `次スロット` で次の slot に進む
7. `テンプレート解除` で preview を消す

## 4. 保存と復旧

- プロジェクト形式は `.pauseink`
- load は lenient、save は normalized
- autosave は既定で 10 秒ごとです
- 前回の autosave が残っていると起動直後に `復旧` ウィンドウが開きます
- `復旧する` で最新 autosave を読み込み、`破棄する` で削除します

## 5. clear と page の考え方

- `全消去` を押した時点で clear event が timeline に入ります
- clear event 間の区間が 1 page です
- 下部 `ページイベント` タブで clear event 一覧を確認できます

## 6. 書き出し

右ペイン `書き出し` で以下を選びます。

- family
  - container / codec family
- profile
  - 配布先または品質 preset
- 出力種別
  - family が対応していれば `合成` と `透過` を切り替え

動作:

- `カスタム` profile では数値欄を直接編集できます
- それ以外の profile では数値欄は計算結果の表示のみです
- 実行中ジョブと履歴は下部 `書き出しキュー` に出ます
- PNG Sequence は注釈 overlay の RGBA 連番を書き出します

## 7. 設定 / キャッシュ / 診断

上部ツールバーから次を開けます。

- `設定`
  - undo 履歴深さ
  - ガイド修飾キー
  - ガイド傾き
  - プレビュー GPU toggle
  - メディア HW accel toggle
  - autosave 間隔
  - Google Fonts family 管理
  - ローカルフォントフォルダ
- `キャッシュ`
  - Google Fonts / font index / media probe / thumbnails / temp の概算サイズ表示
  - 各カテゴリの削除
- `診断`
  - runtime origin
  - ffmpeg / ffprobe path
  - encoder / muxer / hwaccel 一覧

## 8. Google Fonts

- `設定` の Google Fonts 欄で family 名を追加します
- `取得` を押すと Google Fonts CSS2 API から取得し、portable cache に保存します
- 失敗しても他の UI は止まりません
- `キャッシュ削除` で個別削除できます

## 9. portable data

PauseInk は既定で executable 直下に `pauseink_data/` を作ります。

主な中身:

- `config/settings.json5`
- `cache/google_fonts/`
- `autosave/`
- `runtime/`
- `temp/`

## 10. 現時点の既知制約

- Google Fonts は cache / graceful failure / configured family 管理まで実装済みで、template rendering への厳密な反映は今後の改善余地があります
- built-in style preset は base style の厚みと色の適用が中心です
- group / ungroup / multi-select / z-order UI はまだ最小です
- GUI の `eframe` deprecation warning が残っていますが、現 build/test は通っています
