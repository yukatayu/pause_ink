# PauseInk — Codex operating instructions

Read this file first. Then read **all files under `.docs/`**. Then skim `README.md`, `manual/`, `presets/`, `samples/`, `progress.md`, and `docs/implementation_report_v1.0.0.md`.

If any document conflicts, use the precedence order in the final section of this file.

---

## Mission

Build a careful, portable, desktop-first annotation application for paused-video handwriting workflows.

The product must feel useful for:

- Vlog annotation,
- relaxed commentary overlays,
- VOICEROID-style handwritten notes,
- quick scene-local explanatory scribbles.

This is not a whiteboard app and not a typography substitution app.  
The visual output should remain primarily the **user's own stroke data**.

---

## Locked product rules

### 1. Clear model

- v1.0 uses **manual clear events only**.
- Do **not** auto-insert clear events from scene-cut detection in v1.0.
- A clear event is **screen-wide**.
- A clear event is inserted when the user triggers Clear while paused or while playing.
- The interval between clears is a **page**.
- v1.0 does **not** include partial clear.

### 2. Handwriting ownership

- The final visible text is primarily the user's own strokes.
- Normal fonts are used for:
  - template slot generation,
  - underlay display,
  - spacing,
  - kana/latin/punctuation scaling,
  - tracking and slope support.
- Do not replace the final output with ordinary font glyphs as the default rendering path.

### 3. Project files

- Project extension is **`.pauseink`**.
- Format is **human-readable text**.
- Load must be **lenient**.
- Save must be **normalized and canonical**.
- Preserve unknown fields where practical so that hand edits and forward compatibility remain possible.
- Comments and trailing commas are acceptable on load.
- Canonical save format should remain deterministic and readable.

### 4. Undo / redo

- Default history depth: **256** commands.
- The depth must be configurable from settings.
- The implementation must be bounded and test-covered.

### 5. Portable-state rule

By default, PauseInk must not write to:

- XDG data dirs,
- `%APPDATA%`,
- `~/Library/Application Support`,
- registry-backed preferences,
- or other OS app-data locations.

All mutable state managed by the app must stay under an executable-local portable root, expected to be:

```text
<executable dir>/pauseink_data/
```

Allow a test/developer override by environment variable, but keep the default executable-local behavior.

### 6. Font support

- Local font files are required.
- Google Fonts support is required.
- Cache Google Fonts under the portable root.
- Broken or failed font entries may simply not appear in the dropdown.
- Do not let a bad Google Font fetch break the rest of the UI.

### 7. Pen pressure

- Real pen pressure is **out of scope for v1.0**.
- Do not build the v1.0 UX around pressure.
- Keep the architecture open for future pressure support.
- Future pseudo-pressure / auto taper work should be planned in docs.

### 8. Smoothing / stabilization

- v1.0 must provide user-adjustable stroke stabilization.
- Store raw input points non-destructively.
- Derive a render path from raw points.
- Preserve corners; do not over-round everything.
- The design target is an adaptive smoothing path such as **One Euro style filtering plus corner protection**.

### 9. Effects and presets

- v1.0 uses **built-in effects** plus **declarative preset files**.
- Do **not** add arbitrary user scripting to the render hot path.
- Effects must remain extensible by adding Rust code plus declarative preset definitions.
- A reveal-head highlight/glow effect belongs to entrance/clear behavior, not to the static base style alone.

### 10. GPU behavior

Treat GPU usage as two separate concerns:

- **UI/preview GPU usage**
- **media decode/encode hardware acceleration**

Required UI/UX:

- expose separate settings toggles for them
- default both to enabled
- detect/runtime-probe availability
- fall back cleanly if unavailable
- never fail the whole app merely because GPU acceleration is unavailable

v1.0 correctness priority:

- preview: GPU-preferred
- final overlay composition: CPU-safe baseline
- decode/encode: hardware acceleration when available, software fallback otherwise

### 11. Media runtime / FFmpeg

- FFmpeg must live behind a provider abstraction.
- v1.0 mainline expects a **portable sidecar runtime**.
- Do **not** rely on a “download FFmpeg automatically on first run” design for the mainline implementation.
- If helper scripts for acquiring runtime binaries are added, keep them outside the critical app path and document provenance/compliance clearly.
- Keep the app core MIT-friendly by not forcing GPL-only codec paths into the mainline architecture.

### 12. Export families

Main built-in export families for v1.0:

- WebM / VP9 / Opus
- WebM / AV1 / Opus
- MP4 / AV1 / AAC-LC (**Advanced**)
- MOV / ProRes 422 HQ / PCM
- MOV / ProRes 4444 / PCM
- PNG Sequence / RGBA
- AVI / MJPEG / PCM (**Legacy rescue**)

Optional future codec pack:

