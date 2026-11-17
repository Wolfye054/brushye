# F-003 Fill tool: Paint unfilled area
- Status: **observed** (run 1: settings, the region rule; run 2: the stroke's footprint;
  open items below)
- CSP version: 1.13.2 (PRO, 64-bit, build 202301210208)
- Date / method / observer: 2026-11-16 / automated-Wine (pen display :78, XTEST mouse) /
  Claude
- Data: `data/unfilled/run1/` (exports re-encoded without iCCP). Inputs `un1`, `en1` from
  `tools/observe/make_fill_inputs.py` (our own test images, not CSP material).
- Related: F-002 (Enclose and fill: the same regions, with a lasso).

## Setup (exact)
- File > Open the input, Layer > New Raster Layer on top; Fill tool, sub tool **Paint
  unfilled area** (the fourth); main colour black.
- Strokes are mouse drags (XTEST: press, moves of a tenth of each segment 30 ms apart,
  release); taps are single clicks. Then the input layer is hidden, File > Export (Single
  Layer) > .png, the layer shown again, Edit > Undo when the export has pixels. Exported
  **alpha = the fill**.
- **Screen → canvas:** a click at screen pixel (X, Y) lands at canvas (X − 543 + ½,
  Y − 163 + ½): a Real G-Pen click at (1043, 663) gave a dab centred at (500.48, 500.58)
  (`Z_pen_dot`). Below, canvas positions are those of the pixel centres the pointer hit.
- Every setting change checked on a screenshot.

## Observations
### Settings (UI, defaults)
- **Tool property:** Target color ("Treat semi-transparent as transparent"), **Tolerance
  80.0**, Area scaling ☐, Refer multiple ✓, **Brush Size 20.0**. No Close gap row.
- **Sub Tool Detail** categories: Closed area fill, Reference, Brush Size, Ink,
  Anti-aliasing, Brush shape (Brush tip, Spraying effect, Stroke), Correction, Starting
  and ending.
  - Closed area fill: Target color; Close gap ☐ (five-step bar); Tolerance 80; Area
    scaling ☐ (Scaling mode).
  - Reference: Refer multiple ✓; Exclude; Refer to image border ✓; Reference overflow
    frame ☐; Fill up to vector path ☐ (Include vector path).
  - Brush Size 20 (Specify by size on screen ☐, At least 1 pixel ☐). Ink: Opacity 100,
    Blending mode Normal. **Anti-aliasing: off.**
  - Brush shape: a preset list (Pen, Pencil, Air brush, Spray, Watercolor, Waved line,
    Rough edged, Bumpy, Dashed line, Dotted line, Heart, Star, Laced ribbon, Melody); Brush
    tip: Thickness 100, Direction Horizontal, Angle 0; Spraying effect ☐; Stroke: Gap (four
    presets and a slider), Continuous spraying ☐, Correct velocity input ✓, Ribbon ☐.
  - Correction: Sharp angles ☐, Stabilization 0, Adjust by speed ☐, Post correction ☐,
    Taper. Starting and ending: None (Specify length; Starting 20, Ending 20, by speed ✓).

### The region rule (run 1)
`un1`: white with a grid of 10×10 white cells (18 px, 2 px black lines) at (60, 60); an
empty 200 px square outline at (400, 60); a black bar (60–330, 400–420) with white holes
of 1, 2, 3, 4, 6, 8 px; a square outline at (400, 400) around a gray (128) square.
- **Brush 20** (defaults): **S3**, a stroke along the bar from (50.5, 410.5) to (340.5,
  410.5): **all six holes** filled (130 px = 1 + 4 + 9 + 16 + 36 + 64), nothing else. A
  stroke along the first grid row (S1), inside the empty square (S2), on open background
  (S4), across the gray square (S5) and a 10 px stroke inside one cell (S6): **nothing**.
- **Brush 60:** **E1**, a stroke from (70.5, 91.5) to (250.5, 91.5): the **26 cells**
  wholly covered by the stroke (rows 1–3 of the grid without the four corner cells, which
  reach 30.08 px from the end dabs' centres); no grid line. **E2**, a tap at (131.5,
  131.5): **the one cell** under it (324 px). E3 (background), E4 (inside the 200 px
  square): nothing.
- **Black regions too** (`en1`, brush 60 taps): on a 30 px outlined square (F1) **the
  whole square, outline included** (900 px), on the 40 px square (F2) 1600 px.
- **Rule:** the regions are those of Enclose and fill (F-002): every region lying
  **entirely inside the area the stroke covered** is filled, a region reaching outside it
  (the background, the grid lines, a square bigger than the brush) is not.
- **The footprint of a tap** (G, `en1`'s 40 px square tapped at its centre (580.5,
  100.5), brush 54–58): sizes 54, 55, 56 fill only the square's inside (1156 = 34²; the
  outline reaches 28.28 px from the tap), 57 and 58 the whole square (1600). A pixel is
  covered when **its centre lies within size / 2 of the tap** (28.28 ≤ 28.5, > 28; whole
  pixels within the radius would have needed size 58).

## Run 2 (2026-11-16, pen display :78): the stroke's footprint
Data in `data/unfilled/run2/`; input `dab1` (`tools/observe/make_fill_inputs.py`): a
full-canvas black/white checkerboard. No pixel has a 4-neighbour of its colour, so every
pixel is its own region and **the export is the footprint itself**. Defaults (brush 20,
anti-aliasing off) unless stated.
- **D1**, a tap at (200, 200): an 18 px wide near-disk centred on the pixel corner
  (200, 200), 260 px (rows 6, 10, 14, 14, 16, 16, 18 × 6, 16, 16, 14, 14, 10, 6).
- **D2 / D3**, the same 400 px horizontal drag with 10 coarse moves (40 px apart) and with
  400 moves 1 px apart: **identical** (7460 px). The input's sampling does not matter.
- **The footprint is the stroke as Brushye's engine paints it with anti-aliasing off
  (`brush.stroke … aa=none`, samples at the pointer's canvas positions as integers), shrunk
  by one pixel with the 4-neighbourhood** (a pixel counts when it and its four neighbours
  are painted): 0 pixels different on D1 and D2/D3; on the 60 px drag (D5) and a diagonal
  drag (D4), 12 and 6 pixels differ, all at the stroke's end (CSP's last dab reaches about
  1 px further). Shrinking by the 8-neighbourhood is 20 px off on D1.
- **Run 1 again with this footprint:** with F-002's regions (tolerance 80), all 13 run 1
  captures are exact, including S1 and S6 (brush 20), which filled nothing: their cells
  touch the shrunk footprint's edge.
- **Gap:** the Stroke category's Gap has four presets, the third selected by default. With
  the second (**D6**) the band is the same and only its end moves (by 1 px); CSP's default
  equals Brushye's `gap=normal` (and `narrow`) exactly, the second preset Brushye's `wide`
  within 6 px (at the end).
- (Pointer positions: in run 1 a Real G-Pen click at screen (1043, 663) put its dab's
  centroid at (500.48, 500.58), while this tool's tap at screen (743, 363) is centred on
  (200, 200): open whether the two tools read the pointer differently.)

## Open questions
- The stroke's end (the last dab's position, ~1 px); the pointer convention (above).
- Target color, Tolerance (80 by default), Close gap and Area scaling with this sub tool.
- Brush shapes other than the round tip; anti-aliasing on; opacity; the reference options.
