# PauseInk Codex handoff repository

Provisional product name: **PauseInk**  
Locked project extension: **`.pauseink`**

This repository is a **Codex handoff package**. Its purpose is to let Codex read one repository, understand the final locked product direction, and then implement the application in a careful, test-heavy, low-regression way.

## Product summary

PauseInk is a portable desktop application for:

- opening a local video,
- pausing or playing it,
- writing hand-drawn annotations onto frames,
- replaying them with controlled reveal effects,
- clearing them with **manual screen-wide clear events**, and
- exporting either:
  - a composite video, or
  - annotation-only transparent output.

The target style is Vlog / relaxed commentary / VOICEROID-like annotation overlays.

## What is already decided

The following are no longer open questions:

- **Desktop-first v1.0**: Linux, macOS, Windows.
- **Single-window UI**.
- **Manual clear only** in v1.0. There is no automatic scene-cut insertion in v1.0.
- A clear event is **screen-wide**, inserted when the user triggers Clear while paused or playing.
- **Partial clear is out of scope for v1.0**.
- Final visible output remains primarily the **user's own stroke data**.
- Fonts are used for **template slots, underlays, spacing, and kana/latin/punctuation scaling**.
- Project files are **human-readable text** in a **lenient-load / normalized-save** style.
- All mutable state must stay **next to the executable** inside a portable data directory.
- **Google Fonts support is required**.
- **Pen pressure is not implemented in v1.0**.
- v1.0 uses **built-in effects + declarative presets**, not arbitrary scripting in the hot render path.
- Media import/export is abstracted behind an **FFmpeg runtime provider**.
- GPU use is **configurable** and must **fall back cleanly** when unavailable.

## Repository map

- `AGENTS.md` — Codex operating contract
- `.docs/` — locked design, architecture, export rules, test strategy, implementation plan
- `docs/implementation_report_v1.0.0.md` — live implementation report that must be updated throughout the run
- `progress.md` — current phase and checkpoint log
- `manual/` — user and developer guides plus tutorial stubs
- `presets/` — declarative preset examples and expected schema direction
- `samples/` — example project/settings files
- `runtime/README.md` — expected FFmpeg sidecar layout
- `crates/` — minimal Rust workspace scaffold

## Recommended Codex workflow

1. Read `AGENTS.md`.
2. Read **all** files under `.docs/`.
3. Skim `README.md`, `progress.md`, `manual/`, `presets/`, `samples/`, and `docs/implementation_report_v1.0.0.md`.
4. Rewrite `progress.md` into an execution-ready plan.
5. Launch sub-agents at the required checkpoints.
6. Implement in phases, with tests and report updates throughout.

## Important packaging stance

The app binary is intended to be **MIT-licensed**.  
Codec/runtime concerns must stay isolated behind the FFmpeg provider layer.

The baseline project should work without requiring a GPL-only FFmpeg build. Optional codec packs may exist later.

## Notable output families

Built-in target families for v1.0:

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

## End-user state locality rule

PauseInk must not scatter state across OS app-data directories by default.  
Expected layout is under:

```text
<executable dir>/
  pauseink_data/
    config/
    cache/
    logs/
    autosave/
    runtime/
```

For tests or development, an override environment variable may redirect this root, but the default remains executable-local.

## Current status of this repository

This handoff repository contains:

- the locked product definition,
- a detailed phased implementation plan,
- acceptance criteria,
- future work planning,
- developer manual targets,
- sample data,
- and a minimal workspace scaffold.

It is **not** the finished application.


## Packaging preference

Preferred release shape:

- one main application binary when practical
- portable `pauseink_data/` beside it
- optional media/runtime sidecars kept clearly separated

Do not optimize for a monolithic all-codecs bundle if that would make licensing/compliance or portability worse.
