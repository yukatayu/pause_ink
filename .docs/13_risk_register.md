# Risk register

| Risk | Impact | Mitigation |
|---|---|---|
| GPU backend instability on some machines | preview/export issues | separate preview GPU from media accel, keep CPU-safe baseline |
| Over-smoothed handwriting | poor visual quality | raw+derived split, stabilization tests, corner guard |
| Project format brittleness | hand-edited files break | lenient load, normalized save, parser tests |
| FFmpeg runtime mismatch | import/export failures | provider diagnostics, sidecar manifest, capability probing |
| Google Fonts network/cache failures | poor UX | graceful skip, cached index, no hard failure |
| Cross-platform UI divergence | layout bugs | single-window design, testable state model |
| Export preset sprawl | hard maintenance | data-driven profile files + developer docs |
| Licensing confusion around codecs | shipping risk | separate app/core from runtime/codec policy, document packaging clearly |
| Disk usage growth | low-storage issues | cache manager, bounded caches, cleanup tools |
| Concurrency bugs | corrupted state or crashes | UI-thread mutation + immutable worker snapshots |
