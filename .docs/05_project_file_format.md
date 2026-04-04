# プロジェクトファイル形式

## 1. 拡張子と encoding

- 拡張子: `.pauseink`
- encoding: UTF-8 text

## 2. 構文の前提

project 形式は JSON5 風テキストです。

- load 時にコメントを受け付ける
- load 時に trailing comma を受け付ける
- 選んだ parser が対応していれば、unquoted key も許容してよい
- 正規化された save は安定した canonical formatting を出す

## 3. 基本方針

- 人間が読める
- 人間が編集できる
- load 時は寛容
- save 時は予測可能
- 可能な範囲で未知フィールドを保持する

## 4. Save 正規化の目標

save 時には次を行います。

- フィールドを文書化された順序にそろえる、または安定化する
- 必要に応じて数値表現を正規化する
- 一時的な runtime-only state を取り除く
- コメント保持に対応した保存モデルなら残し、そうでなければ制約を明記する
- 決定的な output を出す

## 5. 未知フィールド

未知フィールドの保持は望ましいです。

最低要件は次の通りです。

- 未知フィールドで crash しない
- 可能なら memory 上に保持する
- できるなら書き戻す
- 保持できない未知フィールドがあるなら、その制約を正確に記録する

## 6. 現行 top-level shape

```json
{
  "format_version": "1.0.0",
  "project": {
    "metadata": {},
    "media": {},
    "settings": {},
    "pages": [],
    "strokes": [],
    "objects": [
      {
        "id": "object_0001",
        "stroke_ids": ["stroke_0001"]
      }
    ],
    "groups": [],
    "clear_events": [],
    "presets": {}
  }
}
```

これは例示であり、最終固定の syntax そのものではありません。

## 7. Autosave / recovery

autosave は portable root 配下の別ファイルとして扱い、明示的な save がない限り main project path を上書きしてはいけません。

## 8. Settings

settings は project data とは別で、portable config に保存します。  
再現性に関わる project-specific settings は project file 側に置きます。

## 9. History depth

runtime settings file には、設定可能な bounded history depth を含めなければなりません。既定値は 256 です。

## 10. 例

`samples/minimal_project.pauseink` を参照してください。
