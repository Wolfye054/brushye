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
//! - Area scaling (run 3): Rectangle / Round grow (N > 0) or shrink (N < 0) the region by the
//!   square / the rounded kernel of S-001 run 5 (the canvas edge counts as outside), exact
//!   without anti-aliasing; with it, CSP smooths curved edges of the grown area (not modelled:
//!   the result here is the binary area, exact on straight edges). With anti-aliasing the
//!   scaled area's outline is smoothed in every mode (runs 5–6, `crate::smooth`).
//! - To darkest pixel (run 4): growth step by step into darker pixels, stopping at a line's
//!   darkest pixel (`darkest_grow`), exact without anti-aliasing; shrinking (N < 0) is the
//!   Rectangle shrink. With anti-aliasing CSP smooths the grown outline (not modelled).

use crate::brush::{self, Aa, BrushError, BrushSettings, Gap};
use crate::document::{Layer, LayerKind};
use crate::ids::LayerId;
use crate::input::Sample;

/// Box sizes of the Close gap levels 1–5: a gap of up to `k − 1` px is closed.
const GAP_BOX: [usize; 5] = [2, 4, 6, 11, 21];

/// Area scaling mode (Tool property > Area scaling > Scaling mode).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ScaleMode {
    Rectangle,
    Round,
    /// To darkest pixel (CSP's default).
    #[default]
    Darkest,
}

impl ScaleMode {
    pub const ALL: [ScaleMode; 3] = [Self::Rectangle, Self::Round, Self::Darkest];
    pub fn name(self) -> &'static str {
        match self {
            Self::Rectangle => "rectangle",
            Self::Round => "round",
            Self::Darkest => "darkest",
        }
    }
    pub fn from_name(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.name() == s)
    }
}

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
    /// Area scaling in px (0 = off) and its mode.
    pub scale: i32,
    pub scale_mode: ScaleMode,
}

impl Default for FillParams {
    fn default() -> Self {
        Self {
            tolerance: 10.0,
            connected: true,
            close_gap: 0,
            aa: true,
            scale: 0,
            scale_mode: ScaleMode::Darkest,
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
    if p.scale != 0 {
        return scaled_coverage(reference, w, h, &region, p.scale, p.scale_mode, p.aa);
    }
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

/// Area scaling replaces the soft border: the binary area grown or shrunk, then, with
/// anti-aliasing, its outline smoothed (F-001 runs 3–7, `smooth`).
fn scaled_coverage(
    reference: &[u8],
    w: u32,
    h: u32,
    region: &[bool],
    scale: i32,
    mode: ScaleMode,
    aa: bool,
) -> Vec<u8> {
    let scaled: Vec<bool> = if scale > 0 && mode == ScaleMode::Darkest {
        darkest_grow(
            reference,
            w as usize,
            h as usize,
            region,
            scale.unsigned_abs(),
        )
    } else {
        // Rectangle, Round, and To darkest pixel shrinking (the square, run 3).
        let v: Vec<u8> = region.iter().map(|&r| if r { 255 } else { 0 }).collect();
        crate::selection::morph(
            &v,
            w,
            h,
            scale.unsigned_abs(),
            mode == ScaleMode::Round,
            scale > 0,
            0,
        )
        .iter()
        .map(|&v| v > 0)
        .collect()
    };
    if aa {
        crate::smooth::smooth_area(&scaled, w, h)
    } else {
        scaled.iter().map(|&r| if r { 255 } else { 0 }).collect()
    }
}

/// Target color of Enclose and fill (F-002), in CSP's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum EncloseTarget {
    /// Target all colors.
    All,
    /// Only transparent.
    Transparent,
    /// Area surrounded by transparent.
    AroundTransparent,
    /// Only black.
    Black,
    /// Area surrounded by black.
    AroundBlack,
    /// Only white and transparent.
    WhiteTransparent,
    /// Area surrounded by white and transparent.
    AroundWhiteTransparent,
    /// Treat semi-transparent as transparent (CSP's default).
    #[default]
    SemiAsTransparent,
    /// All enclosed areas except transparency.
    EnclosedExceptTransparent,
    /// All enclosed areas including transparency.
    EnclosedIncludingTransparent,
}

impl EncloseTarget {
    pub const ALL: [EncloseTarget; 10] = [
        Self::All,
        Self::Transparent,
        Self::AroundTransparent,
        Self::Black,
        Self::AroundBlack,
        Self::WhiteTransparent,
        Self::AroundWhiteTransparent,
        Self::SemiAsTransparent,
        Self::EnclosedExceptTransparent,
        Self::EnclosedIncludingTransparent,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Transparent => "transparent",
            Self::AroundTransparent => "around_transparent",
            Self::Black => "black",
            Self::AroundBlack => "around_black",
            Self::WhiteTransparent => "white_transparent",
            Self::AroundWhiteTransparent => "around_white_transparent",
            Self::SemiAsTransparent => "semi_as_transparent",
            Self::EnclosedExceptTransparent => "enclosed_except_transparent",
            Self::EnclosedIncludingTransparent => "enclosed_including_transparent",
        }
    }
    /// CSP's label.
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "Target all colors",
            Self::Transparent => "Only transparent",
            Self::AroundTransparent => "Area surrounded by transparent",
            Self::Black => "Only black",
            Self::AroundBlack => "Area surrounded by black",
            Self::WhiteTransparent => "Only white and transparent",
            Self::AroundWhiteTransparent => "Area surrounded by white and transparent",
            Self::SemiAsTransparent => "Treat semi-transparent as transparent",
            Self::EnclosedExceptTransparent => "All enclosed areas except transparency",
            Self::EnclosedIncludingTransparent => "All enclosed areas including transparency",
        }
    }
    pub fn from_name(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.name() == s)
    }
}

