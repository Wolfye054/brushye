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

## Run 3 (2026-11-14, pen display :78, mouse via XTEST)
Same setup; data in `data/run3/`. Readouts as before (Edit > Fill on an empty layer, alpha = S).

### Ellipse and polygon edges
- Seven ellipses in one selection (New, then Add), boxes [100,111)×[100,107), [150,187)×[100,121),
  [250,351)×[100,151), [400,500)×[100,200), [550,553)×[100,103), [600,780)×[100,113),
  [100,113)×[300,500); AA Middle (`ells_aa_middle`) and None (`ells_aa_none`). Every ellipse
  fills exactly its box.
- **AA:** partial values climb in steps of 64 (64, 128, 191 plateaus with one partial pixel
  between them), so each pixel row is sampled by **four sub-scanlines**, each adding its exact
  horizontal coverage × 64. The sub-scanlines sit at **y + k/4, k = 0..3**: the top-centre
  pixel of the 11×7 ellipse is 193, not 191 (the line y = 100 grazes the ellipse), and the
  shapes are shifted ⅛ px down (bottom rows more covered than top rows).
- The ellipse itself is not exactly the true ellipse of the box: measured per sub-scanline,
  the edge of the 100 px circle runs at an effective radius 49.6–49.9 px away from the equator
  (50.0 there). An inscribed polygon (8–256 sides) and a uniform radius shrink don't fit either.
  **Open** (Brushye uses the true ellipse with the k/4 sub-scanlines: about 2 000 pixels off
  by up to 66 over the eight ellipses).
- AA None: the 3×3 ellipse selects all 9 pixels; the larger ones are close to, but not
  exactly, a pixel-centre test. Open with the curve.
- **Polyline triangle** (100,100) → (300,130) → (150,300), corners by clicks (`tri_aa_none`,
  `tri_aa_middle`): AA None = **pixel centres inside, exact**. AA Middle = the k/4
  sub-scanlines with exact coverage and `round(255·c/4)`: 438 of 649 edge pixels exact, the
  rest +1 (CSP rounds those up; no simple rounding variant tried fits all).
- **Polyline closing:** clicking the first corner closes the shape; so does a double-click on
  the last corner.

### Click without a drag
- Rectangle sub tool, New mode: a click outside the selection **deselects**; a click inside
  keeps it (it is a zero-length move).

### Fill and Delete over existing pixels
- Black layer, white fill in the AA ellipse E1 (`fill_white_over_black`, S = `S_E1`): colour =
  S exactly on all 23 778 pixels, alpha stays 255.
- A partly transparent backdrop (`del_backdrop`: Edit > Delete of a moved E1 on the black
  layer; alpha = 255 − S exactly, and the moved selection is S_E1 shifted by exactly the drag
  vector (200,150)), then a white fill in E3 (`fill_white_over_partial`, S = `S_E3`):
  Brushye's Normal ink with stroke alpha S/255 gives 34 848 of 34 849 pixels exactly (one
  pixel's alpha 134 vs 135).

### Expand / Shrink
- On the AA ellipse E1, width 5: square and rounded Expand, square and rounded Shrink are all
  **grey-level max/min** over the square or the disk dx² + dy² < 36, exact (`eE1_*`).
- Rounded Expand of the rectangle at w = 1, 2, 3, 25 (`rect_exp*_round`): w = 1 and 2 fit
  d² < (w+1)² (they equal the square). **w = 3 gives the full square** (profile 3,3,3), and
  w = 25 gives the per-row extents 25,25,25,25,25,25,24,24,24,23,23,22,22,21,21,20,19,18,17,
  16,15,13,11,9,6, which match neither d² < 676 nor the AA-none circles of diameter 51/53
  (`circles_none`). Rounded Shrink at w = 3 cuts a 3×3 corner (`rect_shr3_round`), consistent
  with the square too. **Open:** the rounded kernel's shape.

