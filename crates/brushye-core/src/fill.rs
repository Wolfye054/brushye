//! The flood fill shared by the Fill tool and Auto select (spec F-001): from a clicked pixel of
//! a reference image, the coverage (0–255) of the area to fill or select.
//!
//! Observed exactly (F-001 runs 1–2):
//! - distance of a pixel to the clicked colour: the largest difference over R, G, B and A
//!   (straight values); tolerance t (percent) → threshold `T = round(2.55·t)`, half up;
//! - not connected: every pixel with `d ≤ T`; connected: those reachable from the click;
//! - anti-aliasing off: binary; on, t < 50: the region gets 255 and its 4-neighbours outside
//!   it `⌈255·(255 − d)/(255 − T)⌉`; on, t ≥ 50: region pixels with a 4-neighbour outside the
//!   region get `min(255, ⌊255·(255 − d)/T⌋)`, all others in the region 255, nothing outside;
//! - Close gap levels 1–5 hold gaps up to 1, 3, 5, 10 and 20 px and leak at 2, 4, 6, 11 and
//!   24 px (level 5's bound lies in 21–23);
//! - opacity o (percent): `⌊a·o/100⌋`; Fill and Auto select give the same coverage.
//!
//! Not observed, chosen here: 4-connectivity; the gap closing itself (a box closing whose
//! sizes reproduce the thresholds above; the fill's shape inside a closed gap is approximate).
//! Area scaling is not implemented (F-001: rule open).

/// Box sizes of the Close gap levels 1–5: a gap of up to `k − 1` px is closed.
const GAP_BOX: [usize; 5] = [2, 4, 6, 11, 21];

/// Settings of one fill or auto-select click.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FillParams {
    /// Tolerance in percent, 0..=100.
    pub tolerance: f64,
    /// Apply to connected pixels only.
    pub connected: bool,
    /// Close gap level: 0 = off, 1..=5.
    pub close_gap: u8,
    /// Anti-aliasing.
    pub aa: bool,
}

impl Default for FillParams {
    fn default() -> Self {
        Self {
            tolerance: 10.0,
            connected: true,
            close_gap: 0,
            aa: true,
        }
    }
}

/// Threshold for a tolerance in percent: `round(2.55·t)`, half up.
pub fn threshold(tolerance: f64) -> u32 {
    (tolerance.clamp(0.0, 100.0) * 255.0 / 100.0 + 0.5).floor() as u32
}

/// Coverage of a fill clicked at (`x`, `y`) on `reference` (straight RGBA, `w`×`h`).
pub fn coverage(reference: &[u8], w: u32, h: u32, x: u32, y: u32, p: &FillParams) -> Vec<u8> {
    let (wu, hu) = (w as usize, h as usize);
    let n = wu * hu;
    if x >= w || y >= h || reference.len() < n * 4 {
        return vec![0; n];
    }
    let seed_i = y as usize * wu + x as usize;
    let seed = &reference[seed_i * 4..seed_i * 4 + 4];
    let dist: Vec<u8> = reference
        .as_chunks::<4>()
        .0
        .iter()
        .map(|px| {
            px.iter()
                .zip(seed)
                .map(|(a, b)| a.abs_diff(*b))
                .max()
                .unwrap_or(0)
        })
        .collect();
    let t = threshold(p.tolerance);
    let inside: Vec<bool> = dist.iter().map(|&d| u32::from(d) <= t).collect();
    let region = if !p.connected {
        inside.clone()
    } else if p.close_gap == 0 {
        flood(&inside, wu, hu, seed_i)
    } else {
        closed_flood(
            &inside,
            wu,
            hu,
            seed_i,
            GAP_BOX[usize::from(p.close_gap.min(5)) - 1],
        )
    };
    let mut out = vec![0u8; n];
    if !p.aa {
        for (o, &r) in out.iter_mut().zip(&region) {
            if r {
                *o = 255;
            }
        }
        return out;
    }
    // The 4-neighbours of pixel i.
    let neighbours = |i: usize| {
        let (xx, yy) = (i % wu, i / wu);
        [
            (xx > 0).then(|| i - 1),
            (xx + 1 < wu).then(|| i + 1),
            (yy > 0).then(|| i - wu),
            (yy + 1 < hu).then(|| i + wu),
        ]
        .into_iter()
        .flatten()
    };
    if t >= 128 {
        // t ≥ 50: region pixels next to a pixel outside the region are softened by distance;
        // nothing outside (a region with no outside neighbour stays 255: the gray ramp).
        for i in 0..n {
            if region[i] {
                out[i] = if neighbours(i).any(|j| !region[j]) {
                    (255 * (255 - u32::from(dist[i])) / t).min(255) as u8
                } else {
                    255
                };
            }
        }
        return out;
    }
    // t < 50: the region, plus a soft border on the pixels next to it.
    for i in 0..n {
        if region[i] {
            out[i] = 255;
        }
    }
    for i in 0..n {
        if !region[i] && neighbours(i).any(|j| region[j]) {
            let num = 255 * (255 - u32::from(dist[i]));
            out[i] = num.div_ceil(255 - t).min(255) as u8;
        }
    }
    out
}

