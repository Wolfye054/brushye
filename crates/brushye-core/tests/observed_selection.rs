//! Selections against CSP 1.13.2 exports (S-001). A selection's readout is the exported alpha
//! of Edit > Fill on an empty layer (alpha = S).

mod common;

use brushye_core::brush::{self, BrushSettings};
use brushye_core::input::Sample;
use brushye_core::selection::{SelectMode, Shape, combine, invert, morph, rasterize};
use brushye_core::{Grid8, Layer, LayerId, LayerKind};
use common::Img;

const W: u32 = 800;
const H: u32 = 800;

fn alpha(name: &str) -> Vec<u8> {
    let img = Img::load_spec("selection", &format!("{name}.png"));
    assert_eq!((img.w, img.h), (800, 800), "{name}");
    img.px.iter().map(|p| p[3]).collect()
}

fn mismatches(got: &[u8], want: &[u8]) -> usize {
    got.iter().zip(want).filter(|(a, b)| a != b).count()
}

fn rect() -> Vec<u8> {
    rasterize(
        &Shape::Rect {
            x: 100,
            y: 100,
            w: 200,
            h: 150,
        },
        W,
        H,
    )
}

#[test]
fn rectangle_and_invert() {
    let r = rect();
    assert_eq!(r, alpha("rect_aa_none"));
    let mut i = r;
    invert(&mut i);
    assert_eq!(i, alpha("rect_inverted"));
}

#[test]
fn expand_and_shrink_square_and_round() {
    for (round, name) in [(false, "square"), (true, "round")] {
        let e = morph(&rect(), W, H, 10, round, true, 0);
        assert_eq!(
            mismatches(&e, &alpha(&format!("expand10_{name}"))),
            0,
            "{name}"
        );
        let s = morph(&rect(), W, H, 10, round, false, 0);
        assert_eq!(
            mismatches(&s, &alpha(&format!("shrink10_{name}"))),
            0,
            "{name}"
        );
        // The binary ellipse (AA none), shrunk.
        let s = morph(&alpha("ellipse_aa_none"), W, H, 10, round, false, 0);
        assert_eq!(
            mismatches(&s, &alpha(&format!("ellipse_shrink10_{name}"))),
            0,
            "ellipse {name}"
        );
    }
}

/// Selection modes on a soft selection A (from a layer's alpha) and an AA ellipse B
/// (S-001 run 2): exact on all 605 partial pixels.
#[test]
fn selection_modes() {
    let (a, b) = (alpha("layer_sel_A"), alpha("ellipse_B"));
    for (mode, name) in [
        (SelectMode::Add, "Ae_add"),
        (SelectMode::Subtract, "Ae_sub"),
        (SelectMode::Intersect, "Ae_int"),
    ] {
        assert_eq!(
            mismatches(&combine(mode, Some(&a), &b), &alpha(name)),
            0,
            "{name}"
        );
    }
}

/// Edit > Delete leaves 255 − S; Mask Outside Selection is M = S (S-001; checked through the
/// command layer in brushye-command's tests). Here: the ellipse edge rule is still open, so
/// the rasterized ellipse is only close to CSP's.
#[test]
fn ellipse_rasterization_is_close_open_s001() {
    for (aa, name, limit) in [
        (false, "ellipse_aa_none", 100),
        (true, "ellipse_aa_middle", 900),
    ] {
        let e = rasterize(
            &Shape::Ellipse {
                x: 400.0,
                y: 100.0,
                w: 250.0,
                h: 200.0,
                aa,
            },
            W,
            H,
        );
        let n = mismatches(&e, &alpha(name));
        assert!(n <= limit, "{name}: {n} mismatches");
    }
}

fn line(y: f64) -> Vec<Sample> {
    (0..=150)
        .map(|k| Sample {
            x: 300.0 + 3.0 * f64::from(k),
            y,
            t_us: k * 8000,
            pressure: Some(1.0),
            tilt: None,
            twist: None,
            eraser: false,
            barrel: [false; 2],
        })
        .collect()
}

