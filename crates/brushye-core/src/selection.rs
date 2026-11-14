//! Selections (D-024, spec S-001): an 8-bit value per pixel, 0 = not selected, 255 = selected,
//! stored like a layer mask ([`Grid8`]). Operations work on a dense canvas-sized buffer.
//!
//! Observed exactly (S-001):
//! - Rectangle: the integer box [x, x+w) × [y, y+h), binary.
//! - Modes: Add `min(A+B, 255)`, Subtract `max(A−B, 0)`, Intersect `floor(A·B/255)`.
//! - Invert: `255 − S`. Expand/Shrink: square = Chebyshev dilation/erosion by w, rounded =
//!   the disk `dx² + dy² < (w+1)²` (exact at w = 1, 2 and 10; w = 3 and 25 differ, run 3).
//!   On anti-aliased edges both are grey max/min (exact, run 3).
//! - Painting clipped to a selection: the unquantised stroke alpha times S/255 (run 2).
//! - Polygons (Polyline, run 3): AA none = pixel centres inside (exact); AA = four
//!   sub-scanlines at y + k/4 with exact horizontal coverage, `round(255·c/4)` (±1 on a third
//!   of the edge pixels: the rounding is open).
//!
//! Open: the ellipse edge. CSP samples the same sub-scanlines, but its curve runs slightly
//! inside a true ellipse away from the equator (S-001 run 3); the true ellipse is used here, so
//! by D-024 the edge may change before 1.0.

use crate::mask::Grid8;
use crate::tile::TILE_SIZE;

/// The selection: values in a sparse grid.
pub type Selection = Grid8;

/// How a new shape combines with the current selection (Tool property "Selection mode").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SelectMode {
    #[default]
    New,
    Add,
    Subtract,
    Intersect,
}

impl SelectMode {
    pub const ALL: [SelectMode; 4] = [Self::New, Self::Add, Self::Subtract, Self::Intersect];
    pub fn name(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Add => "add",
            Self::Subtract => "subtract",
            Self::Intersect => "intersect",
        }
    }
    pub fn from_name(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.name() == s)
    }
}

/// A selection shape in canvas pixels.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// Integer box, binary (S-001).
    Rect { x: i64, y: i64, w: u32, h: u32 },
    /// The ellipse filling the box.
    Ellipse {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        aa: bool,
    },
    /// A closed polygon (lasso, polyline), non-zero winding.
    Polygon { points: Vec<[f64; 2]>, aa: bool },
}

impl Grid8 {
    /// The values inside a `w`×`h` canvas, row-major.
    pub fn to_dense(&self, w: u32, h: u32) -> Vec<u8> {
        let (w, h) = (w as usize, h as usize);
        let mut v = vec![self.background; w * h];
        for (&(tx, ty), t) in self.tiles() {
            let (x0, y0) = (tx as i64 * TILE_SIZE as i64, ty as i64 * TILE_SIZE as i64);
            for iy in 0..TILE_SIZE {
                let y = y0 + iy as i64;
                if y < 0 || y >= h as i64 {
                    continue;
                }
                for ix in 0..TILE_SIZE {
                    let x = x0 + ix as i64;
                    if x < 0 || x >= w as i64 {
                        continue;
                    }
                    v[y as usize * w + x as usize] = t.v[iy * TILE_SIZE + ix];
                }
            }
        }
        v
    }