### Blur border
- Select > Blur border…: one field, "Strength" (px, default 1.00). On the rectangle
  (`rect_blur1/5/20`): strength 1 changes only the edge row (254); 5 gives a smooth edge
  (…, 93 | 161, …) close to a Gaussian with σ ≈ 1.5; 20 a wider one (σ ≈ 6–7, not one
  Gaussian). Open; not in Brushye.

### Layer > Selection from Layer
- Layer A = the AA ellipse E1 filled on Layer 1 (alpha = S_E1); selection B = an AA ellipse
  (`B_sel`); readout on Layer 2 (`fromlayer_add/del/int`). Only one pixel has both values
  partial (A = 90, B = 171): **Add Selection** 200, **Delete Selection** 110, **Select
  Overlapping Area** 60. That fits products, floored: 255 − (255−A)(255−B)/255, B·(255−A)/255
  and A·B/255, **not** the tool modes' saturating add/subtract (which give 255 and 81). Weak
  (one pixel); not in Brushye.

## Run 4 (2026-11-14, pen display :78): the ellipse curve
- 52 drags in one selection (`data/run4/`, boxes in `boxes.txt`, canvas coordinates
  `x0 y0 x1 y1`): circles of diameter 1–40 on a 50 px grid, and ellipses 12×5, 5×12, 20×7,
  7×20, 30×9, 9×30, 41×13, 64×21, 80×33, 100×7, 7×100, 150×40; AA None (`ell_none`) and AA
  Middle (`ell_aa`). The 1×1 and 2×2 drags selected nothing (too small, or the drag was too
  short to register) and the 150×40 drag was lost; the other 49 fill exactly their boxes.
- **AA None is not a threshold of the AA result** (≥ 268 pixels differ at any threshold), and
  not the pixel-centre test of the true ellipse (exact up to d = 8; d = 9 has rows 3,7,7,9…
  where the centre test gives 5,7,9…; 178 pixels over all shapes).
- Inscribed polygons with evenly spaced vertices reproduce most AA None shapes one at a time,
  but the vertex count that fits varies with size (≈ 8 for d ≤ 9, 20–36 for d ≈ 20–40, none
  for 64×21). Four cubic Bézier arcs flattened into n equal-t segments fit best at n = 8–9
  (98 pixels off over all shapes), not exactly.
- **AA:** the k/4 sub-scanlines are confirmed. A sub-scanline that only grazes the top of the
  ellipse leaves the centre pixels at **192 = 3 × 64**, so a full sub-scanline seems to count
  64 (sum clipped at 255) rather than 63.75. The values sit **2–7 below** the true ellipse's
  coverage even on the 3×3 to 6×6 circles (3×3: 109 192 109 / 245 255 245 / 166 255 166 against
  111 191 111 / 247 255 247 / 170 255 170), more in rows away from the box's middle. A
  Bézier circle with a smaller control-point factor (≈ 0.54 instead of 0.5523) reduces the
  largest error to 9–25 for d ≤ 40, but no combination tried (kappa, polygon count and phase,
  horizontal inset 0–0.15 px, per-sub-scanline or summed quantization at 64 or 63.75) is
  exact: about 200 of the edge pixels of the 3–12 px circles stay off.
- **Status: open.** Brushye keeps the true ellipse with k/4 sub-scanlines (within a few
  levels on small ellipses, up to ≈ 66 on large ones). A further pass needs a different
  probe, e.g. selections exported at several zoom levels, or the selection outline
  (Edit > Outline selection) to see the curve itself.

## Run 5 (2026-11-14, pen display :78): the rounded Expand/Shrink kernel
- **Seed:** a drag of 1×1 or 3×3 px selects nothing (CSP treats it as a click), so the seed is
  a 21×21 rectangle shrunk by 10 (square): exactly one pixel, (400, 400). Then Select > Expand
  selected area…, Rounded, width w, and the readout: the result is the kernel itself.
  Widths 1–30, 35, 40, 50, 60, 80, 100 (`data/run5/rounded_kernels.json`: per width, the
  half-width of rows dy = 0..w; images `k_14`, `k_25`, `k_100`).