/// Settings of one Enclose and fill gesture (F-002).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EncloseParams {
    pub target: EncloseTarget,
    /// Tolerance in percent (CSP's default 30).
    pub tolerance: f64,
    /// Anti-aliasing (CSP's default off).
    pub aa: bool,
    /// Area scaling in px (CSP's default 10, To darkest pixel).
    pub scale: i32,
    pub scale_mode: ScaleMode,
}

impl EncloseParams {
    /// Paint unfilled area's defaults (F-003): tolerance 80, no Area scaling, no
    /// anti-aliasing.
    pub fn unfilled() -> Self {
        Self {
            tolerance: 80.0,
            scale: 0,
            ..Self::default()
        }
    }
}

impl Default for EncloseParams {
    fn default() -> Self {
        Self {
            target: EncloseTarget::default(),
            tolerance: 30.0,
            aa: false,
            scale: 10,
            scale_mode: ScaleMode::Darkest,
        }
    }
}

/// Pixels whose centre lies inside the closed polygon `pts` (even-odd).
pub fn polygon_inside(pts: &[[f64; 2]], w: u32, h: u32) -> Vec<bool> {
    let (wu, hu) = (w as usize, h as usize);
    let mut out = vec![false; wu * hu];
    let n = pts.len();
    if n < 3 {
        return out;
    }
    let mut xs = Vec::new();
    for y in 0..hu {
        let cy = y as f64 + 0.5;
        xs.clear();
        for i in 0..n {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            if (a[1] <= cy) != (b[1] <= cy) {
                xs.push(a[0] + (cy - a[1]) * (b[0] - a[0]) / (b[1] - a[1]));
            }
        }
        xs.sort_by(f64::total_cmp);
        for pair in xs.as_chunks::<2>().0 {
            // Pixel centres x + 0.5 in [pair[0], pair[1]).
            let x0 = (pair[0] - 0.5).ceil().max(0.0) as usize;
            let x1 = ((pair[1] - 0.5).ceil().max(0.0) as usize).min(wu);
            for o in out[y * wu + x0.min(wu)..y * wu + x1].iter_mut() {
                *o = true;
            }
        }
    }
    out
}

