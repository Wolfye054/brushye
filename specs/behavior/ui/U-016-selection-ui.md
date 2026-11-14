# U-016 Selection tool, Selection Launcher, marching ants
- Status: **implemented** (2026-11-14: tool, sub tools, Tool property rows, Select menu, marching
  ants, Launcher; Aspect type, Adjust angle, Shrink selection and the disabled Launcher buttons
  wait on their features) / **observed** (layout from screenshots; tooltips by hovering)
- CSP version: 1.13.2 (PRO, 64-bit, build 202301210208)
- Date / method / observer: 2026-11-14 / automated-Wine (Xvfb :77, XTEST) / Claude
- Screenshots: `obs-out/m10/sel/u016_*.png` (not committed).

## Observations
- **Selection area tool** (tool column, tooltip "Selection area (M)"). Sub tools in order:
  Rectangle, Ellipse, Lasso, Polyline, Selection pen, Erase selection, Shrink selection.
- **Tool property (Rectangle / Ellipse):**
  - Selection mode: four buttons and a ▾. The first, new selection, is the default.
  - Aspect type: a checkbox with ⊕ and two disabled buttons.
  - Adjust angle after fixed: a checkbox.
  - Anti-aliasing: four buttons (none, weak, middle, strong) and a ▾. Rectangle shows None
    selected after the earlier runs.
- **Selection border:** dashed black-and-white "marching ants" along the selection edge.
- **Selection Launcher:** a dark bar just below the selection's bounding box, left-aligned
  with it. Buttons in order (tooltips): Deselect · Crop · Invert selected area · Expand
  selected area · Shrink selected area · Delete · Delete Outside Selection · Cut and paste ·
  Copy and paste · Scale/Rotate · Fill · New Tone · | · Selection Launcher Settings.
- **Select menu:** Select All (Ctrl+A) · Deselect (Ctrl+D) · Reselect (Ctrl+Shift+D) · Invert
  selected area (Ctrl+Shift+I) · ─ · Expand selected area… · Shrink selected area… · Blur
  border… · ─ · Select Color Gamut… · ─ · Select Overlapping Vectors · Select Vectors Within
  Area · ─ · Quick Mask · Convert to Selection Layer · Convert Selection Layer to Selection
  (seen in U-002 and this run).
- **Expand selected area dialog:** Expansion width (px, default 1), Expansion type (square,
  rounded), OK and Cancel.
- **Shrink selected area dialog:** Shrinking width, Shrinking type (square, rounded), "Shrink at
  edge of canvas" (✓).

## Open questions
- Selection Launcher position when the selection is near the canvas bottom, and the Launcher
  Settings dialog.
- Modifier keys for the selection modes (the buttons themselves: S-001 run 2).
- Polyline closing gesture (Brushye: click the first corner, double-click or Enter; Esc
  cancels) and a click without a drag (Brushye: nothing): not observed.
