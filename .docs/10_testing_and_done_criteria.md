# Testing strategy and done criteria

## 1. Testing philosophy

The user explicitly prefers:

- careful phased development
- strong unit coverage
- minimized rework
- honest validation logs

So the implementation must be test-heavy and incremental.

## 2. Required unit-test areas

At minimum, cover:

- project parse/normalize/save
- unknown-field preservation
- command model and undo/redo
- clear event semantics
- grouping and ungrouping
- guide geometry
- template slot generation
- profile resolution
- preset inheritance/override/reset
- portable path resolution
- media capability parsing (where mockable)
- hardware-fallback selection logic
- smoothing helper math
- snapshot/job isolation logic

## 3. Required integration/smoke areas

At minimum, validate:

- create project -> save -> reopen -> compare
- import media -> annotate -> clear -> save
- composite export
- transparent export
- Google Fonts graceful failure
- portable root locality
- tutorial sample behavior

## 4. Golden/reference tests

Use golden tests where useful for:

- canonical project save output
- profile computation tables
- CPU compositor outputs (if practical)
- guide geometry reference cases

## 5. Failure logging requirement

Every significant failed attempt must go into `docs/implementation_report_v1.0.0.md`.

## 6. Done criteria

The project is done only when:

- the host build succeeds
- core tests pass
- at least one end-to-end composite export is validated
- at least one transparent export is validated
- docs match reality
- tutorial sample is validated
- Windows build attempt is documented
- known limitations are written down honestly
