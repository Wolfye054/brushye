//! The anti-aliased smoothing CSP applies to a scaled fill / auto-select area (spec F-001,
//! runs 5–6): the outline of the binary area is redrawn as a polygon and rasterised with exact
//! area coverage.
//!
//! The polygon, as observed:
//! - the outline is cut into straight runs (horizontal and vertical stretches of pixel edges);
//!   every run contributes its **midpoint**, so staircases become straight lines through the
//!   midpoints of their runs (exact on every slope tested, run 5);
//! - a **corner between two runs of length ≥ 2 is kept** (rectangles stay sharp), and so are
//!   both corners of a short run that turns the outline around (its neighbours run in opposite
//!   directions: the end of a 1 px line, run 3; the tips of run 5), except that
//!   a run reached from a step continues the line from the step through its midpoint to the
//!   corner when that moves the boundary into the shape, i.e. the area grows (it then meets
//!   the corner ½ px past the hard edge, run 6);
//! - a segment between a step (a run shorter than 2) and a long run that is not continued to a
//!   corner is **translated ¼ px along the long run, away from the step** (monotone
//!   staircases excepted, run 6).
//!
//! Not modelled (open in F-001): 2–3 px steps (CSP treats them like 1 px steps), and CSP's ±1 arithmetic differences.
//! Chosen, not observed: beyond the canvas edge the area is extended by repeating the edge
//! pixels, so the canvas border itself is never smoothed.

/// Pixels of edge-replicating padding around the canvas while tracing.
const PAD: usize = 4;

type P = [f64; 2];

/// Smoothed coverage (0–255) of `area` (`w`×`h`, row-major).
pub fn smooth_area(area: &[bool], w: u32, h: u32) -> Vec<u8> {
    let (w, h) = (w as usize, h as usize);
    let (pw, ph) = (w + 2 * PAD, h + 2 * PAD);
    // The traced shape is the complement of the area (CSP's numbers are the area's coverage,
    // 1 − the shape's), padded by repeating the edge.
    let mask = |x: usize, y: usize| -> bool {
        let cx = x.clamp(PAD, PAD + w - 1) - PAD;
        let cy = y.clamp(PAD, PAD + h - 1) - PAD;
        !area[cy * w + cx]
    };
    let mut rows: Vec<Vec<(usize, f64)>> = vec![Vec::new(); ph];
    for lp in contours(&mask, pw, ph) {
        let poly = polygon(&lp);
        let k = poly.len();
        for i in 0..k {
            accumulate(&mut rows, poly[i], poly[(i + 1) % k], pw);
        }
    }
    let mut out = vec![0u8; w * h];
    for (py, row) in rows.iter_mut().enumerate() {
        if py < PAD || py >= PAD + h {
            continue;
        }
        let y = py - PAD;
        row.sort_unstable_by_key(|e| e.0);
        let mut it = row.iter().peekable();
        let mut run = 0.0;
        for px in 0..PAD + w {
            while let Some(&&(x, d)) = it.peek() {
                if x > px {
                    break;
                }
                run += d;
                it.next();
            }
            if px >= PAD {
                let c = run.abs().clamp(0.0, 1.0);
                out[y * w + px - PAD] = ((1.0 - c) * 255.0 + 0.5).floor() as u8;
            }
        }
    }
    out
}

/// Closed outlines of `mask` as vertex loops on the pixel-corner grid, walked with the mask on
/// the right (y down). Where two pieces touch at a corner the walk turns right.
fn contours(mask: &dyn Fn(usize, usize) -> bool, w: usize, h: usize) -> Vec<Vec<[i64; 2]>> {
    use std::collections::HashMap;
    let m = |x: i64, y: i64| {
        x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h && mask(x as usize, y as usize)
    };
    let mut next: HashMap<[i64; 2], Vec<[i64; 2]>> = HashMap::new();
    let mut add = |a: [i64; 2], b: [i64; 2]| next.entry(a).or_default().push(b);
    for y in 0..h as i64 {
        for x in 0..w as i64 {
            if !m(x, y) {
                continue;
            }
            if !m(x, y - 1) {
                add([x, y], [x + 1, y]);
            }
            if !m(x + 1, y) {
                add([x + 1, y], [x + 1, y + 1]);
            }
            if !m(x, y + 1) {
                add([x + 1, y + 1], [x, y + 1]);
            }
            if !m(x - 1, y) {
                add([x, y + 1], [x, y]);
            }
        }
    }
    let mut starts: Vec<[i64; 2]> = next.keys().copied().collect();
    starts.sort_unstable();
    let mut loops = Vec::new();
    for s in starts {
        if next.get(&s).is_none_or(|v| v.is_empty()) {
            continue;
        }
        let mut lp = vec![s];
        let mut cur = s;
        let mut prev_d: Option<[i64; 2]> = None;
        // Every edge into a vertex has one out of it; an open end can't happen, but the walk
        // stops there rather than panic if it did.
        while let Some(outs) = next.get_mut(&cur).filter(|o| !o.is_empty()) {
            let pick = if outs.len() == 1 {
                0
            } else {
                let d = prev_d.unwrap_or([1, 0]);
                let pref = [[-d[1], d[0]], d, [d[1], -d[0]]];
                pref.iter()
                    .find_map(|pd| {
                        outs.iter()
                            .position(|o| *o == [cur[0] + pd[0], cur[1] + pd[1]])
                    })
                    .unwrap_or(0)
            };
            let n = outs.swap_remove(pick);
            prev_d = Some([n[0] - cur[0], n[1] - cur[1]]);
            cur = n;
            if cur == s {
                break;
            }
            lp.push(cur);
        }
        loops.push(lp);
    }
    loops
}

