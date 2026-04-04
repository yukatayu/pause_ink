# Media runtime, FFmpeg provider, and codec policy

## 1. Mainline runtime strategy

v1.0 mainline uses a **portable sidecar runtime**.

Expected layout concept:

```text
pauseink_data/
  runtime/
    ffmpeg/
      <platform-id>/
        ffmpeg[.exe]
        ffprobe[.exe]
        manifest.json
```

Alternative development-time repository-local layout is acceptable if documented.

## 2. Why not in-app first-run downloader as the mainline

Do not make the mainline app depend on automatic FFmpeg binary download on first run.

Reasons:

- provenance/compliance concerns
- unreliable third-party binary sourcing
- harder offline behavior
- harder test reproducibility
- more moving parts in the most failure-prone path

Helper tooling may be added later, but it should not be the critical path in v1.0.

## 3. Capability-based behavior

The app must discover runtime capabilities at execution time, including:

- available decoders
- available encoders
- available muxers
- available pixel formats
- possible hardware acceleration support

Do not hard-code broad assumptions from container extension alone.

## 4. Import stance

Import support is intentionally broader than export-family support.

The app may attempt to import any file the active runtime can probe/decode.

At import time, classify the media as:

- supported
- supported with caveats
- unsupported

Possible caveat examples:

- variable frame rate
- unsupported alpha
- unusual timebase
- codec readable but not efficiently seekable

## 5. GPU / media acceleration stance

Media acceleration is optional and separately configurable from UI preview GPU use.

Preferred algorithm:

1. if media HW acceleration is enabled and plausible, try it
2. if not possible or it fails, fall back to software
3. keep the app working

Never fail the whole product just because hardware acceleration is missing.

## 6. Main built-in export families

Mainline built-ins:

- WebM / VP9 / Opus
- WebM / AV1 / Opus
- MP4 / AV1 / AAC-LC (Advanced)
- MOV / ProRes 422 HQ / PCM
- MOV / ProRes 4444 / PCM
- PNG Sequence / RGBA
- AVI / MJPEG / PCM (Legacy rescue)

## 7. Optional codec-pack territory

These are intentionally treated as optional/future codec-pack territory:

- H.264 encode
- HEVC encode

Reasons:

- licensing/patent/compliance complexity
- desire to keep the core app MIT-friendly
- desire to avoid forcing GPL-only FFmpeg builds into the mainline assumption

## 8. H.264 / HEVC import note

The user explicitly asked whether reading H.264 material can also raise licensing concerns.

Design consequence:

- keep codec runtime policy separate from app license policy
- keep media provider and packaging decisions well documented
- do not present H.264 ingest as legally “free of concern” in docs
- state clearly that release packaging may require separate legal review

## 9. Adobe-focused deliverables

For Adobe interoperability, v1.0 must include:

- MOV / ProRes 422 HQ / PCM
- MOV / ProRes 4444 / PCM
- PNG Sequence / RGBA

These are the safest declared editing/intermediate outputs for Adobe-centric workflows.

## 10. Logging

Every export should log:

- chosen export family
- chosen distribution profile
- computed concrete bitrates/settings
- runtime path
- whether hardware path was attempted
- whether fallback occurred
- provider stderr/stdout summaries if useful
