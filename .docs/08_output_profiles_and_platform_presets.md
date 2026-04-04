# Output profiles and platform presets

## 1. Split the export decision into two layers

### Layer A — container/codec family

Examples:

- WebM / VP9 / Opus
- WebM / AV1 / Opus
- MP4 / AV1 / AAC-LC
- MOV / ProRes 422 HQ / PCM
- MOV / ProRes 4444 / PCM
- PNG Sequence / RGBA
- AVI / MJPEG / PCM

### Layer B — distribution/profile preset

Examples:

- Low
- Medium
- High
- YouTube
- X
- Instagram
- Adobe Edit
- Adobe Alpha
- Custom

This split keeps the system extensible and lets developers add platform presets without rewriting codec logic.

## 2. UI behavior

- User chooses family + profile.
- The app computes concrete numeric settings.
- For non-Custom profiles, numeric input fields display the computed values but remain disabled.
- For Custom, numeric fields become editable.

Numeric fields should include at least:

- target video bitrate
- max video bitrate if applicable
- audio bitrate
- GOP/keyframe interval
- sample rate
- possibly CRF/CQ or quality target for codec families that use them

## 3. Official vs app-authored profile sources

### 3.1 YouTube

Use official published encoding guidance where directly available.

### 3.2 X

Use official published upload guidance where directly available.

### 3.3 Instagram

Use official public constraints where available.  
Where exact bitrate ladders are not formally published in the same way, use app-authored “safe defaults” and label them honestly.

### 3.4 Adobe

Use app-authored intermediate/editing presets based on Adobe-compatible families.

## 4. Built-in preset expectations

### 4.1 Web/social default families

- WebM VP9 + Opus: main open default
- WebM AV1 + Opus: high compression default
- MP4 AV1 + AAC-LC: advanced single-file compatibility option

### 4.2 Adobe/editing families

- MOV ProRes 422 HQ + PCM: editing master/intermediate
- MOV ProRes 4444 + PCM: alpha/intermediate
- PNG sequence RGBA: maximum transparency interoperability

### 4.3 Legacy rescue

- AVI MJPEG + PCM

## 5. Data-driven profile files

Store profile definitions in declarative files under `presets/export_profiles/`.

Suggested file responsibilities:

- platform/profile name
- intended family compatibility
- bitrate ladder
- frame-rate adjustment rules
- audio defaults
- explanatory notes
- source reference URLs

The application should load them through a stable schema, not by hard-coding every platform rule in UI code.

## 6. Resolution-aware calculation direction

The computation engine should consider at least:

- output width/height
- frame rate bucket
- family capabilities
- platform/profile preference
- alpha requirement
- audio presence

## 7. Example platform guidance references

Codex should verify and record exact values used during implementation from these kinds of official pages:

- YouTube recommended upload encoding settings
- X media upload/media studio guidance
- Instagram Reels/public constraints
- Adobe Media Encoder supported import/export format pages

Store the final selected URLs and values in `docs/implementation_report_v1.0.0.md`.

## 8. Profile extensibility rule

Adding a new platform preset must be easy for a developer:

1. create a new profile file
2. register it in the profile catalog if needed
3. add/update tests
4. document it in the developer guide


## 9. Audio policy note

- **PCM** is the uncompressed/lossless-style intermediate choice for editing/master outputs.
- Social/web delivery presets should prefer **AAC-LC** or **Opus** depending on the chosen family/platform.
- Adobe-oriented intermediate presets may use PCM because file size is less important than edit-friendliness and fidelity.
