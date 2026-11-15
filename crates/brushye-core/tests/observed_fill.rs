//! The flood fill against CSP 1.13.2 exports (F-001). Inputs are our generated images
//! (`data/inputs`); each export's alpha is the fill's coverage on an empty layer.

mod common;

use brushye_core::brush::{InkMode, ink};
use brushye_core::fill::{FillParams, ScaleMode, apply_opacity, coverage};
use common::Img;

const W: u32 = 800;
const H: u32 = 800;

fn rgba(name: &str) -> Vec<u8> {
    let img = Img::load_spec("fill", name);
    assert_eq!((img.w, img.h), (800, 800), "{name}");
    img.px.iter().flatten().copied().collect()
}

fn alpha(name: &str) -> Vec<u8> {
    Img::load_spec("fill", name)
        .px
        .iter()
        .map(|p| p[3])
        .collect()
}

fn diff(got: &[u8], want: &[u8]) -> usize {
    got.iter().zip(want).filter(|(a, b)| a != b).count()
}

fn params(t: f64, aa: bool) -> FillParams {
    FillParams {
        tolerance: t,
        connected: true,
        close_gap: 0,
        aa,
        ..FillParams::default()
    }
}

/// Tolerance and distance: four ramps (value depends on x), clicked at x = 401 (v = 128).
#[test]
fn ramps() {
    for (input, prefix, ts) in [
        (
            "ramp_gray",
            "gray",
            &[0.0, 1.0, 2.0, 5.0, 10.0, 20.0, 30.0, 50.0, 75.0, 100.0][..],
        ),
        ("ramp_red", "red", &[5.0, 10.0, 20.0][..]),
        ("ramp_redgreen", "redgreen", &[5.0, 10.0, 20.0][..]),
        ("ramp_alpha", "alpha", &[5.0, 10.0, 20.0][..]),
    ] {
        let img = rgba(&format!("inputs/{input}.png"));
        for &t in ts {
            let got = coverage(&img, W, H, 401, 400, &params(t, true));
            let want = alpha(&format!("{prefix}_t{t}.png"));
            assert_eq!(diff(&got, &want), 0, "{prefix} t={t}");
        }
    }
}

/// Anti-aliasing on a soft-edged ring, clicked between disk and ring.
#[test]
fn ring_anti_aliasing() {
    let img = rgba("inputs/ring.png");
    for t in [
        "0", "10", "20", "30", "40", "45", "49", "49.5", "49.8", "50", "51", "60", "75",
    ] {
        let got = coverage(
            &img,
            W,
            H,
            400,
            250,
            &params(t.parse().unwrap_or(0.0), true),
        );
        assert_eq!(diff(&got, &alpha(&format!("ring_t{t}.png"))), 0, "t={t}");
    }
    for t in ["10", "60"] {
        let got = coverage(
            &img,
            W,
            H,
            400,
            250,
            &params(t.parse().unwrap_or(0.0), false),
        );
        assert_eq!(
            diff(&got, &alpha(&format!("ringnoaa_t{t}.png"))),
            0,
            "no AA t={t}"
        );
    }
}

/// Run 2: not connected; opacity 50; filling the referenced layer itself (Normal mix).
#[test]
fn connected_off_opacity_and_editing_layer() {
    let img = rgba("inputs/ring.png");
    let base = coverage(&img, W, H, 400, 250, &params(10.0, true));
    assert_eq!(diff(&base, &alpha("run2/ring_as0.png")), 0);
    let p = FillParams {
        connected: false,
        ..params(10.0, true)
    };
    assert_eq!(
        diff(
            &coverage(&img, W, H, 400, 250, &p),
            &alpha("run2/ring_unconnected.png")
        ),
        0
    );
    let mut half = base.clone();
    apply_opacity(&mut half, 50);
    assert_eq!(diff(&half, &alpha("run2/ring_op50.png")), 0);
    // Black filled onto the ring layer itself: the colour is the Normal composite.
    let want = Img::load_spec("fill", "run2/ring_editing.png");
    for (i, &a) in base.iter().enumerate() {
        let dst = [img[i * 4], img[i * 4 + 1], img[i * 4 + 2], img[i * 4 + 3]];
        let got = ink::apply(InkMode::Normal, dst, [0, 0, 0], f64::from(a) / 255.0, false);
        assert_eq!(got, want.px[i], "pixel {i}");
    }
}

/// Close gap: which gaps hold (the fill stays inside the square) at each level. Squares on
/// a 4×2 grid, clicked inside; "leak" = the fill reaches the canvas corner.
#[test]
fn close_gap_thresholds() {
    let sets = [
        (
            "gaps",
            "gi",
            [1, 2, 3, 4, 6, 8, 12, 16],
            &[1u8, 2, 3, 4, 5][..],
        ),
        (
            "gaps2",
            "g2",
            [5, 7, 9, 10, 11, 20, 24, 32],
            &[3u8, 4, 5][..],
        ),
    ];
    for (input, prefix, gaps, levels) in sets {
        let img = rgba(&format!("inputs/{input}.png"));
        for &level in levels {
            for (i, gap) in gaps.iter().enumerate() {
                let (x0, y0) = (40 + (i as u32 % 4) * 190, 200 + (i as u32 / 4) * 260);
                let p = FillParams {
                    close_gap: level,
                    ..params(10.0, false)
                };
                let got = coverage(&img, W, H, x0 + 75, y0 + 75, &p);
                let want = alpha(&format!("{prefix}_L{level}_{i}.png"));
                let leak = |c: &[u8]| c[(20 * W + 20) as usize] > 0;
                assert_eq!(leak(&got), leak(&want), "{input} level {level} gap {gap}");
            }
        }
    }
}

/// Run 3: Area scaling, Rectangle and Round. Without anti-aliasing, exact (grown and shrunk);
/// with it, exact away from the curved disk (CSP smooths curves; not modelled).
#[test]
fn area_scaling_rectangle_and_round() {
    let scale = rgba("inputs/scale.png");
    let outside_disk = |i: usize| {
        let (x, y) = ((i % 800) as i64, (i / 800) as i64);
        (x - 650).pow(2) + (y - 550).pow(2) >= 80 * 80
    };
    for (mode, name) in [(ScaleMode::Rectangle, "rect"), (ScaleMode::Round, "round")] {
        for n in [1, 2, 3, 5, 10] {
            for aa in [false, true] {
                let p = FillParams {
                    scale: n,
                    scale_mode: mode,
                    ..params(10.0, aa)
                };
                let got = coverage(&scale, W, H, 50, 50, &p);
                let want = alpha(&format!(
                    "run3/sc_{name}{n}_{}.png",
                    if aa { "aa" } else { "no" }
                ));
                let bad = (0..640_000)
                    .filter(|&i| got[i] != want[i] && (!aa || outside_disk(i)))
                    .count();
                assert_eq!(bad, 0, "{name} {n} aa={aa}");
            }
        }
    }
    let outlines = rgba("inputs/outlines.png");
    for (mode, name) in [(ScaleMode::Rectangle, "rect"), (ScaleMode::Round, "round")] {
        let p = FillParams {
            scale: -3,
            scale_mode: mode,
            ..params(10.0, false)
        };
        let got = coverage(&outlines, W, H, 20, 20, &p);
        assert_eq!(
            diff(&got, &alpha(&format!("run3/ol_{name}-3_no.png"))),
            0,
            "{name} -3"
        );
    }
}
