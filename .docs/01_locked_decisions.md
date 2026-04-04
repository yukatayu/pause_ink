# Locked decisions

| Topic | Decision |
|---|---|
| Clear insertion | Manual only in v1.0 |
| Clear scope | Screen-wide |
| Partial clear | Not in v1.0 |
| Main output visual source | User stroke data |
| Fonts | Template/layout/underlay only |
| Project extension | `.pauseink` |
| Project format | Human-readable JSON5-style text |
| Load behavior | Lenient |
| Save behavior | Normalized canonical save |
| Unknown fields | Preserve where practical |
| Undo depth | Configurable, default 256 |
| State locality | Executable-local portable root |
| Google Fonts | Supported with cache |
| Pen pressure | Not in v1.0 |
| Stroke smoothing | Required in v1.0 |
| Effect scripting | Not in v1.0 |
| GPU usage | Separate toggles for UI/preview and media acceleration |
| Final composition path | CPU-safe baseline |
| Media runtime | FFmpeg sidecar/provider |
| Mainline FFmpeg acquisition | Do not auto-download in app |
| UI model | Single window |
| Export families | WebM VP9/AV1, MP4 AV1 advanced, ProRes, PNG seq, AVI MJPEG |
| Adobe-focused outputs | ProRes 422 HQ, ProRes 4444, PNG sequence |
| H.264/HEVC | Optional codec-pack territory, not mainline assumption |
