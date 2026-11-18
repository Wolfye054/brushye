# F-002 Fill tool: Enclose and fill
- Status: **implemented** (2026-11-16: `fill.enclose`, the Enclose and fill sub tool;
  exact on all 29 run 1 captures; 2026-11-18: run 2's rules, exact on 53 of its 56
  captures; the lasso's shape (run 3): run 2's other three within 4 px, run 3's captures
  with S ≤ 20 within 10 px; `crates/brushye-core/tests/observed_enclose.rs`) /
  **observed** (run 1: settings, the region rule, Target color; run 2: Target color and
  Tolerance with F-003's `un2`; run 3: the lasso's shape and Stabilization; open items below)
- CSP version: 1.13.2 (PRO, 64-bit, build 202301210208)
- Date / method / observer: 2026-11-16 / automated-Wine (pen display :78, XTEST mouse) /
  Claude
- Data: `data/enclose/run1/`, `run2/`, `run3/` (exports re-encoded without iCCP). Inputs `en1`, `en2`, `wh` from
  `tools/observe/make_fill_inputs.py` (our own test images, not CSP material).
- Related: F-001 (the flood fill, Close gap, Area scaling and its smoothing).

## Setup (exact)
- File > Open the input (`Z:\…\fillin\<name>.png`), Layer > New Raster Layer on top; Fill
  tool, sub tool **Enclose and fill** (the third); main colour black.
- A lasso is drawn with the mouse (XTEST): press at the first corner, move through the other
  corners of a rectangle in 8 steps per side and back, release. Then the input layer is
  hidden, File > Export (Single Layer) > .png, the layer shown again, and Edit > Undo when the
  export has pixels (an undo after an empty result removes the new layer instead: L5).
  Exported **alpha = the fill**. Canvas origin on screen (543, 163).
- Every setting change checked on a screenshot.

## Observations
### Settings (UI, defaults)
- **Tool property:** Target color (dropdown, "Treat semi-transparent as transparent"), Close
  gap ✓ (two of five steps, ▸), Tolerance 30.0, Area scaling ✓ 10, Refer multiple ✓ (first
  icon lit, ▾), Fill up to vector path ☐, more rows below.
- **Sub Tool Detail**, categories **Closed area fill**, **Reference**, **Ink**,
  **Anti-aliasing**, **Correction**:
  - Closed area fill: Target color; Close gap; Tolerance 30.0; Area scaling ✓ 10 with
    Scaling mode (To darkest pixel selected).
  - Reference: Refer multiple ✓ with its icon row; Exclude (icon row); **Refer to image
    border ✓**; Reference overflow frame ☐; Fill up to vector path ☐ / Include vector path.
  - Ink: Opacity 100; Blending mode Normal.
  - **Anti-aliasing: off.**
  - Correction: Stabilization 6; Adjust by speed ☐ (Stabilization mode: Increase
    stabilization when drawing slowly / Reduce stabilization when drawing quickly); Enable
    snapping ✓.
- **Target color**, in order: Target all colors; Only transparent; Area surrounded by
  transparent; Only black; Area surrounded by black; Only white and transparent; Area
  surrounded by white and transparent; **Treat semi-transparent as transparent** (default);
  All enclosed areas except transparency; All enclosed areas including transparency.
- Sub Tool Detail is toggled by the wrench button; after its dropdown closes, the palette
  may disappear while CSP still counts it as open (the next click on the button closes it).

### The region rule (run 1, `en1`, defaults)
`en1`: white with hard 3 px black outlines (100 px squares, a 40 px square, a 160 px square
with a 60 px square inside, a 100 px square with a 10 px gap in its top side, a gray (128)
filled square, a ring r 50, a row of 30 px squares). Rectangular lassos, defaults:
- **L1**, around square A and half of square B: A filled **entirely, its outline included**
  (10000 px = 100²); B, which the lasso crosses, untouched.
- **L2**, around the nested squares: everything inside the outer square's outer edge (25600
  = 160²: both outlines, the ring between and the inner square).
