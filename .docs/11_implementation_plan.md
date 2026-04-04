# 詳細実装計画

この計画は意図的に細かくしています。  
Codex は順序を少し調整して構いませんが、全体の規律は飛ばさないでください。

## フェーズ0 — 環境とレポートの初期化

1. repository の docs をすべて読む。
2. 環境情報を implementation report に記録する。
3. `progress.md` を live tracker に書き直す。
4. 初期 crate / module 分割を決める。
5. 初期方針を implementation report に書く。
6. 最初の architecture sanity-review sub-agent を起動し、結果を待つ。
7. sub-agent の指摘を採用 / 見送りで明示的に整理する。

## フェーズ1 — workspace と scaffolding

8. workspace member を確定する。
9. 最小限 compile できる crate をつなぐ。
10. 基本的な lint / test コマンドを足す。
11. logging / tracing の土台を入れる。
12. 共通の error / result 規約を入れる。
13. serialization 補助 utility を足す。
14. 必要なら test utilities crate / module を足す。
15. skeleton が build できることを確認する。
16. 実行したコマンドと結果を記録する。

## フェーズ2 — domain model

17. core ID と time unit を定義する。
18. stroke model を定義する。
19. glyph object model を定義する。
20. group model を定義する。
21. clear event model を定義する。
22. page の導出規則を定義する。
23. z-order と reveal order の分離を定義する。
24. transform を定義する。
25. style snapshot model を定義する。
26. entrance model を定義する。
27. post-action model を定義する。
28. preset reference と resolved snapshot model を定義する。
29. page 導出の unit test を追加する。
30. clear semantics の unit test を追加する。

## フェーズ3 — project format

31. JSON5-style load と normalized save の parser / writer stack を選ぶ。
32. versioned project schema を定義する。
33. 寛容な load を実装する。
34. normalized save を実装する。
35. unknown-field 保持戦略を実装する。
36. save output の golden test を追加する。
37. roundtrip test を追加する。
38. malformed input の耐性 test を追加する。
39. コメント保持が完全でないなら、その制約を正確に書く。

## フェーズ4 — portable filesystem

40. executable-local root 解決を実装する。
41. developer / test 用 override 環境変数を追加する。
42. config / cache / logs / autosave / runtime / temp の subdirectory を実装する。
43. settings の load / save を実装する。
44. history depth 設定を実装する。
45. path test を実装する。
46. log rotation か基本的な log cleanup policy を実装する。
47. directory layout を docs / report に記録する。

## フェーズ5 — command model と undo / redo

48. command trait / model を定義する。
49. dispatcher を実装する。
50. reversible command を実装する。
51. bounded undo stack を実装する。
52. bounded redo stack を実装する。
53. depth limit の test を追加する。
54. redo invalidation の test を追加する。
55. grouped command の test を追加する。

## フェーズ6 — presets と profile 基盤

56. style preset の schema を定義する。
57. export profile の schema を定義する。
58. built-in preset / profile の読み込みを実装する。
59. user preset の overlay 読み込みを実装する。
60. resolved snapshot の意味を実装する。
61. inheritance / override / reset の test を追加する。
62. export profile resolution の test を追加する。
63. 新しい profile / preset の追加方法を developer guide に書く。

## フェーズ7 — fonts と template catalog

64. local font discovery を実装する。
65. Google Fonts の configured-family 処理を実装する。
66. portable root 配下で cache / download / index を実装する。
67. 壊れた font に対する graceful failure を実装する。
68. template settings model を実装する。
69. font cache path と bad-entry handling の test を追加する。
70. network / cache policy を正確に記録する。

## フェーズ8 — template layout

71. grapheme-aware slot 生成を実装する。
72. kana scale を実装する。
73. latin scale を実装する。
74. punctuation scale を実装する。
75. tracking を実装する。
76. line height を実装する。
77. slope angle を実装する。
78. underlay mode を実装する。
79. placement preview を実装する。
80. slot の commit / advance / cancel を実装する。
81. slot 生成と scaling rule の test を追加する。

