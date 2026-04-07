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
  - 見出しは固定、長い設定群は縦スクロール
- 右ペイン
  - タイトル
  - built-in / user style preset の適用と管理
  - built-in / user entrance preset の適用と管理
  - 基本スタイル
  - 効果
  - 出現
  - ガイド
  - 書き出し
  - 見出しは固定、長い設定群は縦スクロール
- 下部タブ
  - `オブジェクト一覧`
  - `ページイベント`
  - `書き出しキュー`
  - `ログ`
  - object / group をクリックで選択、`Ctrl` または `Cmd` 併用クリックで複数選択できます
  - `グループ化` / `グループ解除` / `背面へ` / `一つ後ろ` / `一つ前` / `前面へ` をここから実行できます
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
- 一時停止中に今書いた paused batch は preview 上で即座に全表示されます。既にある batch は現在時刻ベースの reveal のまま見えます
- `再生`、`保存`、`書き出し` を行う時点で paused batch は通常の timeline object として扱い直され、preview 専用の全表示 override は外れます
- 再生中は書き込みを受け付けません。書く前に一時停止してください
- `Ctrl+Z` で元に戻す、`Ctrl+Shift+Z` または `Ctrl+Y` でやり直せます
- 直前の object へ stroke を継ぎ足す処理は app session 側にありますが、現 UI では個別の切替ボタンは未露出です
- 色、太さ、不透明度、手ブレ補正は右ペイン `基本スタイル` で調整します
- アウトライン、ドロップシャドウ、グロー、ブレンドモードも右ペインから調整できます
- 色 picker は色相変更用で、透明度は `不透明度` スライダー 1 つに統一されています
- 同じテンプレート slot や guide 参照文字へ stroke を継ぎ足す場合も、次に確定する stroke から最新の `基本スタイル` が反映されます
- 右ペインでは style preset と entrance preset を別々に選んで適用できます
- style preset は基本スタイルだけを、entrance preset は出現設定だけを扱います
- 現在の基本スタイルと出現設定は、それぞれ `追加保存` / `上書き保存` / `削除` で user preset として管理できます
- `オブジェクト一覧` で複数 object または group を選んでいる場合、右ペインの style / entrance 操作はその選択全体へまとめて適用されます
- style / entrance の各項目には `preset 継承中` と `上書き中` の表示があり、`presetへ戻す` でその項目だけ継承へ戻せます
- フォント、テンプレート、ガイド傾き、基本スタイル、outline / drop shadow / glow、entrance 設定は `settings.json5` に保存され、次回起動時に復元されます
- 同じ page で、同じ style / entrance のまま連続して確定した object は auto-group として自動でまとまります
- guide 基準更新、guide 解除、template の再配置 / reset、clear、undo / redo を挟むと、その auto-group chain はそこで切れます

### 3.3 ガイド capture

- ガイド修飾キーを押しながら 1 文字ぶん描くと、その bounding box を元に guide が生成されます
- 修飾キーを押しているあいだは複数 stroke を同じ参照文字として扱い、修飾キーを離した時点で guide が確定します
- 既定の修飾キーは Linux / Windows では `Ctrl`、macOS では `Alt`
- 修飾キーは `設定` ウィンドウで上書きできます
- 横ガイドは現在の表示領域の左右端まで伸びます
- 直前の文字を書いたあとに修飾キーを短く押すと、横線はそのままで次文字用の縦ガイドだけを先へ送れます
- このときの「直前の文字」は、前回 guide を確定または送ったあとから今までに確定した stroke 全体の外接矩形で判定します。多画の文字でも最後の 1 画だけは見ません
- `Ctrl+Z`、`Ctrl+Shift+Z`、`Ctrl+Y` などのショートカットで修飾キーを使っても、ガイド送りは発生しません
- 次文字用の縦ガイド幅は一定で、位置だけ直前文字の右端に合わせます
- `次文字字間` は `cell_width` 比で効きます。`0.25` なら 1 マス幅の 25% だけ右へ空け、負値では少し食い込ませられます
- `ガイド解除` で現在の guide と capture 文脈をまとめて解除できます
- `Esc` は、まず開いている popup / 設定 window を閉じます。閉じる window が無いときは、現在の guide を解除します
- ただし text 入力欄に focus がある間は、`Esc` による global cancel を横取りしません

### 3.4 オブジェクト一覧での編集

- 下部 `オブジェクト一覧` の object 行をクリックすると、その object を選択します
- Linux / Windows では `Ctrl`、macOS では `Cmd` を押しながらクリックすると複数選択を追加 / 解除できます
- group 行をクリックすると group を選択し、その group に含まれる object 群をまとめて操作対象にできます
- `グループ化` は選択対象を flat な 1 group へ統合します。group 同士をまとめた場合も、入れ子は作らず member を merge します
- `グループ解除` は選択中 group を外し、member object を再び個別 selection に戻します
- `背面へ` / `一つ後ろ` / `一つ前` / `前面へ` は、選択 object または選択 group の member object をまとめて前後移動します
- 右ペインの基本スタイル、効果、出現 preset / 出現設定は、通常時は選択中 object 群全体へ同じ値を適用します
- template 配置中や guide capture 中は、その場で編集中の object を優先して更新します

### 3.5 テンプレート配置