- **L3**, around the square with the 10 px gap: **only its outline** (1134 px, exactly the
  outline's pixels); the interior leaks through the gap to the white outside.
  **L3b**, the same with Close gap at level 5 (F-001: holds gaps up to 20 px in a normal
  fill): **the same 1134 px**. Close gap does not separate the interior here.
- **L4**, around the gray square and the ring: both whole (10000 + the disk r 50 including
  its line).
- **L5**, around plain white: nothing.
- **L6**, across the row of small squares: exactly the five squares wholly inside (4500 =
  5 × 30²); the sixth, which the lasso crosses, not.
- **L7**, a lasso inside square A's interior, touching nothing: nothing.
- **L8**, around the 40 px square: that square (1600).
- **Rule (all eight):** the image is divided into regions of like colour; every region that
  lies **entirely inside the lasso** is filled, and nothing else. A region reaching past the
  lasso (the white background, the leaking interior, a crossed square) is not filled.

### Target color (run 1, `en2`, one lasso around all shapes)
`en2`: line art on a transparent background: 120 px squares with 3 px outlines, A black,
B black at alpha 128, C red, D black with an opaque white inside, E black with an opaque
gray 40 px square inside; F an anti-aliased black ring r 50. Captured with Area scaling on
(defaults, `T*`) and off (`U*`); without scaling (fractions of each part filled):

| Target color | A line / in | B | C | D line / white | E line / in / blob | F ring / in |
|---|---|---|---|---|---|---|
| Target all colors | 1 / 1 | 1 / 1 | 1 / 1 | 1 / 1 | 1 / 1 / 1 | 0.91 / 1 |
| Only transparent | 0 / 1 | 0 / 1 | 0 / 1 | 0 / 0 | 0 / 1 / 0 | 0.09 / 1 |
| Area surrounded by transparent | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 1 | 1 / 0 / 1 | 0.82 / 0 |
| Only black | 1 / 0 | 0 / 0 | 0 / 0 | 1 / 0 | 1 / 0 / 0 | 0.68 / 0 |
| Area surrounded by black | 0 / 1 | 0 / 0 | 0 / 0 | 0 / 1 | 0 / 1 / 1 | 0.15 / 1 |
| Only white and transparent | 0 / 1 | 0 / 1 | 0 / 1 | 0 / 1 | 0 / 1 / 0 | 0.09 / 1 |
| Area surrounded by white and transparent | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 / 1 | 0.82 / 0 |
| Treat semi-transparent as transparent | as Target all colors |
| All enclosed areas except transparency | as Target all colors |
| All enclosed areas including transparency | as Target all colors |

(F's ring fractions are of its pixels with alpha > 0; its faint anti-aliased fringe goes
with the transparent side, its solid core with the line.)
- "Only X": the enclosed regions of colour X. "Area surrounded by X": the enclosed regions
  that are **not** X (they are surrounded by X), including regions inside them (E's blob is
  filled with E's line). Black at alpha 128 (B) is not black; red (C) is neither.
- The four "all" options are identical on this image. Black at alpha 128 is not treated as
  transparent by the default (B fills whole).
- **With Area scaling** (To darkest pixel 10, the default), a selected region then grows by
  F-001's rule: transparent interiors grow 2 px into their 3 px black lines (t − 1), and
  E's interior 10 px into its gray square (0.75 of it). Everything else is unchanged.

## Run 2 (2026-11-17, pen display :78): Target color and Tolerance (as F-003 run 3)
Data in `data/enclose/run2/` (re-encoded without iCCP). F-003 run 3's inputs and names
(`un2` rows A / B, plain white `W`, plain transparent `T`, the white/transparent boundary
`X`, `en2` rows `R1` / `R2`), with rectangular lassos (canvas) A (20, 80)–(795, 185),
B (20, 480)–(795, 585), W (100, 200)–(700, 300), T (100, 650)–(700, 750), X (350, 350)–
(450, 450), R1 (40, 40)–(740, 200), R2 (40, 240)–(420, 400). **Area scaling off**;
`_t<t>`: default target at tolerance t (30, the default, 100, 80, 60, 40, 20, 5, 0);
`_g<i>`: Target color item i at tolerance 80. Every setting checked on a screenshot; the
defaults (Treat semi-transparent as transparent, 30, Area scaling 10) restored after.
- **Every capture equals F-003 run 3's** (the same name there), pixel for pixel, except
  "All enclosed areas including transparency" on opaque areas: Enclose and fill's rules
  are Paint unfilled area's, with the lasso's inside in place of the stroke's footprint.
  - Default target: on row A every square fills at every tolerance (opaque colours never
    merge); on row B a bridge leaks when its alpha ≤ T (t 30, T 77: 60 leaks, 100 holds;
    t 100: all leak, the outlines stay opaque).
  - Target all colors and the six class options: as F-003 run 3 (and F-002 run 1).
  - All enclosed areas except transparency: two classes; nothing on row A.
  - **All enclosed areas including transparency**: the same plus **every non-transparent
    pixel inside the lasso** (row A 81218 px, W 59886, X 4994: the white half). Compared
    with the nominal rectangles, the model has 157 / 114 / 6 px more, all at the lasso's
    corners (rounded by the lasso's Stabilization 6).
- With these rules (F-003 run 3's) F-002 run 1's 29 captures stay exact.

## Run 3 (2026-11-18, pen display :78): the lasso's shape and Stabilization
Data in `data/enclose/run3/` (re-encoded without iCCP) with every capture's input events in
`events.txt` (`<name> x,y;…`, canvas px: the press point, then every move; the last is the
release point). Input `wh` (plain opaque white); Target color **All enclosed areas
including transparency**, Area scaling off, so the fill is exactly the lasso's inside.
Analysis: `tools/observe/f002_lasso_model.py data/enclose/run3`.
- Lassos: the rectangle (100, 100)–(400, 300), pressed at (100, 100), moved clockwise through
  the corners in `n` evenly spaced events per side (integer positions) and back to the start,
  then released; `tri`: the triangle (100, 400), (400, 420), (220, 750) with 8 events per side.
  Mouse (XTEST, `S<stab>_…`: 30 ms between events, `slow` 150 ms, n32 10 ms) and pen
  (`vpen.py`, pressure 0.5, `P<stab>_…`: 30 ms for n8 and tri, 10 ms for n64, `w30` 30 ms).
- Stabilization is set in Sub Tool Detail > Correction (it is not in the Tool property).
  Values checked on screenshots: 0, 100 for the mouse; 0, 6, 20, 50, 100 for the pen.
  (Mouse captures at 6 and 20 were also made, identical to 0, but their setting was not
  screenshot-checked, so they are not kept.)

Results (CSP px = filled pixels; model below):

| capture | S | CSP px | model px | extra | missing |
|---|---|---|---|---|---|
| S0_n1 / n2 / n8 / n32 | 0 | 52491 / 58116 / 59882 / 59992 | 52490 / 58119 / 59886 / 59993 | 1 / 3 / 4 / 1 | 2 / 0 / 0 / 0 |
| S0_n8slow / S0_tri | 0 | 59882 / 51283 | 59886 / 51291 | 4 / 8 | 0 |
| S100_n1 / n8 / n8slow / n32 (mouse) | 100 | **identical to S0's**, pixel for pixel | | | |
| P0_n8 / n64 / tri | 0 | 59882 / 59999 / 51285 | 59886 / 59999 / 51291 | 4 / 0 / 8 | 0 / 0 / 2 |
| P6_n8 / n64 / n64w30 / tri | 6 | 55777 / 59936 / 59936 / 46578 | 55782 / 59936 / 59936 / 46585 | 5 / 0 / 0 / 7 | 0 |
| P20_n64 | 20 | 59273 | 59273 | 0 | 0 |
| P50_n64 | 50 | 56141 | 55430 | 0 | 711 |
| P100_n64 | 100 | 50381 | 42706 | 0 | 7675 |
| P100_n8 (twice) | 100 | **0** (nothing filled) | 13860 | | |

1. **The lasso is a smoothed curve through the input events, even at Stabilization 0**: the
   quadratic B-spline of brush strokes (B-002 run 2) — a straight piece from the press point
   to the first segment midpoint, a quadratic Bézier (mᵢ, Pᵢ, mᵢ₊₁) around every interior
   event, a straight piece to the release point — closed back to the press point. A pixel is
   inside when its centre is (event positions taken as they are, no half-pixel offset; the
   model with +0.5 is worse). So corners are cut by half the neighbouring segments: the press
   corner stays sharp, and fewer events cut more (n1 touches each side only at its middle).
   Within 1–8 px on every capture at S = 0.
2. Timing has no effect: 30 vs 150 ms (mouse) and 10 vs 30 ms (pen) give identical exports.
3. **Stabilization acts on pen input only** (under Wine): the mouse at 100 is identical to 0.
   With the pen, the lasso line lags behind the pen while drawing (seen on screen at 100).
4. **With the pen, Stabilization S is Brushye's brush rule** (B-007): each event becomes the
   mean of the last S events, then the spline of 1. Exact at S = 6 and S = 20 on the n64
   captures; S ± 1 is worse on every capture (e.g. P6_n8: S 5 +1290 px, S 7 −1512 px). As for
   brushes, larger settings are off (S 50: 711 px short; S 100: 7675 px short), and at S 100
   a lasso of only 32 events fills nothing (the model still has 13860 px).
- Flag `verify-native`: whether native Windows also leaves mouse input unstabilized.

## Implementation notes (Brushye)
- `brushye_core::fill::enclose` and the command `fill.enclose LAYER points=[x,y;…]
  color=#rrggbb [stabilization= target= tolerance= aa= scale= scale_mode= opacity= refer=]`;
  the command's defaults are the sub tool's (Treat semi-transparent as transparent, tolerance
  30, no anti-aliasing, To darkest pixel 10, the visible image) except stabilization (0).
  `points` are the input events; the app draws a lasso with the Enclose and fill sub tool
  and sends one `fill.enclose` on release, with the sub tool's Stabilization (preset 6) only
  when the pen drew it. Its Tool property has the Target color row; the preview shows the
  lasso as it will fill (`fill::lasso_live`: the pen's line lags, as in CSP).
- The lasso (run 3): `fill::lasso(events, S)` = `brush::path::stabilize` (B-007's moving
  mean, with the catch-up at release) then `brush::path::spline` (B-002), flattened; the
  polygon closes back to the press point. S ≥ 50 inherits B-007's open rule.
- Journals written before 2026-11-18 replay with the lasso's B-spline instead of the polygon
  through the points (a fidelity fix, D-021).
- Regions are 4-connected. "Inside the lasso" is a pixel's centre inside the polygon
  (even-odd). Area scaling and its smoothing are F-001's.
- Target color and tolerance are F-003 run 3's rules (shared code with Paint unfilled
  area, `brushye_core::fill`), with the lasso's inside as the painted area. With
  T = round(2.55·t): transparent = alpha ≤ T and < 255; black = alpha ≥ 255 − T with
  R, G, B ≤ T; white = alpha ≥ 255 − T with R, G, B ≥ 255 − T. Target all colors: a pixel
  joins a region when every channel is within T of the region's first pixel. The default:
  transparent pixels together, others only with exactly equal RGBA. Except / including
  transparency: two classes (transparent / not); "including" adds every non-transparent
  pixel inside the lasso.
- Chosen, not observed: Close gap has no effect; anti-aliasing without area scaling gives
  the binary area.

## Open questions
- Close gap: no effect in L3b; whether it ever acts in Enclose and fill.
- Refer to image border; Reference overflow frame; Refer multiple / Exclude; Fill up to
  vector path.
- Anti-aliasing on; Opacity and Blending mode.
- Stabilization ≥ 50 with the pen (as for brushes, B-007), and why a short lasso at 100
  fills nothing; "Adjust by speed".
- The lasso's own edge: a region that the lasso line touches but does not cross; regions
  partly on the lasso line.
- Other ways to draw the area (polyline / rectangle) if any.
