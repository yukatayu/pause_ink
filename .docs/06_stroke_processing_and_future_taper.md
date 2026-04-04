# Stroke processing, stabilization, and future taper

## 1. v1.0 requirements

- no pen pressure pipeline
- yes to user-adjustable stabilization
- raw samples must be preserved
- render path must be derived
- corners must survive reasonably well
- implementation must remain compatible with future pressure/taper work

## 2. Recommended v1.0 stabilization design

### 2.1 Input storage

Store raw points with timestamps.

### 2.2 Derived path

Generate a stabilized path from raw points using:

- adaptive One Euro style filtering or equivalent
- corner/curvature guard to reduce smoothing near sharp turns
- optional streamline/resampling pass for mesh/path generation

### 2.3 UI

Single numeric control in v1.0:

- `Stroke stabilization strength` (e.g. 0–100)

Implementation may internally map this to:

- min cutoff
- beta/adaptation strength
- resampling tolerance
- corner guard threshold

## 3. Why not plain heavy smoothing

A simple heavy low-pass filter:

- kills corners
- makes kana/kanji structure mushy
- creates lag at faster movements
- makes handwriting feel “rubbery”

So v1.0 should not use a naive fixed smoothing pass alone.

## 4. Future automatic taper / pseudo-pressure

A future “Auto taper” checkbox is explicitly planned.

Recommended future signal mix:

- start taper based on distance from stroke start
- end taper based on distance to stroke end
- synthetic pressure influenced by speed
- curvature/corner protection so corners do not collapse
- optional post-corner recovery

### 4.1 Important conclusion

“Distance since the last big curve” alone is **not enough**.  
Use it only as a supporting signal, not the main signal.

### 4.2 Better heuristic mix

Preferred future heuristic:

- speed
- normalized path progress
- local curvature
- start/end proximity

## 5. Useful prior art to study during implementation

- One Euro Filter
- Google Ink Stroke Modeler
- perfect-freehand

Codex should note in the implementation report which parts influenced the final v1.0 implementation.