- Every kernel is binary, symmetric in x, y and the diagonal, never wider than w in either
  axis, and exactly described by its row half-widths. **It is not a Euclidean disk:** the
  squared-radius ranges consistent with the data are irregular (e.g. w = 11: r ∈ (12.04,
  12.08]; w = 13: (13.89, 13.93]) and w = 14 and 16 fit no disk at all (w = 14 contains
  (12, 9), d² = 225, but not a point with d² = 221). Widths 1–3 are the full square.
  Midpoint-circle variants, regular polygons (centre test), offset squares and CSP's own
  selection circles (run 4) were tried; none reproduces all widths. The construction is
  **open**.
- **Rounded Shrink erodes by the same kernel:** a 300×300 rectangle (`r300`) shrunk by 14 and
  by 16 (`s_14`, `s_16`) equals erosion by the measured kernels, exact.
- Brushye uses the measured kernels for those widths (exact; tests reproduce k_14, k_25,
  k_100, s_14, s_16 and run 3's w = 3 and 25) and the disk `dx² + dy² < (w+1)²` for other
  widths (exact at several measured widths, within a pixel per row at the others).

## Run 6 (2026-11-15, pen display :78): Blur border and Selection from Layer
Data in `data/run6/`.

### Blur border
- Select > Blur border…, Strength 0.5, 1, 1.5, 2, 3, 4, 5, 6, 8, 10, 15, 20, 30, 50 on the
  rectangle [100,400)² (`b_<strength>`).
- Rule: **a separable Gaussian, σ = 0.3 × strength, over offsets −r..r with r = max(⌊strength⌋,
  1), horizontal pass then vertical, each pass floored to 8 bits.** Whole images exact at 0.5,
  1, 1.5, 2, 5 and 6; the other strengths differ by ±1 on scattered pixels (3: 2 384 px at the
  edge, 30: 6 242, 50: 8 480 of which 8 are off by 2), consistent with a different arithmetic
  precision; fixed-point weights at 256/1024/4096/65536 don't remove them. Strengths 0.5
  and 1 only lower the inner edge row to 254 (σ ≤ 0.3).
- Which pass comes first and the canvas edge treatment can't be told from this data
  (Brushye: horizontal first, outside the canvas = unselected).

### Layer > Selection from Layer
- Layer 1 = the rectangle [100,400)² blurred at 20 and filled (alpha A, `A`); selection B =
  [300,600)² blurred at 20 (`B`); readouts on Layer 2 (`fl_*`). 2 592 pixels have both values
  partial.
- **Create Selection** = A. **Add Selection** = A + B − ⌈A·B/255⌉. **Delete Selection** =
  ⌊B·(255−A)/255⌋. **Select Overlapping Area** = ⌊A·B/255⌋. All exact on every pixel; these are
  not the tool modes' rules (saturating add/subtract, run 2). Add/Delete/Overlap are greyed
  in the menu without a selection.
- Brushye: `select.blur strength=`, `select.from_layer L op=create|add|delete|overlap`; the
  Select menu's Blur border… and the Layer menu's Selection from Layer submenu.

## Open questions
- Ellipse edge curve (AA and None; run 4 data and the fits that failed); polygon AA rounding
  (+1 on a third of the edge pixels).
- The rounded kernel's construction (measured for 36 widths, run 5; unmeasured widths use a
  disk approximation).
- Blur border's ±1 precision at some strengths; the layer alpha used by Selection from Layer
  for layers with a mask or opacity (not observed).
- Shrink selection sub tool, Auto select, Select Color Gamut, Quick mask, selection layers;
  Selection pen with anti-aliasing on; modifier keys for the modes.
