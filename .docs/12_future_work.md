# Future work

## 1. Partial clear

Not in v1.0.  
Future order of consideration:

1. manual page break + carry-forward
2. keep selected group into next page
3. group-scoped clear
4. arbitrary partial clear

## 2. Pen pressure

Real pressure support is future work.  
Architectural requirement in v1.0: do not block it.

## 3. Auto taper / pseudo-pressure

Future checkbox-level feature:

- Auto taper on/off
- later: advanced controls

Recommended future signals:

- path progress
- speed
- local curvature
- start/end proximity

## 4. Proxy media

Not in v1.0.  
Design the media layer so proxy generation can be added later without replacing the project model.

## 5. GPU export compositor

Future optimization only.  
Do not compromise v1.0 correctness for it.

## 6. Codec-pack helper tooling

Future helper for obtaining optional codec-capable FFmpeg runtimes with clear provenance/compliance documentation.

## 7. Effect scripting

Out of scope for v1.0.  
A future safe expression layer or scripting API may be added, but not in the hot path first.