/// Enclose and fill (F-002 run 1): the area filled by a lasso `pts` (canvas pixels) on
/// `reference` (straight RGBA). The image is divided into regions (4-connected); every region
/// lying **entirely inside the lasso** is filled, outlines included, then grown by Area
/// scaling. The regions, by Target color (tolerance threshold T = round(2.55·t)):
/// - "Only X": regions of X pixels; "Area surrounded by X": regions of the other pixels;
///   transparent = alpha ≤ T, black = alpha ≥ 255 − T and R, G, B ≤ T, white = alpha ≥
///   255 − T and R, G, B ≥ 255 − T (chosen: thresholds not observed; black at alpha 128 is
///   not black, as observed);
/// - the other four: regions of like colour (a pixel joins a region when within T of the
///   region's first pixel, chosen); identical in run 1 (open: what separates them).
///
/// Not modelled (open): Close gap (it did not hold a 10 px gap in run 1), the reference
/// options, anti-aliasing without scaling (binary).
pub fn enclose(reference: &[u8], w: u32, h: u32, pts: &[[f64; 2]], p: &EncloseParams) -> Vec<u8> {
    enclosed_regions(reference, w, h, &polygon_inside(pts, w, h), None, p)
}

/// Paint unfilled area (F-003): the area filled by a stroke of a round brush `size` px wide
/// (`samples` in canvas pixels). The stroke's **footprint** is the stroke as the brush engine
/// paints it without anti-aliasing, the samples at the pointer's integer canvas positions
/// (`⌊x⌋, ⌊y⌋`), shrunk by one pixel with the 4-neighbourhood (a pixel counts when it and its
/// four neighbours are painted; run 2, exact on a tap and on drags). Then every region of
/// Enclose and fill lying entirely inside the footprint is filled (run 1; all 13 captures
/// exact), with run 3's Target color and tolerance rules (`enclosed_regions`; all 60
/// captures exact, Area scaling included). Chosen, not observed: pixels past the canvas edge
/// count as painted for the shrink.
pub fn unfilled(
    reference: &[u8],
    w: u32,
    h: u32,
    samples: &[Sample],
    size: f64,
    gap: Gap,
    p: &EncloseParams,
) -> Result<Vec<u8>, BrushError> {
    let (wu, hu) = (w as usize, h as usize);
    let samples: Vec<Sample> = samples
        .iter()
        .map(|s| Sample {
            x: s.x.floor(),
            y: s.y.floor(),
            ..*s
        })
        .collect();
    let settings = BrushSettings {
        size,
        aa: Aa::None,
        gap,
        ..BrushSettings::default()
    };
    let mut tmp = Layer::new(LayerId(0), "footprint", LayerKind::Raster);
    brush::stroke(&mut tmp, (w, h), &settings, &samples)?;
    let painted: Vec<bool> = (0..wu * hu)
        .map(|i| tmp.pixel((i % wu) as u32, (i / wu) as u32)[3] > 0)
        .collect();
    let at = |x: usize, y: usize, dx: isize, dy: isize| match (
        x.checked_add_signed(dx),
        y.checked_add_signed(dy),
    ) {
        (Some(x), Some(y)) if x < wu && y < hu => painted[y * wu + x],
        _ => true,
    };
    let inside: Vec<bool> = (0..wu * hu)
        .map(|i| {
            let (x, y) = (i % wu, i / wu);
            painted[i]
                && [(-1, 0), (1, 0), (0, -1), (0, 1)]
                    .iter()
                    .all(|&(dx, dy)| at(x, y, dx, dy))
        })
        .collect();
    Ok(enclosed_regions(
        reference,
        w,
        h,
        &inside,
        Some(&painted),
        p,
    ))
}

