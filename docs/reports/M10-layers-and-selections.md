# M10 report, parts 1–2: layer structure and selections (2026-11-15)

**Status: parts 1 and 2 are done.** M10 itself goes on with the other post-alpha painting
features (ROADMAP). Everything below is pushed. Decisions:
- **D-023** (layer structure: storage, commands, file format). Proposed, then trimmed at the
  maintainer's request to storage only, with all behaviour 1:1 from observation. Accepted.
- **D-024** (selections: document state, shape commands, `.brsh` v4). Accepted.

No new dependencies.

How this milestone was worked: every behaviour was observed in CSP 1.13.2 under Wine before it
was implemented (the maintainer's rule: "don't guess CSP behaviour"). Where a rule couldn't be
pinned down, the spec says so and Brushye's approximation is labelled as one.

## What works

### Part 1: layer structure (D-023)
- **Folders and Through.**
  - A normal folder composites its contents in isolation, then blends like one layer with
    its opacity and mask.
  - A Through folder blends its contents straight into the backdrop, then applies one lerp:
    weight `(w_op·w_M) >> 15`, with `w_op = (o8+1)·128`.
  - Exact against every L-004 export (runs 1–4).
- **Clip to Layer Below.** The clipped layers composite on the base colour, treated as
  opaque. The group keeps the base's alpha and blends like the base.
  - A Through base turns clipping off.
  - A hidden base hides its group, and a layer with no base draws unclipped.
  - Clipped folders clip like layers.
  - Exact against every L-006 export.
- **Layer masks** on layers and folders: values 0–255, applied after opacity as
  `floor(a·M/255)`.
  - Supported: enable, link and Show Mask Area. Show Mask Area is a blue tint, fitted on
    screenshots.
  - Commands: Create, Mask Outside Selection, Mask Selection, Delete and Apply.
  - Mask painting with any brush.
  - Exact against every L-008 export.
- **Layer palette** per U-014 and U-015:
  - folder rows with open/close
  - the clip bar, and Through shown in red when clipping is off
  - mask thumbnails, with the mask or the layer as the paint target
  - drag and drop with CSP's above/into/below zones; Layer Order and Alt+[ / Alt+]
  - New Layer Folder, Create folder and insert layer (Ctrl+G), Ungroup (Ctrl+Shift+G) and
    Clip (Ctrl+Alt+G)
  - CSP's names for new layers and folders, and which layer is selected after each
    operation (U-014, U-015).
- **History** labels and merging per H-002. For example, toggling mask enable merges;
  showing or hiding a layer isn't undoable.
- **`.brsh` v2 and v3** (parent, clip and expanded flags; masks), with migrations and golden
  files.
- **PNG export** writes white under fully transparent pixels, as CSP does.

### Part 2: selections (D-024, S-001 runs 1–6, U-016)
- **The selection is document state:** an 8-bit sparse grid (`Grid8`, shared with masks).
  Changing it is an ordinary undo step. Deselect keeps it for Reselect.
- **Shapes and modes** (`select.rect/ellipse/polygon`, `mode=new|add|subtract|intersect`):
  - rectangle: the integer box, exact
  - polygon without AA: pixel centres, exact
  - polygon with AA: four sub-scanlines at y + k/4, within ±1
  - modes: Add `min(A+B,255)`, Subtract `max(A−B,0)`, Intersect `floor(A·B/255)`, exact.
- **Edits:**
  - Select All, Deselect, Reselect.
  - Invert: `255 − S`.
  - Move: an integer shift.
  - Expand/Shrink:
    - square: grey-level max/min, exact on hard and soft edges
    - rounded: CSP's measured kernel for widths 1–30, 35, 40, 50, 60, 80 and 100, exact;
      other widths use a disk approximation.
  - Blur border: a separable Gaussian, σ = 0.3 × strength, floored after each pass. Exact at 6
    of the 14 observed strengths and ±1 at the rest; 8 corner pixels are ±2 at strength 50.
- **Using a selection:**
  - Strokes are clipped by multiplying the unquantised stroke alpha by S/255. This
    reproduces all 33 observed values (alpha and colour) and works with every ink mode, in
    the live preview too.
  - Edit > Fill lays on the colour like Normal ink with alpha S: exact over opaque pixels, and
    one pixel off out of 34,849 over partly transparent ones.
  - Edit > Delete gives `floor(a·(255−S)/255)`, exact.
  - Mask (Outside) Selection: M = S or 255 − S, exact.
- **Selection Pen and Erase Selection** are `brush.stroke … target=selection mode=add|subtract`.
  They replay with the full input stream, like any stroke.
- **Layer > Selection from Layer:**
  - Create: `A`.
  - Add: `A + B − ⌈A·B/255⌉`.
  - Delete: `⌊B·(255−A)/255⌋`.
  - Overlap: `⌊A·B/255⌋`.
  - All exact on 2,592 pixels where both values are partial. They differ from the tool modes.
- **Queries:** `?selection` (whether anything is selected, its bounds, and whether Reselect is
  available) and `?selection.pixel`.
- **The app** (U-016):
  - **Selection area tool:** its seven sub tools, with the selection mode as a sub tool
    setting, and Tool property rows in CSP's order.
  - **Canvas gestures:**
    - rectangle and ellipse drags
    - dragging inside the selection moves it (New mode)
    - lasso
    - polyline: click the first corner or double-click to close
    - a click outside the selection deselects
    - Selection pen and Erase selection strokes.
  - **Display and menus:**
    - marching ants
    - the Selection Launcher under the selection
    - the Select menu with the Expand, Shrink and Blur dialogs
    - Edit > Fill/Delete and Layer > Selection from Layer.
- **`.brsh` v4** stores the selection and the Reselect selection, with a v3→v4 migration and a
  golden file.
- **Script example** `examples/scripts/selections.bcmd`.

## Verification
- **Tests:** 217 workspace tests pass (164 at the end of M9), and clippy is clean.
  - Observed-data tests (`crates/brushye-core/tests/observed_*.rs`) compare the renderer and
    the selection code with CSP's own exports, pixel by pixel. That covers:
    - folders, Through and clipping, and masks
    - shapes, modes, morphology and the kernels
    - Blur border and Selection from Layer
    - fill over existing pixels, and strokes inside a selection.
  - Command, replay and format tests for every new command. Golden `.brsh` v2, v3 and v4 files.
- **CI** is green on Windows, macOS and Linux for every M10 push, through this report.
- **In the app** (Linux, automated on a private X display):
  - layer structure: the palette, drag and drop, masks
  - selections: rectangle and ellipse, Add mode, Fill, moving, the Selection pen, Deselect
    and Reselect from the menu, the Launcher.
  - Not clicked through: lasso, polyline, invert, the Expand, Shrink and Blur dialogs,
    Selection from Layer, and the keyboard shortcuts. Keys don't reach the window on that bare
    display.
- **Observation:** all automated under Wine:
  - virtual pen for strokes
  - XTEST mouse for the rest
  - exported PNGs for every value.

  Nothing in M10 looked Wine-specific, so nothing is flagged for a native recheck.

## What doesn't work yet / limits
- **Ellipse and lasso edges are approximate.** CSP samples the same four sub-scanlines, but its
  curve sits slightly inside the true ellipse, away from the box sides (S-001 run 4).
  Polygons, Bézier flattening, insets and rounding variants all come close and none is exact.
  Brushye is within a few levels on small ellipses and up to ≈66 on large ones. D-024 allows
  the edge to change before 1.0, which would change what old journals re-select. Saved files
  keep their pixels.
- **Rounded Expand/Shrink** is exact only at the measured widths. How CSP builds the kernel is
  unknown: it's 8-fold symmetric, but not a disk.
- **Blur border** is ±1 at some strengths. The canvas-edge treatment and which pass comes first
  weren't observed.
- **Not built yet:**
  - Shrink selection sub tool, Auto select, Select Color Gamut, Quick Mask and selection
    layers
  - the Launcher's Crop, Delete outside, Cut/Copy and paste, Scale/Rotate and New tone
  - Aspect type and Adjust angle (shown disabled).
- **Modifier keys** for the selection modes weren't observed (the UI driver can't hold a key).
- **Layer structure, still open in the specs:**
  - nested-folder rendering (built recursively, but not observed)
  - Through opacity and mask order
  - several clipped layers on one base in other modes
  - lock-transparency colour at AA edges
  - masks painted with partial ink
  - Show gradients values other than Yes
  - a few History labels (Delete Mask, moves by drag).

## Waiting on the maintainer
- **A desktop check** of selections and layer structure, on your own machine (any OS):
  - draw a few selections and move one
  - paint inside a selection with the pen
  - the shortcuts: Ctrl+A, Ctrl+D, Ctrl+Shift+D, Ctrl+Shift+I, Alt+Del and Del
  - Blur border and Selection from Layer.
- **From M9:** still pending is the native Windows/macOS check of MCP attach mode.
- **Observation specs:** no draft is waiting. The open items above are listed in L-004,
  L-006, L-008, U-014, U-015, H-002 and S-001.

## Proposed next step
**M10 part 3: the Fill tool and Auto select.** Both flood-fill from a click by colour, with
the same reference options:
- reference: editing layer, all layers or reference layer
- tolerance, area scaling and gap closing.

So one algorithm and one observation spec serve both. Auto select also finishes the Selection
area work, and Fill is the most-used post-alpha tool.

The plan, as for parts 1 and 2:
1. Observe first (spec F-001: tolerance metric, anti-aliased edges, area scaling, gap
   closing).
2. Then implement as `fill.*` and `select.auto` commands.

This needs no architectural change: it reuses the selection grid and the layer pixels.