/// A 93 % stroke across the AA ellipse selection: where the stroke is solid (alpha 238), the
/// clipped alpha and the colour over white match CSP exactly (S-001 run 2).
#[test]
fn strokes_are_clipped_by_the_selection() {
    let sel = Grid8::from_dense(W, H, &alpha("ellipse_aa_middle"));
    let s = BrushSettings {
        size: 10.0,
        opacity: 0.93,
        ..BrushSettings::default()
    };
    let want_alpha = Img::load_spec("selection", "stroke93_in_ellipse.png");
    let want_white = Img::load_spec("selection", "stroke93_over_white.png");
    let mut white = Layer::new(LayerId(1), "w", LayerKind::Raster);
    white.put_pixels((W, H), 0, 0, W, H, &vec![255u8; (W * H * 4) as usize]);
    let mut empty = Layer::new(LayerId(2), "e", LayerKind::Raster);
    for y in [175.0, 225.0] {
        brush::stroke_clipped(&mut empty, (W, H), &s, &line(y), Some(&sel))
            .unwrap_or_else(|e| panic!("{e}"));
        brush::stroke_clipped(&mut white, (W, H), &s, &line(y), Some(&sel))
            .unwrap_or_else(|e| panic!("{e}"));
    }
    let mut checked = 0;
    for row in (173..=177).chain(223..=227) {
        for x in 0..W {
            let i = (row * W + x) as usize;
            let v = sel.value(x, row);
            if v == 0 || v == 255 {
                continue;
            }
            assert_eq!(
                empty.pixel(x, row)[3],
                want_alpha.px[i][3],
                "alpha ({x},{row})"
            );
            assert_eq!(empty.pixel(x, row)[0..3], [0, 0, 0]);
            assert_eq!(
                white.pixel(x, row),
                want_white.px[i],
                "over white ({x},{row})"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 24);
    // Nothing is painted outside the selection.
    for y in 0..H {
        for x in 0..W {
            if sel.value(x, y) == 0 {
                assert_eq!(empty.pixel(x, y)[3], 0);
            }
        }
    }
}

/// S-001 run 3 data (`data/run3`).
fn alpha3(name: &str) -> Vec<u8> {
    alpha(&format!("run3/{name}"))
}

/// A Polyline triangle with known corners: AA none exact; AA within ±1 (rounding open).
#[test]
fn polygon_edges() {
    let tri = |aa| {
        rasterize(
            &Shape::Polygon {
                points: vec![[100.0, 100.0], [300.0, 130.0], [150.0, 300.0]],
                aa,
            },
            W,
            H,
        )
    };
    assert_eq!(mismatches(&tri(false), &alpha3("tri_aa_none")), 0);
    let (got, want) = (tri(true), alpha3("tri_aa_middle"));
    assert!(got.iter().zip(&want).all(|(a, b)| a.abs_diff(*b) <= 1));
    assert!(mismatches(&got, &want) <= 211);
}

/// Expand and Shrink on an anti-aliased ellipse: grey max/min over the square or the disk.
#[test]
fn morphology_on_soft_edges() {
    let s = alpha3("S_E1");
    for (round, grow, name) in [
        (false, true, "eE1_exp5_square"),
        (true, true, "eE1_exp5_round"),
        (false, false, "eE1_shr5_square"),
        (true, false, "eE1_shr5_round"),
    ] {
        assert_eq!(
            mismatches(&morph(&s, W, H, 5, round, grow, 0), &alpha3(name)),
            0,
            "{name}"
        );
    }
    // Rounded expansion of the rectangle, with the measured kernels (run 5).
    for w in [1, 2, 3, 25] {
        let e = morph(&rect(), W, H, w, true, true, 0);
        assert_eq!(
            mismatches(&e, &alpha3(&format!("rect_exp{w}_round"))),
            0,
            "w={w}"
        );
    }
}

/// Edit > Fill over existing pixels lays the colour on like Normal ink with alpha S: exact over
/// an opaque layer, and all but one pixel over a partly transparent one.
#[test]
fn fill_over_existing_pixels() {
    use brushye_core::brush::{InkMode, ink};
    let rgba = |n: &str| Img::load_spec("selection", &format!("run3/{n}.png")).px;
    for (sel, backdrop, want, limit) in [
        ("S_E1", None, "fill_white_over_black", 0),
        ("S_E3", Some("del_backdrop"), "fill_white_over_partial", 1),
    ] {
        let s = alpha3(sel);
        let back = backdrop.map(rgba);
        let want = rgba(want);
        let mut bad = 0;
        for (i, &v) in s.iter().enumerate() {
            if v == 0 {
                continue;
            }
            let dst = back.as_ref().map_or([0, 0, 0, 255], |b| b[i]);
            let got = ink::apply(
                InkMode::Normal,
                dst,
                [255, 255, 255],
                f64::from(v) / 255.0,
                false,
            );
            let w = want[i];
            if got[3] != w[3] || (w[3] > 0 && got[..3] != w[..3]) {
                bad += 1;
            }
        }
        assert!(bad <= limit, "{sel}: {bad}");
    }
}

/// S-001 run 5: rounded Expand of a single pixel is the kernel itself; rounded Shrink erodes by
/// the same kernel.
#[test]
fn rounded_kernels() {
    let mut seed = vec![0u8; (W * H) as usize];
    seed[(400 * W + 400) as usize] = 255;
    for w in [14, 25, 100] {
        let got = morph(&seed, W, H, w, true, true, 0);
        assert_eq!(mismatches(&got, &alpha(&format!("run5/k_{w}"))), 0, "w={w}");
    }
    let r = alpha("run5/r300");
    for w in [14, 16] {
        let got = morph(&r, W, H, w, true, false, 0);
        assert_eq!(mismatches(&got, &alpha(&format!("run5/s_{w}"))), 0, "w={w}");
    }
}
