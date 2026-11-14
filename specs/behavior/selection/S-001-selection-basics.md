# S-001 Selections: shapes, edit operations, use by Fill / Delete / masks, History
- Status: **implemented** (D-024, 2026-11-14: everything marked exact; the ellipse/polygon
  edge rule is an approximation until fitted) / **observed** (shapes, Invert, Expand/Shrink, Fill, Delete, mask from selection: exact;
  selection modes, painting inside a selection, Selection pen / Erase selection: exact (run 2);
  ellipse/lasso edge rasterization: data recorded, model open)
- CSP version: 1.13.2 (PRO, 64-bit, build 202301210208)
- Needs: automated-Wine OK (strokes inside a selection: virtual pen, see open questions)
- Date / method / observer: 2026-11-14 / automated-Wine / Claude. Run 1: Xvfb :77, XTEST mouse.
  Run 2: pen display Xorg :78 with the virtual pen (D-003) for strokes, XTEST for the rest.
- Data: `data/*.png`, re-encoded without the iCCP chunk.

## Setup (exact)
- **Canvas:** File > New, 800×800 px, 72 dpi, no paper; one empty raster layer "Layer 1"; 100 %
  zoom. The canvas origin is at screen (543, 163).
- **Selection:** Selection area tool (tool column), sub tool and Anti-aliasing as listed
  below, then a mouse drag. Rectangle and ellipse are dragged from canvas (100,100) to
  (300,250), or from (400,100) to (650,300).
- **Readout:** Edit > Fill (main colour black) fills the selection, then File > Export
  (Single Layer) > .png, then Edit > Undo. Exported **alpha = selection value S**
  (0 = not selected, 255 = selected).

## Observations
### Shapes
- **Rectangle:** covers [start, end) of the drag in canvas pixels, here x 100–299, y 100–249,
  binary.
  - At 100 % zoom the corners are whole pixels, so AA None and AA Middle are identical
    (`rect_aa_none`, `rect_aa_middle`).
- **Ellipse:** exactly fills its box [400,650) × [100,300).
  - **AA None** is binary (`ellipse_aa_none`, 39 188 px).
  - **Weak, Middle and Strong are pixel-identical** (`ellipse_aa_weak/middle/strong`): 38 802
    full pixels plus 751 partial edge pixels. The selection AA setting therefore acts as off
    or on.
  - Rasterization rule: open. Pixel-centre sampling of the true ellipse is 88 px off
    (binary), and supersampled coverage is not exact either.
- **Lasso** (polygon (100,400)→(300,420)→(250,600)→(120,560)→close, AA Middle): fills
  [100,300) × [400,600) with partial edge pixels (`lasso_aa_middle`). Edge rule open, like
  the ellipse.
- **Dragging inside an existing selection with the Rectangle sub tool moves the selection** by
  the drag vector (`rect_aa_middle` came out shifted by (200,150) the first time). A new
  selection needs a press outside the current one, or a Deselect first.

### Select menu operations
- **Invert selected area:** S → 255 − S (`rect_inverted`: 610 000 = 640 000 − 30 000).
- **Expand selected area** (dialog: Expansion width in px, default 1; type: square or
  rounded):
  - **square** = dilation by a (2w+1)² square, exact (`expand10_square`, 220×170).
  - **rounded** = dilation by the disk dx²+dy² < 121 at w = 10, exact (`expand10_round`). Any
    threshold from 117 to 120 gives the same disk; consistent with d² < (w+1)².