/// The regions of `reference` (by `p.target`) lying entirely inside `inside`, filled and
/// scaled (F-002, F-003). `painted` (Paint unfilled area: the stroke as painted) selects
/// F-003 run 3's rules, observed for that sub tool only:
/// - transparent = alpha ≤ T, never alpha 255 (at tolerance 100 opaque lines stay opaque);
/// - the default (Treat semi-transparent as transparent): transparent pixels form regions
///   together, the others regions of exactly equal colour (opaque colours never merge);
/// - All enclosed areas except / including transparency: two classes, transparent and not;
///   "including" adds every non-transparent pixel the stroke paints.
///
/// Without it (Enclose and fill, F-002 run 1), those three behave like Target all colors.
fn enclosed_regions(
    reference: &[u8],
    w: u32,
    h: u32,
    inside: &[bool],
    painted: Option<&[bool]>,
    p: &EncloseParams,
) -> Vec<u8> {
    let (wu, hu) = (w as usize, h as usize);
    let n = wu * hu;
    if reference.len() < n * 4 || inside.len() < n {
        return vec![0; n];
    }
    let t = threshold(p.tolerance) as u8;
    let px = |i: usize| -> [u8; 4] {
        let mut q = [0u8; 4];
        q.copy_from_slice(&reference[i * 4..i * 4 + 4]);
        q
    };
    let unfilled = painted.is_some();
    let transparent = |q: [u8; 4]| q[3] <= t && (!unfilled || q[3] < 255);
    let black = |q: [u8; 4]| q[3] >= 255 - t && q[..3].iter().all(|&c| c <= t);
    let white = |q: [u8; 4]| q[3] >= 255 - t && q[..3].iter().all(|&c| c >= 255 - t);
    use EncloseTarget as E;
    // Which class a pixel belongs to (None: regions of like colour instead).
    let class: Option<Box<dyn Fn(usize) -> bool>> = match p.target {
        E::Transparent => Some(Box::new(move |i| transparent(px(i)))),
        E::AroundTransparent => Some(Box::new(move |i| !transparent(px(i)))),
        E::Black => Some(Box::new(move |i| black(px(i)))),
        E::AroundBlack => Some(Box::new(move |i| !black(px(i)))),
        E::WhiteTransparent => Some(Box::new(move |i| white(px(i)) || transparent(px(i)))),
        E::AroundWhiteTransparent => Some(Box::new(move |i| !(white(px(i)) || transparent(px(i))))),
        _ => None,
    };
    let member = |i: usize| class.as_ref().is_none_or(|c| c(i));
    let mut seen = vec![false; n];
    let mut region = vec![false; n];
    let mut stack = Vec::new();
    let mut comp = Vec::new();
    for s in 0..n {
        if seen[s] || !inside[s] || !member(s) {
            continue;
        }
        let seed = px(s);
        let like = |i: usize| {
            let q = px(i);
            if class.is_some() {
                member(i)
            } else if unfilled && p.target == E::SemiAsTransparent {
                match (transparent(seed), transparent(q)) {
                    (true, true) => true,
                    (false, false) => q == seed,
                    _ => false,
                }
            } else if unfilled
                && matches!(
                    p.target,
                    E::EnclosedExceptTransparent | E::EnclosedIncludingTransparent
                )
            {
                transparent(seed) == transparent(q)
            } else {
                q.iter().zip(&seed).all(|(a, b)| a.abs_diff(*b) <= t)
            }
        };
        comp.clear();
        stack.push(s);
        seen[s] = true;
        let mut enclosed = true;
        while let Some(i) = stack.pop() {
            comp.push(i);
            enclosed &= inside[i];
            let (x, y) = (i % wu, i / wu);
            for (ok, j) in [
                (x > 0, i.wrapping_sub(1)),
                (x + 1 < wu, i + 1),
                (y > 0, i.wrapping_sub(wu)),
                (y + 1 < hu, i + wu),
            ] {
                if ok && !seen[j] && like(j) {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
        if enclosed {
            for &i in &comp {
                region[i] = true;
            }
        }
    }
    if let Some(painted) = painted
        && p.target == E::EnclosedIncludingTransparent
    {
        for (i, r) in region.iter_mut().enumerate() {
            *r |= painted[i] && !transparent(px(i));
        }
    }
    if p.scale != 0 {
        return scaled_coverage(reference, w, h, &region, p.scale, p.scale_mode, p.aa);
    }
    region.iter().map(|&r| if r { 255 } else { 0 }).collect()
}

/// Coverage with an opacity in percent: `⌊a·o/100⌋`.
pub fn apply_opacity(cov: &mut [u8], opacity: u8) {
    let o = u32::from(opacity.min(100));
    for a in cov {
        *a = (u32::from(*a) * o / 100) as u8;
    }
}

/// Tone of a reference pixel for To darkest pixel (F-001 run 4): BT.601 luma (×1000, of the
/// straight colour) and alpha. A fully transparent pixel counts as white.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Tone {
    luma: i32,
    alpha: i32,
}

impl Tone {
    /// Beyond the canvas edge: darker than any pixel (chosen, not observed).
    const EDGE: Tone = Tone {
        luma: -1,
        alpha: 256,
    };

    fn of(px: &[u8; 4]) -> Tone {
        let [r, g, b, a] = px.map(i32::from);
        Tone {
            luma: if a == 0 {
                255_000
            } else {
                299 * r + 587 * g + 114 * b
            },
            alpha: a,
        }
    }

    /// Not lighter than `o`: luma no higher and alpha no lower (the two are not traded:
    /// 50 % black and opaque gray 126–130 are each not darker than the other, run 4 `dk6`).
    fn not_lighter(self, o: Tone) -> bool {
        self.luma <= o.luma && self.alpha >= o.alpha
    }
}

/// To darkest pixel, growing by `n` (F-001 run 4, exact without anti-aliasing): `n` steps,
/// each adding every pixel q outside the area with a 4-neighbour p inside it (as it was
/// before the step) where q is darker than p, or the same tone as p and the pixel beyond q
/// (q + (q − p)) is not lighter than q. So the growth runs down into lines and stops at their
/// darkest pixel, leaving out the last pixel of a flat run that a lighter pixel follows.
fn darkest_grow(reference: &[u8], w: usize, h: usize, region: &[bool], n: u32) -> Vec<bool> {
    let v: Vec<Tone> = reference.as_chunks::<4>().0.iter().map(Tone::of).collect();
    let at = |x: isize, y: isize| -> Tone {
        if x < 0 || y < 0 || x >= w as isize || y >= h as isize {
            Tone::EDGE
        } else {
            v[y as usize * w + x as usize]
        }
    };
    let mut area = region.to_vec();
    // Only pixels next to the area can join; track that frontier between steps.
    let mut frontier: Vec<usize> = (0..w * h).filter(|&i| !area[i]).collect();
    for _ in 0..n {
        let mut add = Vec::new();
        for &i in &frontier {
            let (x, y) = ((i % w) as isize, (i / w) as isize);
            let vq = v[i];
            let joins = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dy)| {
                // p = q − d inside the area; q + d is the pixel beyond.
                let (px, py) = (x - dx, y - dy);
                if px < 0 || py < 0 || px >= w as isize || py >= h as isize {
                    return false;
                }
                let pi = py as usize * w + px as usize;
                let vp = v[pi];
                area[pi] && vq.not_lighter(vp) && (vq != vp || at(x + dx, y + dy).not_lighter(vq))
            });
            if joins {
                add.push(i);
            }
        }
        if add.is_empty() {
            break;
        }
        for &i in &add {
            area[i] = true;
        }
        // The next step's candidates: neighbours of the pixels just added.
        let mut next = Vec::new();
        for &i in &add {
            let (x, y) = (i % w, i / w);
            for (ok, j) in [
                (x > 0, i.wrapping_sub(1)),
                (x + 1 < w, i + 1),
                (y > 0, i.wrapping_sub(w)),
                (y + 1 < h, i + w),
            ] {
                if ok && !area[j] {
                    next.push(j);
                }
            }
        }
        next.sort_unstable();
        next.dedup();
        frontier = next;
    }
    area
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
