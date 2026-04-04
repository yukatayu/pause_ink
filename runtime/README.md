# Runtime layout

PauseInk v1.0 expects a portable sidecar media runtime.

## Suggested layout

```text
pauseink_data/
  runtime/
    ffmpeg/
      linux-x86_64/
        ffmpeg
        ffprobe
        manifest.json
      windows-x86_64/
        ffmpeg.exe
        ffprobe.exe
        manifest.json
      macos-aarch64/
        ffmpeg
        ffprobe
        manifest.json
```

Exact platform IDs can vary, but keep them deterministic.

## Manifest suggestion

The manifest should ideally record:

- runtime name
- version
- source/provenance
- build notes
- supported codecs/encoders summary if available
- license note reference

## Mainline rule

The app should not depend on downloading this automatically on first run.

## Development-time fallback

During development, if a system `ffmpeg` / `ffprobe` is used, the implementation report must record it clearly.
