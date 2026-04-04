# Project overview

## Product intent

PauseInk is a desktop tool for adding handwritten overlays to video with a workflow centered on:

- loading a local video,
- pausing or playing it,
- drawing annotations directly over the frame,
- optionally using guides or template slots,
- replaying those annotations with reveal effects,
- clearing the page manually at chosen moments,
- exporting either the composite or the annotation layer.

This is not a general nonlinear editor, not a whiteboard recorder, and not a font substitution tool.

## Primary users

- Vlog editors who want casual handwritten overlays
- commentary creators who pause and annotate scenes
- VOICEROID / synthetic voice creators who want “light handwritten note” aesthetics
- users who prefer a portable, install-light workflow

## v1.0 positioning

v1.0 must optimize for simplicity, predictability, and portability:

- single main window
- manual clear only
- no arbitrary partial clear
- no runtime-scripting hot path
- no dependence on OS-level app-state directories
- no mandatory GPU
- no required browser runtime
- no required systemwide install

## Key language

- **Stroke**: one pen-down to pen-up capture
- **Glyph Object**: one handwritten character-like object, possibly composed of multiple strokes
- **Group**: a user-created collection of glyph objects and/or strokes that share reveal behavior
- **Page**: the timeline interval between clear events
- **Clear Event**: a screen-wide event that removes all alive annotations on the page at that time
- **Template Slot**: a layout cell derived from input text and font/template settings
