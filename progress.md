# PauseInk progress

This file must stay short, current, and truthful.

## Current overall status

- Repository type: Codex handoff package
- Product version target: v1.0.0
- Locked design state: **ready for implementation**
- Latest repository refresh reason: final spec freeze after manual clear / GPU / export / Adobe / portable-state decisions

## Immediate next actions for Codex

1. Read `AGENTS.md` and `.docs/`.
2. Rewrite this file into a live execution tracker.
3. Record environment details in `docs/implementation_report_v1.0.0.md`.
4. Launch the first architecture sanity-check sub-agent before major coding.
5. Begin Phase 0 and Phase 1 from `.docs/11_implementation_plan.md`.

## Locked design highlights

- manual clear events only
- no partial clear in v1.0
- `.pauseink` JSON5 project files
- portable mutable state under executable-local `pauseink_data/`
- local fonts + Google Fonts cache
- GPU preview preferred, CPU-safe export composition baseline
- FFmpeg sidecar/provider model
- Web + Adobe + transparent export presets
- declarative presets, no arbitrary hot-path scripting

## Required live updates during implementation

Codex must keep this file updated with:

- current phase
- active blocker(s)
- latest successful validation
- next concrete step