- MP4 / H.264 / AAC-LC
- MP4 or MOV / HEVC / AAC-LC

### 13. Platform presets

The export UI must distinguish:

- **container/codec family**
- **distribution/profile preset**

Distribution/profile presets include at least:

- Low
- Medium
- High
- YouTube
- X
- Instagram
- Adobe Edit
- Adobe Alpha
- Custom

Behavior requirements:

- For non-Custom presets, display the computed values in disabled numeric fields.
- For Custom, allow direct numeric editing.
- Developers must be able to add new export profiles without editing unrelated core logic.

### 14. UI model

- Use a **single main window** for v1.0.
- No floating detached tool windows are required.
- Prefer docked/tabbed panels inside the single window.

### 15. Safety constraints from the user

- avoid dangerous shell commands
- avoid unnecessary heavy package installs
- be careful with disk and memory
- clean large temporary files when no longer needed
- if sub-agents are obviously orphaned and unrelated, they may be terminated
- if sub-agents are launched, wait for them before integrating their results

---

## Technical direction

Default expected implementation direction:

- Rust workspace
- native windowing/event loop
- native renderer
- UI path compatible with Linux/macOS/Windows without browser-runtime dependency
- FFmpeg sidecar runtime provider
- strong module boundaries:
  - domain
  - project format
  - portable filesystem
  - fonts/template layout
  - media/runtime provider
  - renderer
  - UI
  - export orchestration

If you change the stack, document the reason precisely in `docs/implementation_report_v1.0.0.md`.

---

## Required working style

### Plan before coding

Before major code changes:

- refine the phase plan in `progress.md`
- log the chosen immediate milestone in `docs/implementation_report_v1.0.0.md`

### Update `progress.md` continuously

Update it:

- when a phase starts
- when a phase completes
- when a blocker appears
- when a milestone slips or changes

### Update `docs/implementation_report_v1.0.0.md` continuously

This is mandatory.

Do not wait until the end. Record:

- environment details
- decisions
- sub-agent findings
- commands run
- tests run
- failed attempts
- fixes
- known issues
- packaging notes

### Test while building

Mandatory expectations:

- unit tests for nontrivial modules
- integration tests for major flows
- smoke checks for end-to-end save/load/export
- golden/reference tests where useful
- parser/normalizer tests for `.pauseink`
- portable-path tests
- export-profile resolution tests
- hardware-fallback tests when mockable
- concurrency model tests for snapshot-based background work

### Keep the architecture loosely coupled

Required separation rules:

- UI must not own business rules
- project parsing must not depend on rendering
- render effects must not be coupled to file I/O
- background jobs must work on immutable snapshots
- FFmpeg access must be abstracted behind provider interfaces
- preset data must remain declarative

---

## Sub-agent policy

The user explicitly wants sub-agents used.

Minimum required checkpoints:

1. **Architecture sanity review** before the main crate/module structure is locked.
2. **Media/export/licensing sanity review** before export behavior is locked.
3. **Final QA/docs sanity review** before declaring done.

Rules:

- keep concurrent sub-agents low (normally 1–2)
- wait for them
- if a clearly unrelated orphan remains alive, terminate it
- record findings and adopted/rejected changes in the implementation report

---

## Required deliverables

The final repository after implementation must include at least:

- working desktop app
- save/load for `.pauseink`
- autosave and recovery
- undo/redo
- free-ink mode
- guide mode
- template placement mode
- object outline panel
- page clear event track
- built-in presets
- transparent export
- composite export
- portable-state layout
- local fonts + Google Fonts handling
- user manual
- developer manual
- at least one validated developer tutorial sample
- updated implementation report
- updated progress file

---

## Done criteria

Do not mark the project done until all of the following are true:

- docs, code, tests, and sample/tutorial content are mutually consistent
- save/load roundtrip works on the host OS
- manual clear semantics are validated
- at least one composite export is validated
- at least one transparent export is validated
- portable-state rule is validated
- Google Fonts graceful failure behavior is validated
- export-profile computation is validated
- developer tutorial sample is validated
- Windows build is attempted from Linux if feasible, otherwise blocker is documented precisely
- known limitations are listed honestly
- `docs/implementation_report_v1.0.0.md` is complete and current

---

## Documentation precedence

If documents disagree, use this order:

1. `AGENTS.md`
2. `.docs/02_final_spec_v1.0.0.md`
3. `.docs/04_architecture.md`
4. `.docs/07_media_runtime_and_ffmpeg.md`
5. `.docs/08_output_profiles_and_platform_presets.md`
6. the rest of `.docs/`
7. `README.md`


## Packaging preference

- one main app binary when practical
- portable sidecar runtime only where needed
- do not chase an all-in-one codec bundle if it worsens licensing/compliance or maintainability