- **Shrink selected area** (dialog: Shrinking width; the same two types; "Shrink at edge of
  canvas" ✓ by default):
  - square = erosion by the square; rounded = erosion by the same disk. Both exact on the
    rectangle (`shrink10_*`, the same 180×130 for both) and on the ellipse
    (`ellipse_shrink10_square` 30 596 px, `ellipse_shrink10_round` 32 000 px).
- The AA edges of an AA selection under Expand/Shrink were not run (open).

### Using a selection
- **Edit > Fill** fills the selection with the main colour: alpha = S (the readout).
- **Edit > Delete** on an opaque layer leaves alpha = **255 − S** exactly
  (`delete_in_ellipse`).
- **Layer > Layer Mask > Mask Outside Selection** with a selection makes a mask **M = S**
  exactly (`mask_outside_ellipse`): inside revealed, outside hidden.
- **Painting with a selection:** a pen stroke is clipped to the selection.
  - Run 2 measured the rule (below, "Painting inside a selection").

### History (labels)
| Action | History entry |
|---|---|
| A selection made with a tool | the tool path, e.g. "Selection area/Selection area/Rectangle", "…/Ellipse" |
| Select > Deselect | Deselect |
| Expand / Shrink selected area | Expand selected area / Shrink selected area |
| Edit > Fill | Fill |

Selections are History steps (H-001).

## Run 2 (2026-11-14, pen display :78)
Same canvas setup. Strokes use the virtual pen: a horizontal line at full pressure, a 3 px step
every 8 ms (`tools/observe` vpen fifo; script `move`/`wait` lines as in `pen_grid.py`).

### Painting inside a selection
- **Repeatability:** the same virtual-pen stroke twice with Real G-Pen (size 10, stabilization 6,
  Adjust by speed on) is *not* bit-identical (`stroke_nosel_1/2`): the edge rows shift by a
  sub-pixel, but the core rows are 255 in both. So the rule is fitted where the unclipped
  stroke alpha is known: in the stroke core.
- **Opaque stroke** (opacity 100) across the AA ellipse [400,650)×[100,300)
  (`stroke_in_ellipse`, selection = `ellipse_aa_middle`): wherever the core is 255, the
  painted **alpha = S** exactly; nothing is painted where S = 0.
- **Stroke at opacity 93** (core alpha 238 without a selection): on all 19 partial edge pixels
  in the core rows, **alpha = floor(238 · S / 255)** (`stroke93_in_ellipse`). Rounding,
  ceiling and `·/256` each miss several.
- **The same stroke over an opaque white fill** (`stroke93_over_white`): edge colours equal the
  brush engine's normal stroke merge with stroke alpha floor(238 · S / 255) (12 of 14 checked
  values reproduce Brushye's current Normal composite). The two alpha 237/238 values are 19/18
  where L-003's layer composite gives 18/17; Brushye's brush engine already gives 18 for the
  unselected stroke, so the selection only scales the stroke alpha.
- Rule: **the stroke's unquantised alpha a (opacity × stroke buffer, B-003) is multiplied by
  S/255, then the stroke merges into the layer as usual.** With a = 0.93 this reproduces all 33
  observed values: the 19 alphas on the empty layer (= floor(238·S/255) there) and the 14
  colours over white. Brushye: `brush::stroke_clipped`; `crates/brushye-core/tests/
  observed_selection.rs` checks 24 edge pixels bit-exact.

### Selection modes (Selection area tool, Tool property "Selection mode")
The four buttons are New, Add, Subtract and Intersect. In Add/Subtract/Intersect, dragging
inside the current selection makes a new shape; it doesn't move the selection.
- Hard rectangle A, then AA ellipse B (drag (200,180)→(400,350)) (`mode_add/sub/int`): Add is
  255 inside A, Intersect is B inside A, and Subtract + Intersect = 255 inside A. Reversed order
  (ellipse first, then add the rectangle, `mode_add_rev`) gives the same result as `mode_add`.
- To separate max/min from sum/product, **A = Layer > Selection from Layer > Create Selection**
  of a layer whose alpha is a 93 % Real G-Pen stroke (alpha 238 in the core, partial values at
  its edge; the selection equals the layer alpha exactly, `layer_sel_A`), combined with the AA
  ellipse B (`ellipse_B`, 605 partial pixels), `Ae_add/sub/int`:

| Mode | Rule | Mismatches |
|---|---|---|
| Add | **min(A + B, 255)** | 0 (max: 428, screen: 393) |
| Subtract | **max(A − B, 0)** | 0 (min(A, 255−B): 428, A·(255−B)/255: 387) |
| Intersect | **floor(A · B / 255)** | 0 (min: 428, rounded product: 195) |

  Two AA ellipses (`ellipse_e1`, `ee_*`) agree but separate the rules on one pixel only.

### Selection pen / Erase selection
- Sub tools of Selection area. Defaults: Selection pen = mode **Add**, Erase selection = mode
  **Subtract**; both Brush Size 30, Anti-aliasing **none**, Hardness, Brush density 100,
  Stabilization 2.
- The same virtual-pen line (canvas (200,300)→(599,300)): Selection pen on no selection gives
  a binary stroke shape (`selection_pen`, 12 664 px, y 285–314, x 185–613). Erase selection
  after Select All removes exactly the same pixels (`erase_selection` = 255 − `selection_pen`).
  With AA none, the two strokes came out identical.
- Brushye's brush engine (`brush.stroke size=30 aa=none`, samples at the canvas positions)
  draws the same shape except 16 px at the stroke ends (stabilizer and timing). So the
  selection pen is a brush stroke written into the selection, combined by the tool's mode.

### Harness notes
- Ctrl+A in a Tool property number field does Select All on the canvas; the field doesn't
  take it.
- Menu items in a submenu need a hover pause before the click.

## Open questions
- Ellipse and lasso edge rasterization (binary and AA). The data is in `data/`; analysis only.
- Rounded Expand/Shrink at other widths; Expand/Shrink of AA edges; Blur border.
- Polyline, Shrink selection sub tool, Auto select, Select Color Gamut, Quick mask, selection
  layers; Selection pen with anti-aliasing on (soft values under Add).
- Layer > Selection from Layer > Add / Delete / Select Overlapping Area (whether they use the
  same arithmetic as the tool modes).
