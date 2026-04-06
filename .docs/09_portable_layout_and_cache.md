# ポータブル layout と cache policy

## 1. 既定の portable root

既定の mutable root は次の通りです。

```text
<executable dir>/pauseink_data/
```

この root に、アプリが管理する mutable state をすべて置きます。

## 2. 推奨ディレクトリ構成

```text
pauseink_data/
  config/
    settings.json5
  cache/
    google_fonts/
    font_index/
    media_probe/
    thumbnails/
  logs/
  autosave/
  runtime/
    ffmpeg/
  temp/
```

正確な名前は多少変わっても構いませんが、考え方は保ってください。

## 3. locality rule

既定では、アプリ管理の mutable state を portable root の外へ書き出してはいけません。

## 4. 開発 / テスト用 override

CI や repo-local の temp root へ state を分離するため、developer / test 専用の override 環境変数は認めます。

## 5. cache の挙動

### 5.1 Google Fonts cache

- ダウンロードした asset は portable root 配下に cache する
- 壊れたダウンロードは無視または cleanup してよい
- 1 つの壊れた family で UI 全体を止めない

### 5.2 probe cache

- media probe 結果を、file path + metadata signature を鍵にして cache する

### 5.3 thumbnail cache

- bounded
- 利用者が消去可能

## 6. cleanup ツール

v1.0 には、少なくとも基本的な cache manager dialog または action を入れます。次ができること。

- 主な cache category を表示する
- 選択した category を消去する
- 可能なら概算サイズを示す

## 7. logging

log も portable root 配下に置きます。  
既定で OS のグローバル log へ撒き散らしてはいけません。
