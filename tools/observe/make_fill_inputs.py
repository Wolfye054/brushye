"""Inputs for the fill observation pass (spec F-001): 800×800 images whose value depends only
on x, so a fill's horizontal extent gives the tolerance threshold directly.
v(x) = clamp((x − 16) // 3, 0, 255): 0 at x ≤ 18, 255 from x = 781.
usage: make_fill_inputs.py OUT_DIR"""
import sys, os
import numpy as np
from PIL import Image

out = sys.argv[1]
os.makedirs(out, exist_ok=True)
x = np.arange(800)
v = np.clip((x - 16) // 3, 0, 255).astype(np.uint8)
def save(name, r, g, b, a):
    img = np.zeros((800, 800, 4), np.uint8)
    for i, c in enumerate((r, g, b, a)):
        img[..., i] = c[None, :] if np.ndim(c) else c
    Image.fromarray(img, "RGBA").save(os.path.join(out, name + ".png"))
z = np.zeros(800, np.uint8)
save("ramp_gray", v, v, v, 255)
save("ramp_red", v, z, z, 255)
save("ramp_redgreen", v, 255 - v, z, 255)
save("ramp_alpha", z, z, z, v)

# Shapes (opaque, white background): an anti-aliased black ring (4× supersampled), and eight
# square outlines (3 px, hard) with one gap in their top side of 1, 2, 3, 4, 6, 8, 12, 16 px.
ss = 4
yy, xx = (np.mgrid[0:800 * ss, 0:800 * ss] + 0.5) / ss
d = np.hypot(xx - 400, yy - 400)
ring = ((d >= 200) & (d <= 203)).reshape(800, ss, 800, ss).mean(axis=(1, 3))
disk = (d <= 60).reshape(800, ss, 800, ss).mean(axis=(1, 3))
k = np.clip(np.maximum(ring, disk), 0, 1)
g = np.round(255 * (1 - k)).astype(np.uint8)
save("ring", g, g, g, 255)
def gap_image(name, gaps):
  img = np.full((800, 800), 255, np.uint8)
  for i, gap in enumerate(gaps):
    x0, y0 = 40 + (i % 4) * 190, 200 + (i // 4) * 260
    s = 150
    img[y0:y0 + 3, x0:x0 + s] = 0
    img[y0 + s - 3:y0 + s, x0:x0 + s] = 0
    img[y0:y0 + s, x0:x0 + 3] = 0
    img[y0:y0 + s, x0 + s - 3:x0 + s] = 0
    c = x0 + s // 2 - gap // 2
    img[y0:y0 + 3, c:c + gap] = 255
  save(name, img, img, img, 255)
gap_image("gaps", [1, 2, 3, 4, 6, 8, 12, 16])
gap_image("gaps2", [5, 7, 9, 10, 11, 20, 24, 32])

# Area scaling (F-001 run 3): white with hard black vertical bars 1, 2, 3, 4, 6, 9, 15 px wide
# (x from 100, 40 px apart, y 100–400), a 9 px bar with a valley profile
# 200 150 100 50 0 50 100 150 200 at x = 480, a black 60×60 square at (600, 150) and a black
# hard disk (r 50, centre-in test) at (650, 550).
img = np.full((800, 800), 255, np.uint8)
x = 100
for wdt in [1, 2, 3, 4, 6, 9, 15]:
    img[100:400, x:x + wdt] = 0
    x += 40 + wdt
for i, v in enumerate([200, 150, 100, 50, 0, 50, 100, 150, 200]):
    img[100:400, 480 + i] = v
img[150:210, 600:660] = 0
yy, xx = np.mgrid[0:800, 0:800]
img[(xx + 0.5 - 650) ** 2 + (yy + 0.5 - 550) ** 2 <= 50 ** 2] = 0
save("scale", img, img, img, 255)

# One-sided lines (F-001 run 3): square outlines (150 px) of thickness 1, 2, 3, 4, 6, 9, 15 and
# a 9 px outline with the valley profile 200 … 0 … 200 across its thickness; clicked outside,
# the fill grows into each outline from one side only.
img = np.full((800, 800), 255, np.uint8)
specs = [[0] * t for t in [1, 2, 3, 4, 6, 9, 15]] + [[200, 150, 100, 50, 0, 50, 100, 150, 200]]
for i, prof in enumerate(specs):
    x0, y0, s = 40 + (i % 4) * 190, 200 + (i // 4) * 260, 150
    for k, v in enumerate(prof):
        img[y0 + k, x0 + k:x0 + s - k] = v
        img[y0 + s - 1 - k, x0 + k:x0 + s - k] = v
        img[y0 + k:y0 + s - k, x0 + k] = v
        img[y0 + k:y0 + s - k, x0 + s - 1 - k] = v
save("outlines", img, img, img, 255)

# To darkest pixel (F-001 run 4): square outlines as in `outlines`, each with its own profile
# across the thickness (outside → inside) and an interior value (default white). `dk1`, `dk2`.
def profile_image(name, specs):
    img = np.full((800, 800), 255, np.uint8)
    for i, (prof, inner) in enumerate(specs):
        x0, y0, s = 40 + (i % 4) * 190, 200 + (i // 4) * 260, 150
        img[y0:y0 + s, x0:x0 + s] = inner
        for k, v in enumerate(prof):
            img[y0 + k, x0 + k:x0 + s - k] = v
            img[y0 + s - 1 - k, x0 + k:x0 + s - k] = v
            img[y0 + k:y0 + s - k, x0 + k] = v
            img[y0 + k:y0 + s - k, x0 + s - 1 - k] = v
    save(name, img, img, img, 255)
profile_image("dk1", [([100, 50, 80, 0], 255), ([100, 50, 50, 80, 0], 255), ([0, 0, 128], 255),
                      ([128, 0], 255), ([100, 100, 100, 100], 255), ([0, 0, 0, 50, 0, 0, 0], 255),
                      ([0, 128, 0], 255), ([100, 100, 0], 255)])
profile_image("dk2", [([0, 30, 30, 30], 255), ([0, 50, 100, 150, 200], 255),
                      ([220, 200, 180, 160, 140, 120, 100, 80, 60, 40, 20, 0], 255),
                      ([50, 0, 50, 0, 50], 255), ([0, 1], 255), ([1, 0], 255),
                      ([100, 0, 0, 100], 255), ([100, 100, 100], 0)])

# 2-D (`dk3`): a 1 px 8-connected diagonal black line, a 2 px one, a black ring 3 px thick
# (r 60–63), a diagonal valley band (value by distance to the diagonal), a radial valley ring,
# and a black plus sign (bars 5 px).
img = np.full((800, 800), 255, np.uint8)
yy, xx = np.mgrid[0:800, 0:800]
for i in range(200):
    img[100 + i, 100 + i] = 0
    img[100 + i, 200 + i:202 + i] = 0
r = np.hypot(xx + 0.5 - 600, yy + 0.5 - 200)
img[(r >= 60) & (r < 63)] = 0
valley = np.array([200, 150, 100, 50, 0, 50, 100, 150, 200])
dd = (xx - yy) - 300
band = (np.abs(dd) <= 4) & (yy >= 400) & (yy < 700) & (xx < 780)
img[band] = valley[dd[band] + 4]
ring = (np.abs(np.hypot(xx + 0.5 - 600, yy + 0.5 - 560) - 70) < 4.5)
ri = np.clip(np.floor(np.hypot(xx + 0.5 - 600, yy + 0.5 - 560) - 70 + 4.5).astype(int), 0, 8)
img[ring] = valley[ri[ring]]
img[640:645, 80:200] = 0
img[580:705, 137:142] = 0
save("dk3", img, img, img, 255)

# Darkness measure (F-001 run 4): two-pixel profiles [A, B] (outside → inside, white interior).
# The fill takes both only if B is darker than A, so each outline shows the order of a pair
# under CSP's measure: `dk4` colour pairs (luma 601 / 709, mean, max disagree), `dk5` alpha
# pairs on a transparent background (RGBA 0,0,0,0) with a transparent interior.
def rgba_profile_image(name, specs, bg):
    img = np.zeros((800, 800, 4), np.uint8)
    img[...] = bg
    for i, (prof, inner) in enumerate(specs):
        x0, y0, s = 40 + (i % 4) * 190, 200 + (i // 4) * 260, 150
        img[y0:y0 + s, x0:x0 + s] = inner
        for k, v in enumerate(prof):
            img[y0 + k, x0 + k:x0 + s - k] = v
            img[y0 + s - 1 - k, x0 + k:x0 + s - k] = v
            img[y0 + k:y0 + s - k, x0 + k] = v
            img[y0 + k:y0 + s - k, x0 + s - 1 - k] = v
    Image.fromarray(img, "RGBA").save(os.path.join(out, name + ".png"))
W4 = (255, 255, 255, 255)
R, G, B = (255, 0, 0, 255), (0, 255, 0, 255), (0, 0, 255, 255)
gray = lambda g: (g, g, g, 255)
rgba_profile_image("dk4", [([R, B], W4), ([B, R], W4), ([G, R], W4), ([gray(80), R], W4),
                           ([gray(30), B], W4), ([gray(170), G], W4), ([gray(100), B], W4),
                           ([gray(110), (255, 0, 255, 255)], W4)], W4)
k = lambda a: (0, 0, 0, a)
T0 = (0, 0, 0, 0)
rgba_profile_image("dk5", [([k(255), k(255)], T0), ([k(128), k(255)], T0),
                           ([k(255), k(128)], T0), ([W4, k(255)], T0), ([k(128), k(128)], T0),
                           ([gray(128), k(128)], T0), ([k(200), k(255)], T0), ([k(255)], T0)], T0)
# `dk6`: the alpha rounding around 50 % black (transparent background and interior):
# [gray g, black α128] for g = 126, 127, 129, 130 and [black α128, gray g] for g = 126..129.
rgba_profile_image("dk6", [([gray(g), k(128)], T0) for g in [126, 127, 129, 130]]
                   + [([k(128), gray(g)], T0) for g in [126, 127, 128, 129]], T0)

# Anti-aliased smoothing of scaled areas (F-001 run 5), `sm1`: black shapes on white, the fill
# clicked in the white and grown into them. Row 1–2: right triangles whose hypotenuse has slope
# 1, 1/2, 1/3, 1/4, 1/5, 1/8, 2/3, 3/4, 2/5, 0.37 (cells 150 px). Row 3: 140×60 rectangles with,
# on their top side, a single step of 1, 2, 3 px, a 1 px notch 1, 3, 6 px wide, a 2 px bump 3
# and 6 px wide. Row 4: squares 1–5 px and disks r 3, 5, 8, 12.
img = np.full((800, 800), 255, np.uint8)
yy, xx = np.mgrid[0:800, 0:800]
slopes = [(1, 1), (1, 2), (1, 3), (1, 4), (1, 5), (1, 8), (2, 3), (3, 4), (2, 5), (37, 100)]
for i, (a, b) in enumerate(slopes):
    x0, y0 = 30 + (i % 5) * 150, 30 + (i // 5) * 160
    w = 130
    h = w * a // b
    for x in range(w):
        top = y0 + 140 - (x * a) // b - 1   # hypotenuse rises to the right
        img[top:y0 + 140, x0 + x] = 0
cells = [("step", 1), ("step", 2), ("step", 3), ("notch", 1), ("notch", 3), ("notch", 6),
         ("bump", 3), ("bump", 6)]
for i, (kind, s) in enumerate(cells):
    x0, y0 = 20 + (i % 4) * 190, 380 + (i // 4) * 110
    img[y0:y0 + 60, x0:x0 + 140] = 0
    if kind == "step":
        img[y0:y0 + s, x0 + 70:x0 + 140] = 255
    elif kind == "notch":
        img[y0:y0 + 1, x0 + 70 - s // 2:x0 + 70 - s // 2 + s] = 255
    else:
        img[y0 - 2:y0, x0 + 70 - s // 2:x0 + 70 - s // 2 + s] = 0
for i, s in enumerate([1, 2, 3, 4, 5]):
    img[640:640 + s, 40 + i * 40:40 + i * 40 + s] = 0
for i, r in enumerate([3, 5, 8, 12]):
    cx, cy = 300 + i * 120, 680
    img[(xx + 0.5 - cx) ** 2 + (yy + 0.5 - cy) ** 2 <= r * r] = 0
save("sm1", img, img, img, 255)

# Long-run rule (F-001 run 6), `sm2`: black rectangles on white with one feature on one side,
# 28 cells of 190×100 (4 per row, from (10, 10)). The feature is drawn on the top side of a
# W×50 rectangle (by default W = 150), as a height offset per column (1 = the black starts one
# row lower), then the cell is mirrored or rotated for the side cells.
def step_at(w, p):          # left part high, right part one row lower, from column p
    return [0] * p + [1] * (w - p)
def two_steps(w, d):        # two 1 px steps down, d apart, centred
    a = (w - d) // 2
    return [0] * a + [1] * d + [2] * (w - a - d)
def notch(w, n):            # a 1 px deep dent n wide, centred
    a = (w - n) // 2
    return [0] * a + [1] * n + [0] * (w - a - n)
def bump(w, n):             # a 1 px high bump n wide, centred
    a = (w - n) // 2
    return [1] * a + [0] * n + [1] * (w - a - n)
SM2 = ([("step", 150, step_at(150, p), "") for p in [10, 20, 40, 75, 110, 130]]
       + [("stepup", 150, step_at(150, p)[::-1], "") for p in [20, 75, 130]]
       + [("stepw", w, step_at(w, w // 2), "") for w in [20, 40, 80, 170]]
       + [("two", 150, two_steps(150, d), "") for d in [5, 10, 30, 60]]
       + [("notch", 150, notch(150, n), "") for n in [10, 20, 40]]
       + [("bump", 150, bump(150, n), "") for n in [10, 20, 40]]
       + [("side", 90, step_at(90, 45), rot) for rot in ["left", "right", "bottom"]]
       + [("plain", 150, [0] * 150, "")])
def sm2_cell(w, prof, rot):
    c = np.full((100, 190), 255, np.uint8)
    h = 50 if rot == "" else 80
    shape = np.full((h + 4, w), 255, np.uint8)
    for x, o in enumerate(prof):
        shape[2 + o:2 + h, x] = 0
    if rot == "left":
        shape = np.rot90(shape, 1)
    elif rot == "right":
        shape = np.rot90(shape, -1)
    elif rot == "bottom":
        shape = shape[::-1]
    sh, sw = shape.shape
    y, x = (100 - sh) // 2, (190 - sw) // 2
    c[y:y + sh, x:x + sw] = np.minimum(c[y:y + sh, x:x + sw], shape)
    return c
img = np.full((800, 800), 255, np.uint8)
for i, (_, w, prof, rot) in enumerate(SM2):
    cx, cy = 10 + (i % 4) * 195, 10 + (i // 4) * 110
    img[cy:cy + 100, cx:cx + 190] = sm2_cell(w, prof, rot)
save("sm2", img, img, img, 255)

# Tall steps (F-001 run 7), `sm3`: 28 cells as in `sm2` (black rectangles, one feature on the top
# side, given as a per-column offset of the top edge). Steps of 1–6 px at the centre; 2 px
# steps at columns 20, 40, 110, 130; 3 px steps at 20, 130; a 2 px step up (mirrored); two 2 px
# steps 30 apart; a 1 px then a 2 px step; notches 2 px deep 10, 20 wide and 3 px deep 20 wide;
# bumps 2 px high 10, 20 wide and 3 px high 20 wide; then small black squares 1–6 px and disks
# r 2, 3, 4, 6 (cells 26–27).
def tall(w, p, t):
    return [0] * p + [t] * (w - p)
def dent(w, n, t, up=False):
    a = (w - n) // 2
    out = [0] * a + [t] * n + [0] * (w - a - n)
    return [t - o for o in out] if up else out
SM3 = ([tall(150, 75, t) for t in [1, 2, 3, 4, 5, 6]]
       + [tall(150, p, 2) for p in [20, 40, 110, 130]]
       + [tall(150, p, 3) for p in [20, 130]]
       + [tall(150, 75, 2)[::-1]]
       + [[0] * 60 + [2] * 30 + [4] * 60]
       + [[0] * 50 + [1] * 50 + [3] * 50]
       + [dent(150, n, t) for n, t in [(10, 2), (20, 2), (20, 3)]]
       + [dent(150, n, t, up=True) for n, t in [(10, 2), (20, 2), (20, 3)]])
def sm3_cell(prof):
    c = np.full((100, 190), 255, np.uint8)
    w = len(prof)
    x0, y0 = (190 - w) // 2, 25
    for x, o in enumerate(prof):
        c[y0 + o:y0 + 55, x0 + x] = 0
    return c
img = np.full((800, 800), 255, np.uint8)
for i, prof in enumerate(SM3):
    cx, cy = 10 + (i % 4) * 195, 10 + (i // 4) * 110
    img[cy:cy + 100, cx:cx + 190] = sm3_cell(prof)
# Small shapes in the last row (cells 26, 27 area): squares 1–6 px, disks r 2, 3, 4, 6.
yy, xx = np.mgrid[0:800, 0:800]
for i, s in enumerate([1, 2, 3, 4, 5, 6]):
    x, y = 420 + i * 30, 700
    img[y:y + s, x:x + s] = 0
for i, r in enumerate([2, 3, 4, 6]):
    cx, cy = 440 + i * 80, 755
    img[(xx + 0.5 - cx) ** 2 + (yy + 0.5 - cy) ** 2 <= r * r] = 0
save("sm3", img, img, img, 255)

# Enclose and fill (F-002 run 1), `en1`: white with hard 3 px black outlines: squares 100 px at
# (60, 60), (220, 60), (380, 60); a 40 px square at (560, 80); a square 160 px with a 60 px
# square inside it at (60, 260); a 100 px square with a 10 px gap in its top side at (300, 280);
# a gray (128) filled 100 px square at (480, 280); a ring r 50 at (690, 330); and a row of
# 30 px squares 20 px apart along y = 520 from x = 60.
def outline(img, x, y, s, v=0, t=3):
    img[y:y + t, x:x + s] = v
    img[y + s - t:y + s, x:x + s] = v
    img[y:y + s, x:x + t] = v
    img[y:y + s, x + s - t:x + s] = v
img = np.full((800, 800), 255, np.uint8)
for x in (60, 220, 380):
    outline(img, x, 60, 100)
outline(img, 560, 80, 40)
outline(img, 60, 260, 160)
outline(img, 110, 310, 60)
outline(img, 300, 280, 100)
img[280:283, 345:355] = 255
img[280:380, 480:580] = 128
yy, xx = np.mgrid[0:800, 0:800]
rr = np.hypot(xx + 0.5 - 690, yy + 0.5 - 330)
img[(rr >= 47) & (rr <= 50)] = 0
for i in range(12):
    outline(img, 60 + i * 50, 520, 30)
save("en1", img, img, img, 255)

# Enclose and fill, Target color (F-002 run 1), `en2`: line art on a transparent background
# (RGBA 0,0,0,0): 120 px squares with 3 px outlines at y = 60: A black (x 60), B black at
# alpha 128 (x 240), C red (x 420), D black with an opaque white inside (x 600); at y = 260:
# E black with an opaque gray (128) 40 px square in the middle (x 60), F an anti-aliased black
# ring r 50, 3 px (4x supersampled) centred at (360, 320).
img = np.zeros((800, 800, 4), np.uint8)
def outline4(x, y, s, c, t=3):
    img[y:y + t, x:x + s] = c
    img[y + s - t:y + s, x:x + s] = c
    img[y:y + s, x:x + t] = c
    img[y:y + s, x + s - t:x + s] = c
outline4(60, 60, 120, (0, 0, 0, 255))
outline4(240, 60, 120, (0, 0, 0, 128))
outline4(420, 60, 120, (255, 0, 0, 255))
img[63:177, 603:717] = (255, 255, 255, 255)
outline4(600, 60, 120, (0, 0, 0, 255))
outline4(60, 260, 120, (0, 0, 0, 255))
img[300:340, 100:140] = (128, 128, 128, 255)
ss = 4
yy, xx = (np.mgrid[0:800 * ss, 0:800 * ss] + 0.5) / ss
d = np.hypot(xx - 360, yy - 320)
cov = ((d >= 47) & (d <= 50)).reshape(800, ss, 800, ss).mean(axis=(1, 3))
a = np.round(cov * 255).astype(np.uint8)
m = a > 0
img[m, 3] = a[m]
Image.fromarray(img, "RGBA").save(os.path.join(out, "en2.png"))

# Paint unfilled area (F-003 run 1), `un1`: white with hard black lines: a grid of 10×10 cells,
# 20 px pitch with 2 px lines, at (60, 60); a 200 px square outline (3 px) at (400, 60); a row
# of tiny white holes (1, 2, 3, 4, 6, 8 px) in a black bar at y 400–420 from x 60; a
# 200 px square outline at (400, 400) holding a 100 px gray (128) square in its middle.
img = np.full((800, 800), 255, np.uint8)
for k in range(11):
    img[60 + 20 * k:62 + 20 * k, 60:262] = 0
    img[60:262, 60 + 20 * k:62 + 20 * k] = 0
outline(img, 400, 60, 200)
img[400:420, 60:330] = 0
x = 70
for s in [1, 2, 3, 4, 6, 8]:
    img[410 - s // 2:410 - s // 2 + s, x:x + s] = 255
    x += 40
outline(img, 400, 400, 200)
img[450:550, 450:550] = 128
save("un1", img, img, img, 255)
