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
  - `全消去`
- transport bar
  - `再生` / `一時停止`
  - seek スライダー
  - 現在位置 / 全長表示
- 左ペイン
  - メディア情報
  - テンプレート入力と配置操作
  - 読み込み済みフォント一覧
  - Google Fonts の cache 状態
- 右ペイン
  - タイトル
  - built-in / user style preset の適用と管理
  - 基本スタイル
  - 効果
  - 出現
  - ガイド
  - 書き出し
- 下部タブ
  - `オブジェクト一覧`
  - `ページイベント`
  - `書き出しキュー`
  - `ログ`
  - 下端の境界をドラッグして高さを変えられます
  - `内容幅` で内容の基準幅を広げられます
  - 内容が増えても下部パネルの高さは自動では伸びず、必要ぶんはスクロールで確認します
- 中央
  - 動画 preview と注釈 overlay を重ねたキャンバス

## 3. 基本ワークフロー

### 3.1 新規作業

1. `メディア読込` で動画を開く
2. 上部直下の transport bar で再生 / 一時停止とシークを行い、位置を合わせる
3. 中央キャンバスへそのまま描く
4. ページを切りたい位置で `全消去` を押す
5. `保存` で `.pauseink` を保存する

### 3.2 free ink

- キャンバス上でドラッグすると stroke を記録します
- 押した瞬間の位置から未確定 stroke がその場で表示されます。太さも確定後の overlay と同じ縮尺に合わせています。手ブレ補正の結果により、描き終わった直後にわずかに形が整うことがあります
- `Ctrl+Z` で元に戻す、`Ctrl+Shift+Z` または `Ctrl+Y` でやり直せます
- 直前の object へ stroke を継ぎ足す処理は app session 側にありますが、現 UI では個別の切替ボタンは未露出です
- 色、太さ、不透明度、手ブレ補正は右ペイン `基本スタイル` で調整します
- アウトライン、ドロップシャドウ、グロー、ブレンドモードも右ペインから調整できます
- 色 picker は色相変更用で、透明度は `不透明度` スライダー 1 つに統一されています
- 同じテンプレート slot や guide 参照文字へ stroke を継ぎ足す場合も、次に確定する stroke から最新の `基本スタイル` が反映されます
- 右ペインでは built-in preset を選んで適用できます。現在の基本スタイルは `追加保存` / `上書き保存` / `削除` で user preset として管理できます

### 3.3 ガイド capture

- ガイド修飾キーを押しながら 1 文字ぶん描くと、その bounding box を元に guide が生成されます
- 修飾キーを押しているあいだは複数 stroke を同じ参照文字として扱い、修飾キーを離した時点で guide が確定します
- 既定の修飾キーは Linux / Windows では `Ctrl`、macOS では `Alt`
- 修飾キーは `設定` ウィンドウで上書きできます
- 横ガイドは現在の表示領域の左右端まで伸びます
- 直前の文字を書いたあとに修飾キーを短く押すと、横線はそのままで次文字用の縦ガイドだけを先へ送れます
- `Ctrl+Z`、`Ctrl+Shift+Z`、`Ctrl+Y` などのショートカットで修飾キーを使っても、ガイド送りは発生しません
- 次文字用の縦ガイド幅は一定で、位置だけ直前文字の右端に合わせます
- `ガイド解除` で現在の guide と capture 文脈をまとめて解除できます

### 3.4 テンプレート配置

1. 左ペイン `テンプレート` に文字列を入れる
2. `テンプレート font` dropdown から読み込み済み family を選ぶ
3. フォントサイズ、字間、傾きを調整する
4. 字幅と字詰めは選択 font の shaping と kerning を使って preview されます
5. `テンプレート配置` を押す
6. キャンバス上で配置位置をクリックする
7. 表示された slot box をなぞるように手書きする
8. `前スロット` / `次スロット` で slot を移動する
9. `テンプレート解除` で preview を消す

補足:

- 配置待ち中は preview だけが動き、stroke は書かれません
- 配置済みでも、文字列、フォント、フォントサイズ、字間、傾きを変えると slot box はその設定で再計算されます
- 配置後の slot box は傾き設定に合わせて回転表示されます
- template underlay は手書き stroke の下に描かれます
- 左右ペインの境界もドラッグして幅を変えられます

## 4. 保存と復旧

- プロジェクト形式は `.pauseink`
- load は lenient、save は normalized
- project ごとに、現在の基本スタイル snapshot、現在の出現設定 snapshot、選択 preset ID、テンプレート文字列 / font / font size / 字間 / 傾き / underlay、ガイド傾きが保存されます
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
- 実行中は `実行中:` 表示の下に stage 名と progress bar が出ます
- 動画 export では frame 生成後も ffmpeg encode の進行に合わせて progress が 92% 以降も更新されます
- `99%` 付近では `最終処理中` と表示されることがあります。実際の完了は `書き出し完了` が出た時点です
- 実行中ジョブと履歴は下部 `書き出しキュー` に出ます
- PNG Sequence は注釈 overlay の RGBA 連番を書き出します
- 右ペイン `出現` では、方式、時間モード、時間、出現速度を調整できます
- 固定時間では指定時間をベースに、長さ比例では stroke 長に応じた時間をベースにして出現します

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
  - 最後の検出エラー
  - encoder / muxer / hwaccel 一覧
  - Windows / macOS / Linux ごとの runtime 再検出と配置案内

## 8. Google Fonts

- `設定` の Google Fonts 欄で family 名を追加します
- `取得` を押すと Google Fonts CSS2 API から取得し、portable cache に保存します
- 失敗しても他の UI は止まりません
- `キャッシュ削除` で個別削除できます

## 9. portable data

PauseInk は既定で executable 直下に `pauseink_data/` を作ります。

主な中身:

- `config/settings.json5`
- `config/style_presets/`
- `cache/google_fonts/`
- `autosave/`
- `runtime/`
- `temp/`

`config/settings.json5` はアプリ全体の設定です。  
project ごとに再現したい style / template / font / guide 状態は `.pauseink` 側へ保存されます。  
右ペインから保存した user preset は `config/style_presets/` に置かれます。

## 10. 現時点の既知制約

- template 字詰めは実 font shaping と kerning を使いますが、scale が切り替わる run 境界では font engine 上の自然な区切りに従います
- style preset は現在、厚み / 色 / 不透明度 / 手ブレ補正に加え、outline / drop shadow / glow / blend mode / 出現方式 / 出現速度まで保存と適用ができます
- reveal-head effect、post-action chain、clear / combo preset の専用 UI はまだ入っていません
- group / ungroup / multi-select / z-order UI はまだ最小です
- Windows と macOS はこの Linux ホスト上で実行確認しておらず、runtime 探索ロジックは unit test で検証しています
- GUI の `eframe` deprecation warning が残っていますが、現 build/test は通っています
