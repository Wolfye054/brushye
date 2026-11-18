#!/usr/bin/env python3
"""F-002 run 3: compare Enclose and fill lasso captures with the lasso-shape model.

Each capture (`<name>.png`, alpha = the fill of "All enclosed areas including transparency"
on plain white, i.e. the lasso's inside) has its input events in `events.txt`:
`<name> x,y;x,y;…` (canvas px, the press point first, the release point last).
Model: stabilization S (moving mean over the last S events, the last event repeated S − 1
times at release; Brushye's B-007 rule), then the quadratic B-spline of B-002 (a line to the
first segment midpoint, a quadratic Bézier around every interior event, a line to the last
event), closed, filled where the pixel centre is inside (even-odd).
S is the digits after the name's first letter; mouse captures (S…) are modelled with S = 0
(CSP applied no stabilization to XTEST mouse input).

usage: f002_lasso_model.py DATA_DIR
"""
import re, sys
import numpy as np
from PIL import Image


def inside(poly, w=800, h=800):
    p = np.array(poly, float)
    q = np.roll(p, -1, 0)
    out = np.zeros((h, w), bool)
    for y in range(h):
        cy = y + 0.5
        m = (p[:, 1] <= cy) != (q[:, 1] <= cy)
        if not m.any():
            continue
        a, b = p[m], q[m]
        xs = np.sort(a[:, 0] + (cy - a[:, 1]) * (b[:, 0] - a[:, 0]) / (b[:, 1] - a[:, 1]))
        for x0, x1 in zip(xs[0::2], xs[1::2]):
            i0 = max(int(np.ceil(x0 - 0.5)), 0)
            i1 = min(int(np.ceil(x1 - 0.5)), w)
            out[y, i0:i1] = True
    return out


def stabilize(pts, s):
    if s == 0:
        return pts
    ext = list(pts) + [pts[-1]] * (s - 1)
    out = []
    for i in range(len(ext)):
        win = ext[max(0, i - s + 1):i + 1]
        out.append((sum(p[0] for p in win) / len(win), sum(p[1] for p in win) / len(win)))
    return out


def spline(pts, seg=32):
    q = [np.array(pts[0], float)]
    for p in pts[1:]:
        p = np.array(p, float)
        if np.any(p != q[-1]):
            q.append(p)
    if len(q) < 3:
        return q
    out = [q[0]]
    for i in range(1, len(q) - 1):
        a, b, c = (q[i - 1] + q[i]) / 2, q[i], (q[i] + q[i + 1]) / 2
        for k in range(seg + 1):
            t = k / seg
            out.append((1 - t) ** 2 * a + 2 * t * (1 - t) * b + t * t * c)
    out.append(q[-1])
    return out


def main(d):
    print("capture   S  CSP px  model px  extra  missing")
    for line in open(f"{d}/events.txt"):
        name, ev = line.split()
        s = int(re.match(r"[A-Z](\d+)", name).group(1)) if name.startswith("P") else 0
        pts = [tuple(map(float, e.split(","))) for e in ev.split(";")]
        want = np.array(Image.open(f"{d}/{name}.png"))[..., 3] > 0
        got = inside(spline(stabilize(pts, s)))
        print(f"{name:11} {s:3} {want.sum():7} {got.sum():8} {(got & ~want).sum():6} "
              f"{(want & ~got).sum():8}")


if __name__ == "__main__":
    main(sys.argv[1])
