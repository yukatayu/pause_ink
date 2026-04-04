# Project file format

## 1. Extension and encoding

- extension: `.pauseink`
- encoding: UTF-8 text

## 2. Syntax target

The project format is JSON5-style text:

- comments accepted on load
- trailing commas accepted on load
- unquoted keys may be accepted if the chosen parser supports them
- normalized save produces stable canonical formatting

## 3. Guiding principles

- human-readable
- human-editable
- tolerant on load
- predictable on save
- preserve unknown fields where practical

## 4. Save normalization goals

On save:

- sort or stabilize fields in a documented order
- normalize numeric formatting where appropriate
- remove transient runtime-only state
- keep comments only if the chosen preservation model supports them; otherwise document the limitation honestly
- produce deterministic output

## 5. Unknown fields

Unknown field preservation is desirable.

Minimum requirement:

- do not crash on unknown fields
- keep unknown fields in memory when practical
- write them back if feasible
- if some unknown fields cannot be preserved, document the exact limitation

## 6. Top-level shape (target)

```json
{
  "format_version": "1.0.0",
  "project": {
    "metadata": {},
    "media": {},
    "settings": {},
    "pages": [],
    "objects": [],
    "groups": [],
    "clear_events": [],
    "presets": {},
    "unknown": {}
  }
}
```

This is illustrative, not final locked syntax.

## 7. Autosave/recovery

Autosaves are separate files under the portable root and must not overwrite the main project path without explicit save.

## 8. Settings

Settings are separate from project data and stored under portable config.  
Project-specific settings that affect reproducibility belong in the project file.

## 9. History depth

The runtime settings file must contain a configurable bounded history depth, default 256.

## 10. Example file

See `samples/minimal_project.pauseink`.
