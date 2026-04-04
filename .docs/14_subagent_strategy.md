# サブエージェント方針

利用者は、sub-agent をむやみに増やさず、意図して使うことを望んでいます。

## 必須の sub-agent パス

### パス1 — architecture sanity review

タイミング:

- 初期の workspace / module 方針ができた後
- 重い実装を始める前

確認してもらう内容:

- 境界が妥当か
- concurrency / state ownership が妥当か
- クロスプラットフォームで痛みそうな点は何か

### パス2 — media/export/licensing sanity review

タイミング:

- provider abstraction と export family のたたき台ができた後
- export behavior を固定する前

確認してもらう内容:

- runtime / provider の分離が妥当か
- export family の分離が妥当か
- compliance 上の落とし穴は何か
- Adobe compatibility は妥当か

### パス3 — final QA/docs sanity review

タイミング:

- アプリがほぼ完成した段階
- manuals と report の下書きができた段階

確認してもらう内容:

- docs と code の不一致検出
- 足りない test の可能性
- UI の不自然な不整合
- implementation report の未完部分

## ルール

- concurrency は低く保つ（1〜2）
- 結果を待つ
- 明らかに無関係な orphan sub-agent は終了する
- 見つかった内容は implementation report に記録する
