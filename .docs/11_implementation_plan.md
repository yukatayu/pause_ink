# Detailed implementation plan

This plan is intentionally granular.  
Codex may refine task ordering, but should not skip the overall discipline.

## Phase 0 — environment and report bootstrap

1. Read all repository docs.
2. Capture environment details in the implementation report.
3. Rewrite `progress.md` into a live tracker.
4. Decide initial crate/module split.
5. Record the initial approach in the implementation report.
6. Launch the first architecture sanity-review sub-agent and wait for it.
7. Incorporate/reject sub-agent findings explicitly.

## Phase 1 — workspace and scaffolding

8. Finalize workspace members.
9. Wire minimal compileable crates.
10. Add basic lint/test commands.
11. Add logging/tracing baseline.
12. Add shared error/result conventions.
13. Add serialization helper utilities.
14. Add test utilities crate/module if useful.
15. Ensure the skeleton builds.
16. Record the exact commands and outcomes.

## Phase 2 — domain model

17. Define core IDs and time units.
18. Define stroke model.
19. Define glyph object model.
20. Define group model.
21. Define clear event model.
22. Define page derivation rules.
23. Define z-order vs reveal order separation.
24. Define transforms.
25. Define style snapshot model.
26. Define entrance model.
27. Define post-action model.
28. Define preset reference + resolved snapshot model.
29. Add unit tests for page derivation.
30. Add unit tests for clear semantics.

## Phase 3 — project format

31. Choose parser/writer stack for JSON5-style load and normalized save.
32. Define versioned project schema.
33. Implement lenient load.
34. Implement normalized save.
35. Implement unknown-field preservation strategy.
36. Add golden tests for save output.
37. Add roundtrip tests.
38. Add malformed-input tolerance tests.
39. Record exact limitations if comment preservation is imperfect.

## Phase 4 — portable filesystem

40. Implement executable-local root resolution.
41. Add developer/test override env variable.
42. Implement subdirectories for config/cache/logs/autosave/runtime/temp.
43. Implement settings load/save.
44. Implement history-depth setting.
45. Implement path tests.
46. Implement log rotation or basic log cleanup policy.
47. Record directory layout in docs/report.

## Phase 5 — command model and undo/redo

48. Define command trait/model.
49. Implement dispatcher.
50. Implement reversible commands.
51. Implement bounded undo stack.
52. Implement bounded redo stack.
53. Add tests for depth limit.
54. Add tests for redo invalidation.
55. Add tests for grouped commands.

## Phase 6 — presets and profile infrastructure

56. Define schemas for style presets.
57. Define schemas for export profiles.
58. Implement built-in preset/profile loading.
59. Implement user preset overlay loading.
60. Implement resolved snapshot semantics.
61. Add tests for inheritance/override/reset.
62. Add tests for export profile resolution.
63. Document how developers add new profiles and presets.

## Phase 7 — fonts and template catalog

64. Implement local font discovery.
65. Implement Google Fonts configured-family handling.
66. Implement cache/download/index behavior under portable root.
67. Implement graceful failure for broken fonts.
68. Implement template settings model.
69. Add tests for font cache paths and bad-entry handling.
70. Record exact network/cache policy.

## Phase 8 — template layout

71. Implement grapheme-aware slot creation.
72. Implement kana scale.
73. Implement latin scale.
74. Implement punctuation scale.
75. Implement tracking.
76. Implement line height.
77. Implement slope angle.
78. Implement underlay modes.
79. Implement placement preview.
80. Implement slot commit/advance/cancel.
81. Add tests for slot generation and scaling rules.

## Phase 9 — media provider layer

82. Define provider traits/interfaces.
83. Implement runtime discovery.
84. Implement ffprobe-based metadata probing.
85. Implement encoder/muxer capability probing.
86. Implement diagnostics and provider errors.
87. Add tests for path resolution and capability parsing mocks.
88. Launch export/licensing sanity-review sub-agent and wait for it.
89. Integrate findings and document them.

## Phase 10 — playback foundation

90. Implement media import flow.
91. Implement playback state model.
92. Implement current time source.
93. Implement seek/play/pause.
94. Implement frame/canvas coordinate mapping.
95. Add tests for coordinate conversion math.
96. Add smoke checks for media loading.

## Phase 11 — free ink capture

97. Implement raw point capture.
98. Preserve raw samples.
99. Implement derived-path regeneration.
100. Implement stabilization control.
101. Implement selection.
102. Implement shift-grouping for free ink.
103. Add tests for raw/derived retention.
104. Add tests for grouping semantics.

## Phase 12 — guide system

105. Implement guide modifier defaults by platform.
106. Implement guide reference capture.
107. Implement guide geometry calculation.
108. Implement horizontal 3-line + helper lines.
109. Implement next-character vertical guide set.
110. Implement slope persistence.
111. Ensure guides are non-exported.
112. Add tests for guide geometry and exclusion from export.

## Phase 13 — outline, groups, page events

113. Implement object outline tree.
114. Implement derived runs.
115. Implement split/merge display logic.
116. Implement group/ungroup.
117. Implement z-order operations.
118. Implement alive highlighting.
119. Implement auto-follow-current.
120. Implement page event track.
121. Add tests for run derivation and group propagation.

## Phase 14 — styles, entrance, clear effects

122. Implement base style editor model.
123. Implement entrance kinds.
124. Implement reveal-head effect.
125. Implement post-action chain.
126. Implement clear event behaviors.
127. Implement style preset application.
128. Add tests for effect timing calculations.
129. Add tests for clear sequencing logic.

## Phase 15 — export settings UI and engine

130. Implement family + profile selection.
131. Compute concrete settings from profile data.
132. Expose disabled computed fields for non-Custom.
133. Enable direct editing for Custom.
134. Implement CPU-safe overlay composition.
135. Pipe/stream frames to FFmpeg where practical.
136. Implement hardware-try/software-fallback path.
137. Add tests for setting computation and fallback logic.
138. Add smoke export jobs for at least one composite and one transparent path.

## Phase 16 — preferences, cache manager, recovery

139. Implement Preferences dialog.
140. Implement cache manager.
141. Implement autosave cadence and recovery prompt.
142. Implement runtime diagnostics views.
143. Add tests for settings persistence and recovery flow where possible.

## Phase 17 — manuals, tutorials, polish

144. Update user manual to match reality.
145. Update developer guide to match actual code.
146. Implement the developer tutorial sample.
147. Validate the tutorial sample by build/run/test.
148. Launch final QA/docs sub-agent and wait for it.
149. Fix inconsistencies found by QA/docs review.
150. Update implementation report thoroughly.

## Phase 18 — final validation

151. Run full test suite.
152. Perform end-to-end save/load validation.
153. Perform end-to-end composite export validation.
154. Perform end-to-end transparent export validation.
155. Attempt Windows build from Linux if feasible.
156. Document blockers exactly if cross-build is incomplete.
157. Finalize known limitations.
158. Finalize packaging/licensing notes.
159. Finalize `progress.md`.
160. Stop only when done criteria are met.