1. 左ペイン `テンプレート` に文字列を入れる。`Enter` で改行できます
2. `テンプレート font` dropdown から読み込み済み family を選ぶ
3. フォントサイズ、字間、傾きを調整する
4. 必要なら `テンプレート詳細` を開き、行間、かな倍率、英字倍率、句読点倍率、下敷き表示を調整する
5. 字幅と字詰めは選択 font の shaping と kerning を使って preview されます
6. 縦位置は font metrics が取れる family では ascent / descent / x-height / cap-height を使って揃えます。取れない場合も、縮小した英字や句読点は line box の下側へ寄せます
7. `テンプレート配置` を押す
8. キャンバス上で配置位置をクリックする
9. 表示された slot box をなぞるように手書きする
10. `テンプレート解除` で preview を消す

補足:

- 配置待ち中は preview だけが動き、stroke は書かれません
- テンプレート文字入力欄は初期状態で 2 行分あり、右下ドラッグで高さを変えられます
- `テンプレート詳細` は別 window で開きます。値を動かすと、配置済み slot と preview はその場で再計算されます
- 配置済みでも、文字列、フォント、フォントサイズ、字間、行間、かな倍率、英字倍率、句読点倍率、傾き、下敷き表示を変えると slot box はその設定で再計算されます
- 配置後の slot box は傾き設定に合わせて回転表示されます
- 縮小した英字は baseline を意識して下側へ揃え、`y` など descender を持つ文字は `x` より少し下へ伸びます
- slot 番号や current slot の強調表示は出しません。template は見えている下敷きを順に手書きする前提です
- template underlay は手書き stroke の下に描かれます
- `Esc` は、まず開いている popup / 設定 window を閉じます。閉じる window が無いときは、template 配置待ちと配置済み preview をまとめて解除します
- 左右ペインの境界もドラッグして幅を変えられます
- 左右ペインは縦方向にスクロールできます。低い画面でも `書き出し` や `Google Fonts 設定` まで辿れます
- シークバーや preset 名入力欄など、広い方が使いやすい入力欄は panel 幅に応じて自然に広がります

## 4. 保存と復旧

- プロジェクト形式は `.pauseink`
- load は lenient、save は normalized
- object group も project に保存され、再読込後も member 関係を維持します
- project ごとに、現在の基本スタイル snapshot、現在の出現設定 snapshot、選択 preset ID、style/entrance の継承状態、テンプレート文字列 / font / font size / 字間 / 行間 / かな倍率 / 英字倍率 / 句読点倍率 / 傾き / underlay、ガイド傾き / 次文字字間が保存されます
- 保存済み project を `開く` と、記録されている media source path が自動で再読込されます。relative path の場合は `.pauseink` 自体があるフォルダ基準で解決します
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
- stage は `1/3 フレーム生成`、`2/3 動画/PNG 書き出し`、`3/3 一時ファイル整理` の順で進みます
- 動画 export では frame 生成後も ffmpeg encode の進行に合わせて progress が 92% 以降も更新されます
- `99%` 付近では `最終処理中` と表示されることがあります。実際の完了は `書き出し完了` が出た時点です
- 各 stage の下には「何を待っているか」の説明文も表示されます
- 実行中ジョブと履歴は下部 `書き出しキュー` に出ます
- PNG Sequence は注釈 overlay の RGBA 連番を書き出します
- 右ペイン `出現` では、方式、時間モード、時間、出現速度を調整できます
- 固定時間では指定時間をベースに、長さ比例では stroke 長に応じた時間をベースにして出現します
- `先端アクセント` を有効にすると、`なぞりがき` / `ワイプ` の再生と書き出しで、書かれた直後の短い区間だけを白寄せ + 発光気味に重ね描きできます
- `先端アクセント` の `色` は `インク色`、`preset アクセント色`、`カスタム色` を切り替えられます。`preset アクセント色` は Glow / Outline / DropShadow の代表色を優先し、無ければインク色へ戻ります
- `即時` と `ディゾルブ` では `先端アクセント` は出ません。timed entrance の recent segment 演出としてだけ効きます
- `即時` はその object の時点ですぐ表示されます
- `なぞりがき` や dissolve / wipe のような時間を使う出現は、同じ page 内では reveal 順に直列化されます。途中に `即時` object があっても、それはすぐ表示され、次の時間付き object は直前の時間付き object が終わってから始まります

## 7. 設定 / キャッシュ / 診断

上部ツールバーから次を開けます。

- `設定`
  - undo 履歴深さ
  - ガイド修飾キー
  - ガイド傾き
  - 次文字字間
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
- `config/entrance_presets/`
- `config/clear_presets/`
- `config/combo_presets/`
- `cache/google_fonts/`
- `autosave/`
- `runtime/`
- `temp/`

`config/settings.json5` はアプリ全体の設定です。  
project ごとに再現したい style / entrance / template / font / guide 状態は `.pauseink` 側へ保存されます。
右ペインから保存した user preset は、style が `config/style_presets/`、entrance が `config/entrance_presets/` に置かれます。

## 10. 現時点の既知制約

- template 字詰めは実 font shaping と kerning を使いますが、scale が切り替わる run 境界では font engine 上の自然な区切りに従います
- style preset は基本スタイル用、entrance preset は出現用として分離されています。legacy な style preset file に entrance が入っていても読み込み時に救済されます
- post-action chain と clear / combo preset の専用 UI はまだ入っていません
- group / ungroup / multi-select / z-order UI は使えますが、現時点では outline panel 起点の最小導線です。canvas 直接選択や高度な tree 編集はまだありません
- Windows と macOS はこの Linux ホスト上で実行確認しておらず、runtime 探索ロジックは unit test で検証しています
- GUI の `eframe` deprecation warning が残っていますが、現 build/test は通っています