    /// A grid holding `v` (a `w`×`h` canvas). The background is 0 or 255, whichever leaves
    /// fewer tiles to store; only tiles that differ from it are kept.
    pub fn from_dense(w: u32, h: u32, v: &[u8]) -> Self {
        let (w, h) = (w as usize, h as usize);
        let ts = TILE_SIZE;
        let (nx, ny) = (w.div_ceil(ts), h.div_ceil(ts));
        let uniform = |tx: usize, ty: usize, val: u8| {
            (ty * ts..((ty + 1) * ts).min(h)).all(|y| {
                v[y * w + tx * ts..y * w + ((tx + 1) * ts).min(w)]
                    .iter()
                    .all(|&p| p == val)
            })
        };
        let (mut zeros, mut fulls) = (0, 0);
        for ty in 0..ny {
            for tx in 0..nx {
                if uniform(tx, ty, 0) {
                    zeros += 1;
                } else if uniform(tx, ty, 255) {
                    fulls += 1;
                }
            }
        }
        let bg = if fulls > zeros { 255 } else { 0 };
        let mut g = Grid8::new(bg);
        for ty in 0..ny {
            for tx in 0..nx {
                if uniform(tx, ty, bg) {
                    continue;
                }
                let t = g.tile_mut((tx as i32, ty as i32));
                for y in ty * ts..((ty + 1) * ts).min(h) {
                    for x in tx * ts..((tx + 1) * ts).min(w) {
                        t.v[(y - ty * ts) * ts + x - tx * ts] = v[y * w + x];
                    }
                }
            }
        }
        g
    }

    /// Bounding box `[x0, y0, x1, y1)` of the selected (non-zero) pixels in a `w`×`h` canvas.
    pub fn bounds(&self, w: u32, h: u32) -> Option<[u32; 4]> {
        dense_bounds(&self.to_dense(w, h), w, h)
    }
}

/// Bounding box `[x0, y0, x1, y1)` of the non-zero values.
pub fn dense_bounds(v: &[u8], w: u32, h: u32) -> Option<[u32; 4]> {
    let (wu, hu) = (w as usize, h as usize);
    let rows: Vec<usize> = (0..hu)
        .filter(|&y| v[y * wu..(y + 1) * wu].iter().any(|&p| p != 0))
        .collect();
    let (&y0, &y1) = (rows.first()?, rows.last()?);
    let mut x0 = wu;
    let mut x1 = 0;
    for y in y0..=y1 {
        let row = &v[y * wu..(y + 1) * wu];
        if let Some(a) = row.iter().position(|&p| p != 0) {
            x0 = x0.min(a);
        }
        if let Some(b) = row.iter().rposition(|&p| p != 0) {
            x1 = x1.max(b + 1);
        }
    }
    Some([x0 as u32, y0 as u32, x1 as u32, y1 as u32 + 1])
}

/// Rasterize a shape onto a `w`×`h` canvas.
pub fn rasterize(shape: &Shape, w: u32, h: u32) -> Vec<u8> {
    let (wu, hu) = (w as usize, h as usize);
    match shape {
        Shape::Rect { x, y, w: rw, h: rh } => {
            let mut v = vec![0u8; wu * hu];
            let x0 = (*x).clamp(0, i64::from(w)) as usize;
            let x1 = (x + i64::from(*rw)).clamp(0, i64::from(w)) as usize;
            let y0 = (*y).clamp(0, i64::from(h)) as usize;
            let y1 = (y + i64::from(*rh)).clamp(0, i64::from(h)) as usize;
            for yy in y0..y1 {
                v[yy * wu + x0..yy * wu + x1].fill(255);
            }
            v
        }
        Shape::Ellipse {
            x,
            y,
            w: ew,
            h: eh,
            aa,
        } => {
            let (rx, ry) = (ew / 2.0, eh / 2.0);
            let (cx, cy) = (x + rx, y + ry);
            scan(w, h, *aa, |ys| {
                if rx <= 0.0 || ry <= 0.0 {
                    return Vec::new();
                }
                let t = 1.0 - ((ys - cy) / ry) * ((ys - cy) / ry);
                if t <= 0.0 {
                    return Vec::new();
                }
                let hw = rx * t.sqrt();
                vec![(cx - hw, cx + hw)]
            })
        }
        Shape::Polygon { points, aa } => scan(w, h, *aa, |ys| polygon_spans(points, ys)),
    }
}

