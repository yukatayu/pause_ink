# Architecture

## 1. Architectural priorities

1. correctness
2. portability
3. predictable fallback behavior
4. loose coupling
5. future extensibility without rewriting everything

## 2. Proposed workspace/module split

Suggested crates/modules:

- `app` — binary entrypoint and app wiring
- `domain` — core project model and commands
- `project_io` — `.pauseink` parse/normalize/save
- `portable_fs` — executable-local state, cache, logs, autosave
- `presets_core` — preset schemas and profile resolution
- future modules to add during implementation:
  - `ui`
  - `renderer`
  - `media`
  - `export`
  - `fonts`
  - `template_layout`

The exact crate split may change, but the boundaries must remain.

## 3. Data ownership

### 3.1 Single writer model

Mutating project state should happen on the UI/app thread only.

### 3.2 Snapshot-based background jobs

Background workers receive immutable snapshots for:

- export
- probe
- thumbnail generation
- cache cleanup candidate scanning

Workers must not mutate live project state directly.

### 3.3 Event return path

Workers communicate via messages/events/results back to the UI thread.

## 4. Rendering model

### 4.1 Preview

- GPU-preferred path
- backend/runtime probe at startup or first canvas creation
- clean fallback if unavailable

### 4.2 Final composition

v1.0 baseline: CPU-safe composition path for correctness and reproducibility.

Future work may add a GPU export compositor, but it should not block v1.0.

### 4.3 Stroke rendering

Stroke rendering pipeline should be layered:

- raw samples
- stabilized samples
- derived path/mesh
- effect application
- composite

## 5. Media architecture

### 5.1 Provider abstraction

Media access sits behind a provider interface with responsibilities for:

- probing
- capability discovery
- frame access/playback assistance
- export invocation
- diagnostics

### 5.2 Runtime location

Portable sidecar runtime under the portable root or repository-local runtime layout during development.

### 5.3 Capability model

The app must query what the runtime can do rather than assuming:

- decoder support
- encoder support
- muxer support
- hardware acceleration availability

## 6. Export architecture

### 6.1 Separation of concerns

Export is composed from:

- project snapshot
- chosen container/codec family
- chosen distribution profile
- computed concrete settings
- provider capability result
- software/hardware path selection

### 6.2 Fallback sequence

Recommended behavior:

1. compute target settings
2. if media HW accel allowed, try hardware path when capability says plausible
3. if hardware path fails, retry once with software path
4. log the path used and the reason

## 7. Project model notes

The domain model must keep distinct:

- z-order
- capture order
- reveal order
- page clear events
- object/group relationships
- preset references vs resolved snapshots

## 8. Future-proof hooks

Keep hooks for:

- pen pressure
- pseudo-pressure/taper
- partial clear
- proxy media
- user effect scripting
- GPU export compositor
- codec-pack installation helpers

These hooks should exist as extension seams, not as half-built UX.
