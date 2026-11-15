# L-008 Layer masks: values, alpha, opacity
- Status: **implemented** (M10, 2026-11-14; open items below) / **observed** (raster-layer masks: exact; folder masks: see L-004 run 2)
- CSP version: 1.13.2 (PRO, 64-bit, build 202301210208)
- Needs: automated-Wine OK
- Date / method / observer: 2026-11-14 / automated-Wine (Xvfb :77, XTEST mouse drags) / Claude
- Data: `data/observed3/mask*.png` (re-encoded without the sRGB iCCP chunk; pixels unchanged).
  Inputs: `data/inputs/{base,top,top_alpha}.png` (L-001/L-002).

## Setup (exact)
**Run 1 (opaque layer):**
1. File > Open `base.png`, then File > Open `top.png`, Select All, Copy; switch to the base.png
   tab and Edit > Paste. This gives "top Copy" (opaque) over "base", both Normal 100 %.
2. Layer palette "Create layer mask" button (no selection), which makes a reveal-all mask that
   is the paint target.
3. Mask strokes, as horizontal XTEST drags across the full canvas width (screen x 794→1090,
   canvas origin (814, 306), 100 % zoom; 40 steps, 10 ms):
   - Eraser > Hard, size 30, at canvas y 40 and 330.
   - Eraser > Soft, size 60, at y 150 and 420.
   - Pen "B014c" (size 41, AA middle, black) on the mask from canvas x −20 to 128 at y 40.
4. Exports (File > Export (Single Layer) > .png, RGB):
   - the composite: `mask1_normal`, `mask1_multiply`, and `mask1_normal_op50` (layer opacity 50)
   - top Copy alone, base hidden: `mask1_top_alone`, so alpha = mask value M
   - after Layer Mask > Apply Mask to Layer, at opacity 50: `mask1_applied_op50`
   - Also exported but not kept, since they're identical to kept files: the mask disabled
     (= top.png), and applied composite / applied alone / applied Multiply (each identical to
     its masked counterpart).

**Run 2 (semi-transparent layer):**
1. Same base, with `top_alpha.png` pasted as "top_alpha Copy". A new raster layer "Layer 1"
   above it, Edit > Fill with black, which makes it opaque.
2. Reveal-all masks on both layers, then the same eraser strokes from run 3.1 (no pen stroke)
   on each mask. Replayed XTEST drags give identical masks: run 2's M equals run 1's M
   everywhere outside run 1's pen stroke (rows 33–44).
3. Exports:
   - `mask2_readout_alone`: Layer 1 alone, alpha = M
   - `mask2_top_alpha_alone`: top_alpha Copy alone
   - `mask2_top_alpha_normal`: top_alpha Copy over base, Normal 100 %
   - `mask2_top_alpha_normal_op50`: the same at opacity 50

## Observations (all bit-exact, 131 072 pixels each)
- **Masks are continuous** (Layer Property > Mask expression > Show gradients: Yes, the
  default). M takes 211 distinct values in run 1. The soft eraser leaves about 26–30 at its
  centre.
- **Painting on a mask:** the eraser lowers M. The pen with **black** ink (opaque) raises M to
  255. So any opaque colour reveals and the colour is ignored, as the manual says.
- **Mask × pixel alpha:** a masked layer behaves exactly like a layer whose alpha is
  `f = floor(a · M / 255)` (a = pixel alpha). Colour is unchanged. Apply Mask to Layer writes
  exactly f.
- **Composite:** at 100 % opacity, Normal, the result is the usual L-002 lerp with alpha f.
  Multiply with mask = Multiply of the applied layer (bit-exact). Brushye's own model for
  Multiply with partial alpha is still ±2 (L-002/L-003 open, not a mask issue).
- **Opacity with a mask:** opacity first acts on the pixel alpha (L-003:
  `(a · (o8 + 1)) >> 8`, o8 = ⌊op · 255 / 100⌋). Then the mask multiplies with a floor:
  **`eff = floor(((a · (o8 + 1)) >> 8) · M / 255)`**.
  - This is **not** the same as applying the mask and then setting opacity. Applied plus 50 %
    differs from masked plus 50 % on 10 002 pixels (±1).
  - Checked on the opaque layer (a = 255) and on top_alpha (all a).
- **Enable Mask off:** the layer renders as if it had no mask (bit-exact).

- **Export detail:** fully transparent pixels in CSP's exported PNGs carry colour bytes
  (255,255,255) (e.g. `mask1_top_alone`, 11 595 px). It is invisible, and comparisons treat
  alpha-0 pixels as equal. Brushye's PNG export writes (0,0,0,0) there.

