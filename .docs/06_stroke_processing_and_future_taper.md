# ストローク処理、stabilization、将来の taper

## 1. v1.0 の要件

- ペン圧 pipeline は持たない
- 利用者が調整できる stabilization は持つ
- raw sample は保持する
- render path は派生させる
- corner はできるだけ残す
- 将来の pressure / taper 作業と両立できる実装にする

## 2. v1.0 に推奨する stabilization 設計

### 2.1 入力の保存

raw point は timestamp 付きで保存します。

### 2.2 派生 path

raw point から stabilization 済み path を生成する際は、次のような手法を使います。

- adaptive One Euro 風フィルタ、または同等の方法
- 急カーブ付近で smoothing を弱める corner / curvature guard
- mesh / path 生成向けの streamline / resampling の任意ステップ

### 2.3 UI

v1.0 の UI は 1 つの数値コントロールで足ります。

- `Stroke stabilization strength`（例: 0–100）

実装内部では次のような値に対応付けても構いません。

- min cutoff
- beta / adaptation strength
- resampling tolerance
- corner guard threshold

## 3. なぜ単純な強い smoothing ではだめか

強い単純 low-pass filter には次の問題があります。

- corner を潰す
- かな / 漢字の構造が曖昧になる
- 速い動きで遅れが見える
- 手書きが「ゴムっぽく」なる

そのため v1.0 では、雑な固定 smoothing だけに頼るべきではありません。

## 4. 将来の自動 taper / pseudo-pressure

将来の `Auto taper` チェックボックスは、明示的に予定されています。

推奨する将来の signal は次の通りです。

- stroke 開始からの距離に基づく start taper
- stroke 終端までの距離に基づく end taper
- speed の影響を受ける synthetic pressure
- corner が潰れないようにする curvature / corner 保護
- 任意の post-corner recovery

### 4.1 重要な結論

「最後の大きなカーブからどれだけ進んだか」だけでは **不十分** です。  
これは主 signal ではなく、補助 signal としてだけ使います。

### 4.2 より良い heuristic の組み合わせ

将来の推奨 heuristic は次の通りです。

- speed
- 正規化された path 進行度
- 局所 curvature
- 開始 / 終端への近さ

## 5. 実装時に参照するとよい先行例

- One Euro Filter
- Google Ink Stroke Modeler
- perfect-freehand

Codex は、最終的な v1.0 実装にどの先行例が影響したかを implementation report に記録してください。