/// Spans of a closed polygon on the horizontal line `ys` (non-zero winding).
fn polygon_spans(pts: &[[f64; 2]], ys: f64) -> Vec<(f64, f64)> {
    let n = pts.len();
    if n < 3 {
        return Vec::new();
    }
    let mut xs: Vec<(f64, i32)> = Vec::new();
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        if a[1] == b[1] {
            continue;
        }
        let (lo, hi, dir) = if a[1] < b[1] { (a, b, 1) } else { (b, a, -1) };
        if ys >= lo[1] && ys < hi[1] {
            let t = (ys - lo[1]) / (hi[1] - lo[1]);
            xs.push((lo[0] + t * (hi[0] - lo[0]), dir));
        }
    }
    xs.sort_by(|p, q| p.0.total_cmp(&q.0));
    let mut spans = Vec::new();
    let mut wind = 0;
    let mut start = 0.0;
    for (x, d) in xs {
        let was = wind != 0;
        wind += d;
        if !was && wind != 0 {
            start = x;
        } else if was && wind == 0 {
            spans.push((start, x));
        }
    }
    spans
}

/// Scanline fill: `spans(y)` gives the inside intervals on the line y. With `aa`, a pixel's
/// value is its horizontal coverage averaged over the sub-scanlines y + k/4 (S-001 run 3);
/// without, a pixel is in when its centre is.
fn scan(w: u32, h: u32, aa: bool, spans: impl Fn(f64) -> Vec<(f64, f64)>) -> Vec<u8> {
    let (wu, hu) = (w as usize, h as usize);
    let mut v = vec![0u8; wu * hu];
    let mut acc = vec![0f64; wu];
    for y in 0..hu {
        let row = &mut v[y * wu..(y + 1) * wu];
        if !aa {
            for (a, b) in spans(y as f64 + 0.5) {
                // Centres x + 0.5 in [a, b).
                let x0 = (a - 0.5).ceil().max(0.0) as usize;
                let x1 = ((b - 0.5).ceil().max(0.0) as usize).min(wu);
                if x0 < x1 {
                    row[x0..x1].fill(255);
                }
            }
            continue;
        }
        acc.fill(0.0);
        let mut any = false;
        for k in 0..4 {
            for (a, b) in spans(y as f64 + k as f64 / 4.0) {
                let (a, b) = (a.max(0.0), b.min(w as f64));
                if a >= b {
                    continue;
                }
                any = true;
                let (xa, xb) = (a.floor() as usize, (b.ceil() as usize).min(wu));
                for (x, c) in acc.iter_mut().enumerate().take(xb).skip(xa) {
                    let ov = (b.min(x as f64 + 1.0) - a.max(x as f64)).max(0.0);
                    *c += ov / 4.0;
                }
            }
        }
        if any {
            for (p, c) in row.iter_mut().zip(&acc) {
                *p = (c * 255.0 + 0.5).floor().clamp(0.0, 255.0) as u8;
            }
        }
    }
    v
}

/// Combine a shape's values `b` with the current selection `a` (S-001 run 2, exact).
pub fn combine(mode: SelectMode, a: Option<&[u8]>, b: &[u8]) -> Vec<u8> {
    let zero;
    let a = match a {
        Some(a) => a,
        None => {
            zero = vec![0u8; b.len()];
            &zero
        }
    };
    a.iter()
        .zip(b)
        .map(|(&a, &b)| {
            let (a, b) = (u32::from(a), u32::from(b));
            (match mode {
                SelectMode::New => b,
                SelectMode::Add => (a + b).min(255),
                SelectMode::Subtract => a.saturating_sub(b),
                SelectMode::Intersect => a * b / 255,
            }) as u8
        })
        .collect()
}

/// Invert selected area: `255 − S`.
pub fn invert(v: &mut [u8]) {
    for p in v {
        *p = 255 - *p;
    }
}

/// Move the values by (dx, dy); what leaves the canvas is lost, what enters is 0.
pub fn shift(v: &[u8], w: u32, h: u32, dx: i64, dy: i64) -> Vec<u8> {
    let (wu, hu) = (w as usize, h as usize);
    let mut out = vec![0u8; wu * hu];
    for y in 0..hu {
        let sy = y as i64 - dy;
        if sy < 0 || sy >= hu as i64 {
            continue;
        }
        for x in 0..wu {
            let sx = x as i64 - dx;
            if sx >= 0 && sx < wu as i64 {
                out[y * wu + x] = v[sy as usize * wu + sx as usize];
            }
        }
    }
    out
}

