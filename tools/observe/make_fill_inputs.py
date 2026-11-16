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
