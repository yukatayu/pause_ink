# Final product specification — v1.0.0

## 1. Core concept

PauseInk lets the user add hand-drawn overlays to video frames.  
The overlays are tied to video time. They can appear with configurable reveal behaviors, stay visible across a page interval, and disappear through a manual screen-wide clear event.

The design goal is:

- preserve the charm of the user's handwriting,
- provide just enough structure for readability,
- avoid surprise destructive normalization,
- keep the UI understandable.

## 2. Timeline model

### 2.1 Video timeline

The project uses the source media timeline as the master time base.

### 2.2 Page model

A **page** is the interval between two clear boundaries:

- start of project or immediately after a prior clear
- until the next clear event or end of project

### 2.3 Clear event model

A clear event is:

- inserted explicitly by the user while paused or playing
- owned by the page-event track, not by individual strokes
- screen-wide, affecting all alive annotation objects at that instant

v1.0 clear behaviors:

- instant clear
- ordered clear by write order
- reverse ordered clear
- wipe out
- dissolve out

The clear event owns:

- clear kind
- duration
- target granularity for the effect algorithm
  - object
  - group
  - stroke
  - all parallel
- ordering
  - serial
  - reverse
  - parallel

Even though the clear event is screen-wide, the *internal visual sequencing* may still work at object/group/stroke granularity.

### 2.4 No partial clear in v1.0

The UI must not expose:

- clear selected only
- clear current group only
- clear by tag
- clear by region

These stay in future work only.

## 3. Annotation object model

### 3.1 Stroke

A stroke stores:

- raw input points
- timestamped samples
- derived/render path
- style snapshot
- creation time anchor

### 3.2 Glyph Object

A glyph object is the main character-like annotation unit.

It may contain:

- one or more strokes
- style snapshot
- entrance behavior
- post-entrance behavior chain
- geometry transform
- z-order
- capture/reveal order metadata

### 3.3 Group

A group is a user-defined collection of glyph objects and/or strokes used for:

- shared reveal behavior
- shared post-action timing scope
- batch editing

### 3.4 Runs

Runs are display-derived groupings in the outline panel for consecutive objects that share the same settings.  
Runs are not the same as user-defined groups and must not replace explicit group data.

## 4. Input modes

### 4.1 Free Ink

The user draws directly on the canvas.

- default mode: no guides
- one pen-down to pen-up becomes one stroke
- `Shift` groups consecutive strokes into one glyph object

### 4.2 Guide Capture

Normally guides are hidden.  
When the guide modifier is held and the user writes a reference glyph object:

- the reference object defines guide geometry
- guides appear for subsequent writing
- guides are editor-only and not exported

Required guide appearance:

- long horizontal 3-line system
- 2 faint helper lines between them
- next-character short vertical 3-line guide
- 2 faint helper lines for the next-character cell
- configurable upward slope angle
- settings persisted across launches

Platform-default guide modifier:

- Windows/Linux: `Ctrl`
- macOS: `Option`

The modifier must remain remappable in settings.

### 4.3 Template Placement

The user enters text and configures a template underlay:

- font family
- font size
- tracking
- line height
- kana scale
- latin scale
- punctuation scale
- slope angle
- underlay mode

Pressing “Place Template” enters placement mode:

- underlay follows the pointer
- click to place
- settings update the preview in real time
- cancel exits placement mode
- placement mode underlay disappears when cancelled or replaced by a new placement action

The template defines **slots**, not final visible glyph substitution.

#### Underlay modes

v1.0 supports:

- outline underlay
- faint fill underlay
- slot box only
- outline + slot box

### 4.4 Template capture behavior

When a template is active, the default interpretation is:

- multiple strokes contribute to the current slot's glyph object
- commit advances to the next slot using explicit next-slot action or next-slot start
- `Shift` remains available for force-group behaviors outside template mode

## 5. Appearance settings

### 5.1 Base style

Each visible object has base style fields:

- thickness
- color
- opacity
- outline
- drop shadow
- glow
- blend mode (at minimum: normal, additive)

### 5.2 Entrance behavior

Built-in entrance kinds:

- path trace
- instant
- wipe
- dissolve

Entrance parameters include:

- target scope: stroke / glyph object / group / run
- order: serial / reverse / parallel
- duration mode:
  - proportional to stroke length
  - fixed total duration
- speed scalar

### 5.3 Reveal-head effect

Entrance may include an optional head effect:

- none
- solid head
- glow head
- comet/tail head

Head effect parameters:

- color source: preset accent / stroke color / custom
- size multiplier
- blur radius
- tail length
- persistence
- blend mode

### 5.4 Post-actions

Post-actions are chained state changes after or during reveal.  
Each chain entry specifies:

- timing scope:
  - during reveal
  - after stroke
  - after glyph object
  - after group
  - after run
- action:
  - no-op
  - style change
  - interpolated style change
  - pulse
  - blink

v1.0 built-ins are enough; arbitrary scripting is not required.

## 6. Presets

### 6.1 Preset categories

v1.0 includes:

- base style presets
- entrance presets
- clear presets
- combo presets

### 6.2 Built-in vs user presets

- built-in presets are read-only
- user presets are editable and stored under the portable root
- projects store **resolved snapshots** plus optional preset identifiers
- projects must not depend on mutable live preset files to reproduce old visuals

### 6.3 Reset behavior

Each editable field can be:

- inherited from preset
- overridden
- reset back to preset value

## 7. Text layout and spacing behavior

The template system must support:

- grapheme-aware slot creation
- kana scale
- latin scale
- punctuation scale
- tracking
- line height
- slope
- mixed-script layout

The handwritten output is placed into slots but is not forcibly reshaped unless the user opts into gentle fitting behaviors.

### 7.1 Slot fit options

v1.0 supports:

- Off
- Move only
- Weak uniform scale

Default: **Off**.

## 8. Smoothing and stroke stabilization

v1.0 includes adjustable stroke stabilization with these rules:

- raw points are preserved
- render path is derived
- corners should survive
- smoothing must be user-adjustable with a single strength control

Design target:

- adaptive One Euro style filtering
- corner guard / curvature-aware smoothing reduction
- explicit future hook for optional pseudo-pressure/taper

## 9. Selection, ordering, and editing

The user can:

- select objects
- multi-select
- group
- ungroup
- reorder z-order
- batch-edit styles/effects

The application must keep **capture/reveal order** conceptually separate from **z-order**.

## 10. Panels

### 10.1 Object Outline

A tree-like panel showing:

- runs
- groups
- glyph objects
- strokes

Must support:

- expand/collapse
- multi-select
- batch edit
- reorder
- visibility/lock/solo
- current alive highlighting
- optional auto-follow-current

### 10.2 Page Events

A separate timeline track for clear events only.

### 10.3 Export Queue

A simple queue/status view for export jobs.

### 10.4 Logs

An in-app view into recent log output is desirable for troubleshooting.

## 11. Save/load behavior

### 11.1 Project files

- extension: `.pauseink`
- encoding: UTF-8
- format: JSON5-style text
- allow comments and trailing commas on load
- canonical normalized save on write

### 11.2 Unknown fields

Unknown fields should be preserved where practical to support hand editing and forward compatibility.

### 11.3 Autosave

Autosave is required.

### 11.4 Crash recovery

Recovery from recent autosave is required.

## 12. Export behavior

### 12.1 Composite export

Exports source video plus annotations.

### 12.2 Transparent export

Exports annotation-only output.

Required transparent families:

- PNG Sequence RGBA
- MOV / ProRes 4444 / PCM (or silent if no audio included)

### 12.3 Export profiles

The UI must separate:

- **container/codec family**
- **distribution preset**

Distribution presets:

- Low
- Medium
- High
- YouTube
- X
- Instagram
- Adobe Edit
- Adobe Alpha
- Custom

For all non-Custom presets:

- show computed numeric values
- keep numeric entry widgets disabled

For Custom:

- allow direct numeric editing

## 13. Import behavior

Input media support is not restricted to the export families.  
Import accepts whatever the active FFmpeg runtime can probe and decode.

The app should classify imports as:

- supported
- supported with caveats
- unsupported

based on runtime probe results.

## 14. Preferences

Must include at least:

- undo history depth
- portable root override (developer/test only, hidden/advanced is acceptable)
- guide modifier override
- guide slope angle
- guide persistence options
- GPU preview toggle
- media hardware acceleration toggle
- autosave cadence
- cache size guidance or cleanup actions
- Google Fonts configured families
- local font directories (optional extra roots)

## 15. Out of scope for v1.0

- partial clear
- auto scene-cut clear insertion
- pen pressure
- arbitrary effect scripting
- automatic proxy generation
- full NLE-grade media management