/// Expand (`grow`) or shrink by `r` pixels: max or min over the square (Chebyshev) or the
/// rounded disk `dx² + dy² < (r+1)²` (S-001). Pixels outside the canvas count as `edge`.
pub fn morph(v: &[u8], w: u32, h: u32, r: u32, round: bool, grow: bool, edge: u8) -> Vec<u8> {
    let (wu, hu) = (w as usize, h as usize);
    let r = r as usize;
    if r == 0 {
        return v.to_vec();
    }
    let pick = |a: u8, b: u8| if grow { a.max(b) } else { a.min(b) };
    // Horizontal pass with half-width `hx` on every row.
    let rows = |hx: usize| {
        let mut out = vec![0u8; wu * hu];
        for y in 0..hu {
            sliding(
                &v[y * wu..(y + 1) * wu],
                hx,
                grow,
                edge,
                &mut out[y * wu..(y + 1) * wu],
            );
        }
        out
    };
    if !round {
        let hpass = rows(r);
        let mut out = vec![0u8; wu * hu];
        let mut col = vec![0u8; hu];
        let mut res = vec![0u8; hu];
        for x in 0..wu {
            for y in 0..hu {
                col[y] = hpass[y * wu + x];
            }
            sliding(&col, r, grow, edge, &mut res);
            for y in 0..hu {
                out[y * wu + x] = res[y];
            }
        }
        return out;
    }
    let mut out = vec![if grow { 0u8 } else { 255u8 }; wu * hu];
    let lim = (r + 1) * (r + 1) - 1;
    let mut cache: Vec<(usize, Vec<u8>)> = Vec::new();
    for dy in -(r as i64)..=(r as i64) {
        let hx = isqrt(lim - (dy * dy) as usize);
        if !cache.iter().any(|(k, _)| *k == hx) {
            cache.push((hx, rows(hx)));
        }
        let Some((_, src)) = cache.iter().find(|(k, _)| *k == hx) else {
            continue;
        };
        for y in 0..hu {
            let sy = y as i64 + dy;
            let o = &mut out[y * wu..(y + 1) * wu];
            if sy < 0 || sy >= hu as i64 {
                for p in o.iter_mut() {
                    *p = pick(*p, edge);
                }
            } else {
                let s = &src[sy as usize * wu..(sy as usize + 1) * wu];
                for (p, &q) in o.iter_mut().zip(s) {
                    *p = pick(*p, q);
                }
            }
        }
    }
    out
}

fn isqrt(n: usize) -> usize {
    let mut x = (n as f64).sqrt() as usize;
    while x * x > n {
        x -= 1;
    }
    while (x + 1) * (x + 1) <= n {
        x += 1;
    }
    x
}

