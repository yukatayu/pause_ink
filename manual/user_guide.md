# User guide

This guide must be updated to match the real implementation before release.

## 1. What PauseInk is for

PauseInk lets you open a video, draw handwriting on top of it, replay the writing with reveal effects, and export either:

- the composited video, or
- just the annotation layer.

## 2. Key ideas

- A **page** is the part of the timeline between clear events.
- A **clear event** wipes all currently alive annotations.
- Handwriting remains your own strokes.
- Fonts are used for **templates and spacing**, not for silently replacing your writing.

## 3. Main workflows

### 3.1 Free ink

Draw directly on the frame.

### 3.2 Guide capture

Hold the guide modifier and draw one reference character to generate guides.

### 3.3 Template placement

Type text, choose a font/size/layout, place the template underlay, and write over the slots.

## 4. Saving and recovery

Project files use the `.pauseink` extension.  
Autosave and recovery are expected in v1.0.

## 5. Export overview

PauseInk separates:

- family (codec/container)
- profile (YouTube/X/Instagram/etc.)

The numeric values shown under non-Custom profiles are computed automatically.

## 6. Portable data

PauseInk keeps its mutable data next to the executable under `pauseink_data/`.

## 7. To be completed during implementation

Codex must expand this guide to include:

- screenshots or panel descriptions
- exact control names
- clear-event workflow
- export workflow
- cache/runtime troubleshooting
