# Portable layout and cache policy

## 1. Default portable root

Default mutable root:

```text
<executable dir>/pauseink_data/
```

This root should contain all app-managed mutable state.

## 2. Suggested directory layout

```text
pauseink_data/
  config/
    settings.json5
  cache/
    google_fonts/
    font_index/
    media_probe/
    thumbnails/
  logs/
  autosave/
  runtime/
    ffmpeg/
  temp/
```

The exact names may vary, but the spirit must remain.

## 3. Locality rule

Do not write app-managed mutable state outside the portable root by default.

## 4. Development/test override

A developer/test-only override environment variable is acceptable so tests can isolate state in CI or repo-local temp roots.

## 5. Cache behavior

### 5.1 Google Fonts cache

- cache downloaded assets under the portable root
- broken downloads may be ignored or cleaned up
- never block the whole UI on one bad family

### 5.2 Probe cache

- cache media probe results with invalidation keyed by file path + metadata signature

### 5.3 Thumbnail cache

- bounded
- cleanable by the user

## 6. Cleanup tools

v1.0 should include at least a basic cache manager dialog or action that can:

- show major cache categories
- clear selected categories
- report approximate size if feasible

## 7. Logging

Logs also stay under the portable root.  
Do not spray logs into global OS log locations by default.
