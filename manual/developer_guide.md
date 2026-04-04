# Developer guide

This guide must be updated to match the actual implementation.

## 1. Architectural principles

- domain logic independent from UI
- project I/O independent from rendering
- portable filesystem logic centralized
- FFmpeg behind a provider abstraction
- raw strokes preserved non-destructively
- export profile rules data-driven where practical

## 2. Topics this guide must cover before completion

1. workspace/crate overview
2. command model and undo/redo
3. `.pauseink` parsing and normalized save
4. portable root enforcement
5. preset registration and resolution
6. export profile registration
7. FFmpeg runtime/provider discovery
8. how GPU and media acceleration fallbacks work
9. how to add a built-in effect
10. how to add a new export profile
11. how to run tests
12. how to reproduce smoke validations

## 3. Developer tutorial requirements

Codex must provide at least one validated tutorial with a working sample.

Recommended topics:

- add a built-in style preset
- add a new export profile
- validate that the UI/profile catalog sees it

See `manual/tutorials/01_add_export_profile.md` and `manual/tutorials/02_add_builtin_preset.md`.