/// Coverage with an opacity in percent: `⌊a·o/100⌋`.
pub fn apply_opacity(cov: &mut [u8], opacity: u8) {
    let o = u32::from(opacity.min(100));
    for a in cov {
        *a = (u32::from(*a) * o / 100) as u8;
    }
}

/// Pixels of `mask` 4-connected to `seed`.
fn flood(mask: &[bool], w: usize, h: usize, seed: usize) -> Vec<bool> {
    let mut out = vec![false; mask.len()];
    if !mask[seed] {
        return out;
    }
    let mut stack = vec![seed];
    out[seed] = true;
    while let Some(i) = stack.pop() {
        let (x, y) = (i % w, i / w);
        let mut push = |j: usize| {
            if mask[j] && !out[j] {
                out[j] = true;
                stack.push(j);
            }
        };
        if x > 0 {
            push(i - 1);
        }
        if x + 1 < w {
            push(i + 1);
        }
        if y > 0 {
            push(i - w);
        }
        if y + 1 < h {
            push(i + w);
        }
    }
    out
}

/// Flood with gaps of up to `k − 1` px closed: the walls (outside `inside`) are grown by a
/// k×k box, the seed is flooded in what remains, and that area is grown back by the same box
/// within `inside`. A seed inside a grown wall floods without closing.
fn closed_flood(inside: &[bool], w: usize, h: usize, seed: usize, k: usize) -> Vec<bool> {
    let walls: Vec<bool> = inside.iter().map(|&v| !v).collect();
    let (a, b) = ((k - 1) / 2, k - 1 - (k - 1) / 2);
    let blocked = box_dilate(&walls, w, h, a, b);
    let open: Vec<bool> = blocked.iter().map(|&v| !v).collect();
    if !open[seed] {
        return flood(inside, w, h, seed);
    }
    let core = flood(&open, w, h, seed);
    let grown = box_dilate(&core, w, h, b, a);
    let reach: Vec<bool> = grown.iter().zip(inside).map(|(&g, &i)| g && i).collect();
    flood(&reach, w, h, seed)
}

/// Binary dilation by the box of offsets `−before..=after` in x and y (prefix sums).
fn box_dilate(m: &[bool], w: usize, h: usize, before: usize, after: usize) -> Vec<bool> {
    let pass = |src: &[bool], len: usize, lines: usize, at: &dyn Fn(usize, usize) -> usize| {
        let mut out = vec![false; src.len()];
        let mut pre = vec![0u32; len + 1];
        for line in 0..lines {
            for i in 0..len {
                pre[i + 1] = pre[i] + u32::from(src[at(line, i)]);
            }
            for i in 0..len {
                // Output i is set when a source in i − after ..= i + before is set (the
                // source spreads −before..=after around itself).
                let lo = i.saturating_sub(after);
                let hi = (i + before + 1).min(len);
                out[at(line, i)] = pre[hi] > pre[lo];
            }
        }
        out
    };
    let rows = pass(m, w, h, &|line, i| line * w + i);
    pass(&rows, h, w, &|line, i| i * w + line)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gray(v: &[u8]) -> Vec<u8> {
        v.iter().flat_map(|&g| [g, g, g, 255]).collect()
    }

    #[test]
    fn threshold_rounds_half_up() {
        let t: Vec<u32> = [0.0, 1.0, 2.0, 5.0, 10.0, 20.0, 30.0, 50.0]
            .iter()
            .map(|&t| threshold(t))
            .collect();
        assert_eq!(t, [0, 3, 5, 13, 26, 51, 77, 128]);
    }

    #[test]
    fn connected_and_not() {
        // 1×5: 255 0 255 255 0
        let img = gray(&[255, 0, 255, 255, 0]);
        let p = FillParams {
            aa: false,
            ..FillParams::default()
        };
        assert_eq!(coverage(&img, 5, 1, 2, 0, &p), [0, 0, 255, 255, 0]);
        let p = FillParams {
            connected: false,
            ..p
        };
        assert_eq!(coverage(&img, 5, 1, 2, 0, &p), [255, 0, 255, 255, 0]);
    }

    #[test]
    fn box_dilation_offsets() {
        let mut m = vec![false; 7];
        m[3] = true;
        let d = box_dilate(&m, 7, 1, 1, 2);
        assert_eq!(d, [false, false, true, true, true, true, false]);
    }
}