## フェーズ9 — media provider 層

82. provider trait / interface を定義する。
83. runtime discovery を実装する。
84. ffprobe ベースの metadata probing を実装する。
85. encoder / muxer capability probing を実装する。
86. diagnostics と provider error を実装する。
87. path resolution と capability parsing の mock test を追加する。
88. export / licensing sanity-review sub-agent を起動して待つ。
89. 指摘を統合して記録する。

## フェーズ10 — playback 基盤

90. media import flow を実装する。
91. playback state model を実装する。
92. current time source を実装する。
93. seek / play / pause を実装する。
94. frame / canvas coordinate mapping を実装する。
95. coordinate conversion math の test を追加する。
96. media loading の smoke check を追加する。

## フェーズ11 — free ink capture

97. raw point capture を実装する。
98. raw sample を保持する。
99. derived-path regeneration を実装する。
100. stabilization control を実装する。
101. selection を実装する。
102. free ink の shift-grouping を実装する。
103. raw / derived の保持 test を追加する。
104. grouping semantics の test を追加する。

## フェーズ12 — guide system

105. platform ごとの guide modifier default を実装する。
106. guide reference capture を実装する。
107. guide geometry calculation を実装する。
108. horizontal 3-line + helper lines を実装する。
109. next-character vertical guide set を実装する。
110. slope persistence を実装する。
111. guide が export されないことを保証する。
112. guide geometry と export 除外の test を追加する。

## フェーズ13 — outline、group、page event

113. object outline tree を実装する。
114. derived run を実装する。
115. split / merge の表示ロジックを実装する。
116. group / ungroup を実装する。
117. z-order 操作を実装する。
118. alive の強調表示を実装する。
119. auto-follow-current を実装する。
120. page event track を実装する。
121. run 導出と group 伝播の test を追加する。

## フェーズ14 — style、entrance、clear effect

122. base style editor model を実装する。
123. entrance kind を実装する。
124. reveal-head effect を実装する。
125. post-action chain を実装する。
126. clear event 挙動を実装する。
127. style preset の適用を実装する。
128. effect timing 計算の test を追加する。
129. clear sequencing の test を追加する。

## フェーズ15 — export 設定 UI と engine

130. family + profile の選択を実装する。
131. profile data から具体的設定を計算する。
132. non-Custom では無効化された計算済み欄を出す。
133. Custom では直接編集を有効にする。
134. CPU 安全な overlay composition を実装する。
135. 可能なら frame を FFmpeg に流し込む。
136. hardware-try / software-fallback 経路を実装する。
137. 設定計算と fallback の test を追加する。
138. composite / transparent の smoke export を少なくとも 1 つずつ追加する。

## フェーズ16 — preferences、cache manager、recovery

139. Preferences dialog を実装する。
140. cache manager を実装する。
141. autosave cadence と recovery prompt を実装する。
142. runtime diagnostics view を実装する。
143. settings persistence と recovery flow の test を、可能な範囲で追加する。

## フェーズ17 — manual、tutorial、polish

144. user manual を実態に合わせる。
145. developer guide を実コードに合わせる。
146. developer tutorial sample を実装する。
147. tutorial sample を build / run / test で検証する。
148. final QA/docs sub-agent を起動して待つ。
149. QA/docs review で見つかった不整合を修正する。
150. implementation report を十分に更新する。

## フェーズ18 — 最終検証

151. full test suite を実行する。
152. end-to-end save / load を検証する。
153. end-to-end composite export を検証する。
154. end-to-end transparent export を検証する。
155. 可能なら Linux から Windows build を試す。
156. cross-build が不完全なら blocker を正確に記録する。
157. known limitation を確定する。
158. packaging / licensing notes を確定する。
159. `progress.md` を確定する。
160. done criteria を満たしたときだけ止める。
