# テスト方針と完了条件

## 1. テストの考え方

利用者は明示的に次を好んでいます。

- 丁寧な段階的開発
- 強い unit coverage
- やり直しの最小化
- 正直な検証ログ

そのため、実装は test-heavy かつ incremental に進めます。

## 2. 必須の unit test 領域

少なくとも次をカバーします。

- project の parse / normalize / save
- 未知フィールドの保持
- command model と undo / redo
- clear event の意味論
- grouping / ungrouping
- guide geometry
- template slot の生成
- profile resolution
- preset の inheritance / override / reset
- portable path resolution
- media capability parsing（mock 可能な範囲）
- hardware fallback の選択ロジック
- smoothing helper の数理
- snapshot / job の分離ロジック

## 3. 必須の integration / smoke 領域

少なくとも次を検証します。

- project を作る -> save -> reopen -> compare
- media を import して -> 注釈して -> clear して -> save
- composite export
- transparent export
- Google Fonts の graceful failure
- portable root の locality
- tutorial sample の挙動

## 4. Golden / reference test

必要に応じて golden test を使います。対象は次の通りです。

- canonical な project save output
- profile 計算表
- CPU compositor の出力（実用的なら）
- guide geometry の参照ケース

## 5. 失敗ログの要件

目立った失敗はすべて `docs/implementation_report_v1.0.0.md` に記録します。

## 6. 完了条件

次の条件をすべて満たしたときだけ、プロジェクトは完了です。

- host build が成功する
- core tests が通る
- 少なくとも 1 回の end-to-end composite export を検証済み
- 少なくとも 1 回の transparent export を検証済み
- docs が実態と一致している
- tutorial sample を検証済み
- Windows build の試行が記録されている
- 既知の制限が正直に書かれている
