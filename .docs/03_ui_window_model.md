# UI and window model

## 1. Windowing stance

v1.0 uses a **single main window**.

Reasons:

- reduces cross-platform complexity
- avoids detached-window synchronization bugs
- simplifies persistence/restoration
- reduces Codex implementation risk

No detached floating tool windows are required in v1.0.

## 2. Main window layout

Recommended layout:

```text
+----------------------------------------------------------------------------------+
| Menu / project / transport / page info / status                                  |
+----------------------+------------------------------------+----------------------+
| Left rail            | Central canvas                     | Inspector            |
| - Media              | - Video preview                    | - Selection          |
| - Template           | - Overlay preview                  | - Style              |
| - Fonts              | - Capture interactions             | - Entrance           |
| - Presets            |                                    | - Post actions       |
|                      |                                    | - Template settings  |
+----------------------+------------------------------------+----------------------+
| Bottom tabs: Object Outline | Page Events | Export Queue | Logs                  |
+----------------------------------------------------------------------------------+
```

## 3. Required panels

### 3.1 Left rail

Sections:

- **Media**: file import, metadata summary, runtime diagnostics
- **Template**: text entry, underlay mode, slot-related controls
- **Fonts**: local fonts, Google Fonts, refresh, broken-entry visibility
- **Presets**: built-in and user preset browsing

### 3.2 Inspector

Context-sensitive.

Expected groups:

- Selection summary
- Base style
- Entrance
- Reveal-head effect
- Post-actions
- Group info
- Transform
- Template placement settings when template mode is active

### 3.3 Bottom tabs

- **Object Outline**
- **Page Events**
- **Export Queue**
- **Logs**

## 4. Modal dialogs

Allow these in v1.0:

- Open project
- Save As
- Import media
- Export
- Preferences
- Font manager / font refresh
- Cache manager
- Missing runtime / codec provider info
- Recovery prompt
- Error dialog

Avoid extra modal proliferation.

## 5. Transport controls

Required controls:

- play
- pause
- seek bar
- current time
- frame- or short-step controls if feasible
- insert clear

Insert Clear must work both while paused and while playing.

## 6. Visible state cues

The UI must make it easy to see:

- current page boundaries
- currently alive objects
- current selection
- current active template mode
- whether GPU preview is active or disabled/fallback
- whether media HW accel is active, unavailable, or disabled
- whether the active export path is hardware-assisted or software fallback

## 7. Avoidable complexity for v1.0

Avoid adding:

- scripting editor
- floating property windows
- partial-clear targeting UI
- nested multitrack media timelines
- per-object exit track editors
