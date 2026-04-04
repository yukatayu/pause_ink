# リスク登録簿

| リスク | 影響 | 対策 |
|---|---|---|
| 端末によって GPU backend が不安定 | preview / export の不具合 | preview GPU と media accel を分け、CPU 安全な baseline を残す |
| 手書きが平滑化されすぎる | 見た目が悪くなる | raw + derived の分離、stabilization test、corner guard |
| project 形式が脆い | 手編集した file が壊れる | 寛容な load、正規化された save、parser test |
| FFmpeg runtime の不一致 | import / export 失敗 | provider diagnostics、sidecar manifest、capability probing |
| Google Fonts の network / cache 失敗 | UX が悪い | graceful skip、cache 済み index、全体失敗にしない |
| クロスプラットフォーム UI の分岐 | layout バグ | 単一ウィンドウ設計、test 可能な state model |
| export preset の乱立 | 保守しにくい | data-driven な profile file と developer docs |
| codec 周辺の licensing 混乱 | 出荷リスク | app/core と runtime / codec policy を分け、packaging を明記する |
| disk usage の増加 | low-storage 問題 | cache manager、bounded cache、cleanup tool |
| concurrency バグ | state の破損や crash | UI thread mutation + immutable worker snapshot |
