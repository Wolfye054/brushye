# S-001 Selections: shapes, edit operations, use by Fill / Delete / masks, History
- Status: **observed** (shapes, Invert, Expand/Shrink, Fill, Delete, mask from selection: exact;
  ellipse/lasso edge rasterization: data recorded, model open; painting inside a selection:
  open)
- CSP version: 1.13.2 (PRO, 64-bit, build 202301210208)
- Needs: automated-Wine OK (strokes inside a selection: virtual pen, see open questions)
- Date / method / observer: 2026-11-14 / automated-Wine (Xvfb :77, XTEST) / Claude
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
  - The exact rule couldn't be measured: two XTEST mouse strokes differed in their faint edge
    rows (195, 204), so the masked and unmasked strokes aren't comparable. Open; needs the
    virtual pen (D-003), which is bit-repeatable.

### History (labels)
| Action | History entry |
|---|---|
| A selection made with a tool | the tool path, e.g. "Selection area/Selection area/Rectangle", "…/Ellipse" |
| Select > Deselect | Deselect |
| Expand / Shrink selected area | Expand selected area / Shrink selected area |
| Edit > Fill | Fill |

Selections are History steps (H-001).

## Open questions
- Ellipse and lasso edge rasterization (binary and AA). The data is in `data/`; analysis only.
- Painting inside a selection (brush alpha × S rule), with the virtual pen.
- Rounded Expand/Shrink at other widths; Expand/Shrink of AA edges; Blur border.
- Selection modes (add, subtract, intersect), Polyline, Selection pen / Erase selection,
  Shrink selection sub tool, Auto select, Select Color Gamut, Quick mask, selection layers.