## Run 3 (2026-11-14): clipping, folders with opacity, Apply/Ungroup/Mask Selection
Method: automated-Wine (Xvfb :77). Data: `data/observed3/mask3_*.png`.
- **Setup:** base, `top_alpha Copy` (A), `top Copy` (T).
- **Masks:** the standard eraser strokes (run 2). An opaque readout layer made in the same
  session matched run 2's `mask2_readout_alone` exactly, so the strokes are deterministic.
- **Tests:** `crates/brushye-core/tests/observed_masks.rs` (run3).

All results are bit-exact unless noted:
- `mask3_clip_masked_base`: a mask on the clip base (A, T clipped). The group's alpha is the
  base's alpha through the layer rule (floor(a·M/255)).
- `mask3_clip_masked_clipped_screen` / `mask3_clip_screen_mask_disabled`: a mask on the
  clipped layer, which is in Screen, enabled and disabled. The clipped layer's alpha goes
  through the mask before it composites onto the base colour. A disabled mask is ignored.
  - With T in Normal the mask has no visible effect, because top and top_alpha have the same
    colours. That is why Screen was used.
- **Apply Mask with the mask disabled** is still available, and **applies the mask anyway**:
  `mask3_apply_disabled_screen` equals the enabled-mask export.
- **Apply Mask to Layer is greyed out for folders.**
- `mask3_folder_mask_op50`: Normal Folder [A] at 50 % with a mask. The layer rule applies:
  floor(((a·(o8+1))>>8)·M/255) on the folder result's alpha.
- `mask3_through_mask_op50`: the same folder as **Through**. One lerp from the backdrop
  before the folder to the Through result, **w = (w_op · w_M) >> 15**, where
  **w_op = op·32768/100** (16384 at 50 %) and w_M = floor(M·32768/255).
  - Two consecutive lerps, in either order, are wrong.
  - So is a15(o8) = 16320 as the opacity weight. L-004 run 1's opaque data allows anything
    from 16320 to 16384, and this run needs exactly 16384.
- `mask3_ungroup_masked_folder`: Ungroup of a folder with a hiding mask (and opacity 50 %,
  Through) **discards** the folder's mask, opacity and mode. The export equals base +
  top_alpha.
- **Mask Selection with no selection** creates a reveal-all (white) mask, the same as the
  palette button.
- **New layer inside a clipping group:** a raster layer created between a clip base and a
  clipped layer above it is itself **clipped**. One created above the topmost clipped layer
  is not.
- **Show Mask Area (display, from screenshots):** hidden pixels show
  `(C·(255−w) + (0,0,255)·w + 127) / 255`, with `w = round(75·(255−M)/255)` and C the
  displayed pixel. That is 99.95 % exact on pixels whose M is constant over a 7×7
  neighbourhood (77 312 px). Over an empty canvas C is white, giving the (180,180,255) seen in
  U-014.

## Models tried (for the record)
- f: `round(a·M/255)` (22 273 off), `(a·(M+1))>>8` (10 728), a 15-bit product (36 861).
  Only `floor(a·M/255)` is exact.
- Opacity: `floor(f·o8/255)` (27 232 off), `(f·(o8+1))>>8` (2 949), and mask-then-opacity
  orders. Only opacity-then-mask with a floor is exact.

## Run 4 (2026-11-14): Show Mask Area over transparency; PNG export colour
- **Show Mask Area over partly transparent pixels** (base hidden, top_alpha with the standard
  mask, screenshots with the setting off and on): the blue is laid over the **displayed**
  pixel, **checkerboard included**. In the hidden band a white checker square becomes
  (180,180,255) and a grey (204) one (144,144,219).
  - Run 3's "C over white" was a sampling coincidence: its points lay on white checker
    squares.
  - The same formula, `(D·(255−w) + (0,0,255)·w + 127)/255` with D the displayed pixel, is
    99.95 % exact on stable pixels here too.
- **PNG export:** every one of the ~200 CSP exports in `specs/*/data` writes **(255,255,255)**
  under alpha 0. Brushye's PNG export now does the same
  (`brushye_format::png_io::export_png_bytes`).

## Open questions
- Through-folder opacity weight: answered in L-004 run 4, `(o8+1)·128`.
- Mask expression > Show gradients other than Yes (threshold masks).
- Painting a mask with partially transparent ink or lower brush opacity: expected to follow
  the brush's alpha output. Not run.
