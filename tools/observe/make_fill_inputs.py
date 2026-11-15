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
