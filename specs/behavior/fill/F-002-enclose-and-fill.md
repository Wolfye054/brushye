# F-002 Fill tool: Enclose and fill
- Status: **implemented** (2026-11-16: `fill.enclose`, the Enclose and fill sub tool;
  exact on all 29 run 1 captures, `crates/brushye-core/tests/observed_enclose.rs`) /
  **observed** (run 1: settings, the region rule, Target color; open items below)
- CSP version: 1.13.2 (PRO, 64-bit, build 202301210208)
- Date / method / observer: 2026-11-16 / automated-Wine (pen display :78, XTEST mouse) /
  Claude
- Data: `data/enclose/run1/` (exports re-encoded without iCCP). Inputs `en1`, `en2` from
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

## Implementation notes (Brushye)
- `brushye_core::fill::enclose` and the command `fill.enclose LAYER points=[x,y;…]
  color=#rrggbb [target= tolerance= aa= scale= scale_mode= opacity= refer=]`; the command's
  defaults are the sub tool's (Treat semi-transparent as transparent, tolerance 30, no
  anti-aliasing, To darkest pixel 10, the visible image). The app draws a lasso with the
  Enclose and fill sub tool and sends one `fill.enclose` on release; its Tool property has
  the Target color row.
- Regions are 4-connected. "Inside the lasso" is a pixel's centre inside the polygon
  (even-odd). Area scaling and its smoothing are F-001's.
- Chosen, not observed (exact on run 1 regardless): with tolerance T = round(2.55·t),
  transparent = alpha ≤ T, black = alpha ≥ 255 − T with R, G, B ≤ T, white = alpha ≥
  255 − T with R, G, B ≥ 255 − T; for the four "all" options a pixel joins a region when
  every channel is within T of the region's first pixel; the four "all" options behave
  alike; Close gap has no effect; anti-aliasing without area scaling gives the binary
  area.

## Open questions
- What separates the four "all" options and the default ("semi-transparent" threshold;
  transparent regions); tolerance's role in forming regions (30 by default).
- Close gap: no effect in L3b; whether it ever acts in Enclose and fill.
- Refer to image border; Reference overflow frame; Refer multiple / Exclude; Fill up to
  vector path.
- Anti-aliasing on; Opacity and Blending mode; Stabilization's effect on the lasso.
- The lasso's own edge: a region that the lasso line touches but does not cross; regions
  partly on the lasso line.
- Other ways to draw the area (polyline / rectangle) if any.