/// Sliding max (`grow`) or min over `[i − r, i + r]`; indices outside the row are `edge`.
fn sliding(src: &[u8], r: usize, grow: bool, edge: u8, out: &mut [u8]) {
    let n = src.len() as i64;
    let r = r as i64;
    let at = |j: i64| {
        if j < 0 || j >= n {
            edge
        } else {
            src[j as usize]
        }
    };
    let better = |a: u8, b: u8| if grow { a >= b } else { a <= b };
    let mut dq: std::collections::VecDeque<i64> = std::collections::VecDeque::new();
    let mut next = -r;
    for i in 0..n {
        while next <= i + r {
            let v = at(next);
            while let Some(&b) = dq.back() {
                if better(v, at(b)) {
                    dq.pop_back();
                } else {
                    break;
                }
            }
            dq.push_back(next);
            next += 1;
        }
        while let Some(&f) = dq.front() {
            if f < i - r {
                dq.pop_front();
            } else {
                break;
            }
        }
        out[i as usize] = dq.front().map_or(edge, |&f| at(f));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dense_round_trip_and_background_choice() {
        let mut v = vec![255u8; 600 * 300];
        v[1000] = 7;
        let g = Grid8::from_dense(600, 300, &v);
        assert_eq!(g.background, 255);
        assert_eq!(g.tile_count(), 1);
        assert_eq!(g.to_dense(600, 300), v);
        let z = Grid8::from_dense(600, 300, &vec![0; 600 * 300]);
        assert_eq!((z.background, z.tile_count()), (0, 0));
    }

    #[test]
    fn rect_modes_invert_and_shift() {
        let r = rasterize(
            &Shape::Rect {
                x: 2,
                y: 1,
                w: 3,
                h: 2,
            },
            8,
            4,
        );
        assert_eq!(r.iter().filter(|&&p| p == 255).count(), 6);
        assert_eq!(r[8 + 2], 255);
        assert_eq!(r[8 + 5], 0);
        assert_eq!(dense_bounds(&r, 8, 4), Some([2, 1, 5, 3]));
        let a = [238u8, 238, 0, 100];
        let b = [100u8, 20, 50, 255];
        assert_eq!(combine(SelectMode::Add, Some(&a), &b), [255, 255, 50, 255]);
        assert_eq!(
            combine(SelectMode::Subtract, Some(&a), &b),
            [138, 218, 0, 0]
        );
        assert_eq!(
            combine(SelectMode::Intersect, Some(&a), &b),
            [93, 18, 0, 100]
        );
        assert_eq!(combine(SelectMode::Subtract, None, &b), [0; 4]);
        let mut i = r.clone();
        invert(&mut i);
        assert_eq!(i[0], 255);
        let s = shift(&r, 8, 4, 3, 1);
        assert_eq!(dense_bounds(&s, 8, 4), Some([5, 2, 8, 4]));
    }

    #[test]
    fn square_and_round_morphology() {
        let (w, h) = (41, 41);
        let mut v = vec![0u8; w * h];
        v[20 * w + 20] = 255;
        let sq = morph(&v, w as u32, h as u32, 3, false, true, 0);
        assert_eq!(sq.iter().filter(|&&p| p == 255).count(), 49);
        // Rounded at r = 10: the disk dx² + dy² < 121 (S-001).
        let rd = morph(&v, w as u32, h as u32, 10, true, true, 0);
        let want = (-10i64..=10)
            .flat_map(|dy| (-10i64..=10).map(move |dx| (dx, dy)))
            .filter(|(dx, dy)| dx * dx + dy * dy < 121)
            .count();
        assert_eq!(rd.iter().filter(|&&p| p == 255).count(), want);
        // Shrinking the disk back by the same disk leaves the centre pixel.
        let back = morph(&rd, w as u32, h as u32, 10, true, false, 0);
        assert_eq!(back.iter().filter(|&&p| p == 255).count(), 1);
        // Shrink at the canvas edge: outside counts as unselected.
        let all = vec![255u8; w * h];
        let e = morph(&all, w as u32, h as u32, 2, false, false, 0);
        assert_eq!(e.iter().filter(|&&p| p == 255).count(), 37 * 37);
        let e = morph(&all, w as u32, h as u32, 2, false, false, 255);
        assert!(e.iter().all(|&p| p == 255));
    }

    #[test]
    fn polygon_and_ellipse_fill_their_box() {
        let p = rasterize(
            &Shape::Polygon {
                points: vec![[2.0, 2.0], [12.0, 2.0], [12.0, 8.0], [2.0, 8.0]],
                aa: true,
            },
            16,
            12,
        );
        assert_eq!(dense_bounds(&p, 16, 12), Some([2, 2, 12, 8]));
        assert!(p.iter().all(|&v| v == 0 || v == 255));
        let e = rasterize(
            &Shape::Ellipse {
                x: 400.0,
                y: 100.0,
                w: 250.0,
                h: 200.0,
                aa: false,
            },
            800,
            800,
        );
        assert_eq!(dense_bounds(&e, 800, 800), Some([400, 100, 650, 300]));
    }
}