/// A straight run of the outline.
#[derive(Clone, Copy)]
struct Run {
    a: P,
    b: P,
    /// Unit direction of travel.
    d: P,
    len: usize,
}

fn runs(lp: &[[i64; 2]]) -> Vec<Run> {
    let n = lp.len();
    let dir = |i: usize| {
        let (a, b) = (lp[i], lp[(i + 1) % n]);
        [b[0] - a[0], b[1] - a[1]]
    };
    // Start at a direction change.
    let s = (0..n)
        .find(|&i| dir(i) != dir((i + n - 1) % n))
        .unwrap_or(0);
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        let d = dir((s + i) % n);
        let mut j = i;
        while j + 1 < n && dir((s + j + 1) % n) == d {
            j += 1;
        }
        let a = lp[(s + i) % n];
        let b = lp[(s + j + 1) % n];
        out.push(Run {
            a: [a[0] as f64, a[1] as f64],
            b: [b[0] as f64, b[1] as f64],
            d: [d[0] as f64, d[1] as f64],
            len: j - i + 1,
        });
        i = j + 1;
    }
    out
}

fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1]]
}
fn add(a: P, b: P) -> P {
    [a[0] + b[0], a[1] + b[1]]
}
fn mul(a: P, s: f64) -> P {
    [a[0] * s, a[1] * s]
}
fn cross(a: P, b: P) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}
fn dot(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
/// Right-hand normal of a direction (y down).
fn right(d: P) -> P {
    [-d[1], d[0]]
}
/// Intersection of the lines p + t·u and q + s·v.
fn intersect(p: P, u: P, q: P, v: P) -> Option<P> {
    let den = cross(u, v);
    (den.abs() > 1e-12).then(|| add(p, mul(u, cross(sub(q, p), v) / den)))
}

/// What a polygon vertex is: a run's midpoint, or the point standing for a kept corner.
#[derive(Clone, Copy, PartialEq)]
enum V {
    Mid(usize),
    Corner(usize),
}

/// The smoothed polygon of one outline loop (F-001 runs 5–6).
fn polygon(lp: &[[i64; 2]]) -> Vec<P> {
    const LONG: usize = 2;
    const SHIFT: f64 = 0.25;
    let r = runs(lp);
    let m = r.len();
    if m < 3 {
        return r.iter().map(|x| x.a).collect();
    }
    let at = |k: isize| r[k.rem_euclid(m as isize) as usize];
    let mid: Vec<P> = r.iter().map(|x| mul(add(x.a, x.b), 0.5)).collect();
    let md = |k: isize| mid[k.rem_euclid(m as isize) as usize];
    let long = |k: isize| at(k).len >= LONG;
    // A corner is kept at the end of run k when both runs meeting there are long.
    // A short run whose neighbours run in opposite directions turns the outline around (the
    // end of a 1 px line, a tip): the long runs beside it stay straight to it.
    let turn = |k: isize| !long(k) && at(k - 1).d == mul(at(k + 1).d, -1.0);
    let kept = |k: isize| (long(k) && long(k + 1)) || turn(k) || turn(k + 1);
    let mut verts: Vec<(V, P)> = Vec::with_capacity(2 * m);
    for k in 0..m as isize {
        verts.push((V::Mid(k as usize), md(k)));
        if !kept(k) {
            continue;
        }
        let c = at(k).b;
        // Run k reached from a step: continue that line to the next run's line if it grows
        // the area (moves the boundary to the shape's side, the right).
        let qk = (!kept(k - 1))
            .then(|| intersect(md(k), sub(md(k), md(k - 1)), c, at(k + 1).d))
            .flatten()
            .filter(|q| dot(sub(*q, c), right(at(k).d)) > 1e-9);
        let qk1 = (!kept(k + 1))
            .then(|| intersect(md(k + 1), sub(md(k + 1), md(k + 2)), c, at(k).d))
            .flatten()
            .filter(|q| dot(sub(*q, c), right(at(k + 1).d)) > 1e-9);
        let p = match (qk, qk1) {
            (None, None) => c,
            (Some(q), None) | (None, Some(q)) => q,
            (Some(_), Some(_)) => intersect(
                md(k),
                sub(md(k), md(k - 1)),
                md(k + 1),
                sub(md(k + 1), md(k + 2)),
            )
            .unwrap_or(c),
        };
        verts.push((V::Corner(k as usize), p));
    }
    // The ¼ px translation of step-to-long-run segments.
    let n = verts.len();
    let mut out = Vec::with_capacity(n + 8);
    for i in 0..n {
        let (vi, p) = verts[i];
        out.push(p);
        let (vj, q) = verts[(i + 1) % n];
        let (V::Mid(ka), V::Mid(kb)) = (vi, vj) else {
            continue;
        };
        let (la, lb) = (r[ka].len, r[kb].len);
        // (the step, the long run, the long run comes after the step)
        let (step, run, after) = if la < LONG && lb >= LONG {
            (ka, kb, true)
        } else if lb < LONG && la >= LONG {
            (kb, ka, false)
        } else {
            continue;
        };
        let far = if after {
            run as isize + 1
        } else {
            run as isize - 1
        };
        if long(far) {
            // The run's far end is a kept corner: no translation when the run's line was
            // continued to it (the corner moved off the grid point).
            let far_v = if after { (i + 2) % n } else { (i + n - 1) % n };
            let geo = if after { r[run].b } else { r[run].a };
            if let (V::Corner(_), pt) = verts[far_v]
                && (pt[0] - geo[0]).abs().max((pt[1] - geo[1]).abs()) > 1e-9
            {
                continue;
            }
        } else if at(far).d == r[step].d {
            continue; // a monotone staircase
        }
        let t = mul(r[run].d, if after { SHIFT } else { -SHIFT });
        out.push(add(p, t));
        out.push(add(q, t));
    }
    out
}

/// Signed-area accumulation of the edge `p0 → p1` into per-row deltas: after a prefix sum
/// along each row, |sum| is the exact coverage of the closed polygons.
fn accumulate(rows: &mut [Vec<(usize, f64)>], p0: P, p1: P, w: usize) {
    if p0[1] == p1[1] {
        return;
    }
    let (s, a, b) = if p0[1] < p1[1] {
        (1.0, p0, p1)
    } else {
        (-1.0, p1, p0)
    };
    let dxdy = (b[0] - a[0]) / (b[1] - a[1]);
    let y_lo = a[1].floor().max(0.0) as usize;
    let y_hi = (b[1].ceil() as usize).min(rows.len());
    let put = |row: &mut Vec<(usize, f64)>, x: isize, v: f64| {
        let x = x.clamp(0, w as isize) as usize;
        row.push((x, v));
    };
    for (y, row) in rows.iter_mut().enumerate().take(y_hi).skip(y_lo) {
        let ya = a[1].max(y as f64);
        let yb = b[1].min(y as f64 + 1.0);
        if yb <= ya {
            continue;
        }
        let xa = a[0] + (ya - a[1]) * dxdy;
        let xb = a[0] + (yb - a[1]) * dxdy;
        let (lo, hi) = if xa < xb { (xa, xb) } else { (xb, xa) };
        let dy = yb - ya;
        let c0 = lo.floor() as isize;
        let c1 = (hi.ceil() as isize - 1).max(c0);
        for c in c0..=c1 {
            let (x0, x1) = (lo.max(c as f64), hi.min(c as f64 + 1.0));
            let dyc = if hi > lo {
                dy * (x1 - x0) / (hi - lo)
            } else {
                dy
            };
            if dyc == 0.0 {
                continue;
            }
            let xm = 0.5 * (x0 + x1) - c as f64;
            put(row, c, s * dyc * (1.0 - xm));
            put(row, c + 1, s * dyc * xm);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(w: usize, h: usize, f: impl Fn(usize, usize) -> bool) -> Vec<bool> {
        (0..w * h).map(|i| f(i % w, i / w)).collect()
    }

    #[test]
    fn rectangles_stay_sharp() {
        let a = grid(20, 16, |x, y| {
            !(4..15).contains(&x) || !(3..12).contains(&y)
        });
        let s = smooth_area(&a, 20, 16);
        for (i, &v) in s.iter().enumerate() {
            assert_eq!(v, if a[i] { 255 } else { 0 }, "pixel {i}");
        }
    }

    #[test]
    fn diagonal_staircase_is_cut_at_eighths() {
        // White above a 45° black staircase: CSP gives 223 / 32 along it (run 5).
        let a = grid(40, 40, |x, y| y < x || x >= 30 || y >= 30 || x < 5);
        let s = smooth_area(&a, 40, 40);
        assert_eq!((s[15 * 40 + 15], s[16 * 40 + 15]), (32, 0));
        assert_eq!(s[14 * 40 + 15], 223);
    }
}
