//! Paint unfilled area against CSP 1.13.2 exports (F-003 runs 1–2). Inputs are our generated
//! images (`data/inputs/un1`, `en1`, `dab1`); each export's alpha is the fill on an empty
//! layer. Samples are the pixel centres the pointer hit (CSP's mouse strokes were straight
//! drags; the input's sampling does not matter, run 2 D2/D3).

mod common;

use brushye_core::brush::Gap;
use brushye_core::fill::{EncloseParams, EncloseTarget, unfilled};
use brushye_core::input::Sample;
use common::Img;

fn rgba(name: &str) -> Vec<u8> {
    Img::load_spec("fill", name)
        .px
        .iter()
        .flatten()
        .copied()
        .collect()
}

fn alpha(name: &str) -> Vec<u8> {
    Img::load_spec("fill", name)
        .px
        .iter()
        .map(|p| p[3])
        .collect()
}

/// A straight drag through pixel centres (one sample per end; a tap with one point).
fn drag(pts: &[[f64; 2]]) -> Vec<Sample> {
    pts.iter()
        .enumerate()
        .map(|(i, p)| Sample {
            x: p[0] + 0.5,
            y: p[1] + 0.5,
            t_us: i as u32 * 100_000,
            pressure: None,
            tilt: None,
            twist: None,
            eraser: false,
            barrel: [false; 2],
        })
        .collect()
}

fn run(input: &str, pts: &[[f64; 2]], size: f64) -> Vec<u8> {
    unfilled(
        &rgba(input),
        800,
        800,
        &drag(pts),
        size,
        Gap::Normal,
        &EncloseParams::unfilled(),
    )
    .unwrap()
}

fn diff(got: &[u8], want: &[u8]) -> usize {
    got.iter().zip(want).filter(|(a, b)| a != b).count()
}

/// Run 1: the regions lying wholly inside the stroke's footprint, with CSP's defaults.
#[test]
fn regions_inside_the_stroke() {
    let cases: &[(&str, &str, &[[f64; 2]], f64)] = &[
        (
            "un1.png",
            "S1_gridrow",
            &[[40.0, 70.0], [300.0, 70.0]],
            20.0,
        ),
        (
            "un1.png",
            "S2_square",
            &[[450.0, 150.0], [550.0, 150.0]],
            20.0,
        ),
        (
            "un1.png",
            "S3_holes",
            &[[50.0, 410.0], [340.0, 410.0]],
            20.0,
        ),
        (
            "un1.png",
            "S6_onecell",
            &[[126.0, 131.0], [136.0, 131.0]],
            20.0,
        ),
        (
            "un1.png",
            "E1_b60_gridrow",
            &[[70.0, 91.0], [250.0, 91.0]],
            60.0,
        ),
        ("un1.png", "E2_b60_tap", &[[131.0, 131.0]], 60.0),
        ("en1.png", "F1_b60_small30", &[[75.0, 535.0]], 60.0),
        ("en1.png", "F2_b60_square40", &[[580.0, 100.0]], 60.0),
        ("en1.png", "G_b54_square40", &[[580.0, 100.0]], 54.0),
        ("en1.png", "G_b55_square40", &[[580.0, 100.0]], 55.0),
        ("en1.png", "G_b56_square40", &[[580.0, 100.0]], 56.0),
        ("en1.png", "G_b57_square40", &[[580.0, 100.0]], 57.0),
        ("en1.png", "G_b58_square40", &[[580.0, 100.0]], 58.0),
    ];
    for &(input, name, pts, size) in cases {
        let got = run(&format!("inputs/{input}"), pts, size);
        let want = alpha(&format!("unfilled/run1/{name}.png"));
        assert_eq!(diff(&got, &want), 0, "{name}");
    }
}

/// Run 2: on a checkerboard every pixel is its own region, so the fill is the footprint.
/// The drags' last ~1 px differ (CSP's last dab reaches a little further).
#[test]
fn stroke_footprint() {
    let cases: &[(&str, &[[f64; 2]], f64, usize)] = &[
        ("D1_tap20", &[[200.0, 200.0]], 20.0, 0),
        ("D2_coarse20", &[[100.0, 300.0], [500.0, 300.0]], 20.0, 0),
        ("D3_fine20", &[[100.0, 400.0], [500.0, 400.0]], 20.0, 0),
        ("D4_diag20", &[[100.0, 500.0], [400.0, 700.0]], 20.0, 6),
        ("D5_coarse60", &[[100.0, 150.0], [500.0, 150.0]], 60.0, 12),
    ];
    for &(name, pts, size, max_diff) in cases {
        let got = run("inputs/dab1.png", pts, size);
        let want = alpha(&format!("unfilled/run2/{name}.png"));
        let d = diff(&got, &want);
        assert!(d <= max_diff, "{name}: {d} px differ (max {max_diff})");
    }
}

/// Capture name, input, stroke, brush size, settings.
type Case<'a> = (String, &'a [u8], &'a [[f64; 2]], f64, EncloseParams);

/// Run 3: Target color, tolerance and Area scaling. `un2`: square outlines whose top gaps are
/// bridged by grays on white (row A) and by black at alphas on transparency (row B); `en2`:
/// F-002's line art. Names: `_t<tolerance>` (default target), `_g<i>` (Target color item i,
/// tolerance 80), `_s10` (Area scaling 10, To darkest pixel).
#[test]
fn target_color_tolerance_and_scaling() {
    const A: &[[f64; 2]] = &[[70.0, 130.0], [750.0, 130.0]];
    const B: &[[f64; 2]] = &[[70.0, 530.0], [750.0, 530.0]];
    const W: &[[f64; 2]] = &[[100.0, 250.0], [700.0, 250.0]];
    const T: &[[f64; 2]] = &[[100.0, 700.0], [700.0, 700.0]];
    const X: &[[f64; 2]] = &[[400.0, 400.0]];
    const R1: &[[f64; 2]] = &[[100.0, 120.0], [680.0, 120.0]];
    const R2: &[[f64; 2]] = &[[100.0, 320.0], [380.0, 320.0]];
    let un2 = rgba("inputs/un2.png");
    let en2 = rgba("inputs/en2.png");
    let mut cases: Vec<Case> = Vec::new();
    let base = EncloseParams::unfilled();
    for t in [100, 80, 60, 40, 20, 5, 0] {
        let p = EncloseParams {
            tolerance: f64::from(t),
            ..base
        };
        cases.push((format!("A_t{t}"), &un2, A, 100.0, p));
        cases.push((format!("B_t{t}"), &un2, B, 100.0, p));
    }
    for (g, target) in EncloseTarget::ALL.into_iter().enumerate() {
        let p = EncloseParams { target, ..base };
        cases.push((format!("A_g{g}"), &un2, A, 100.0, p));
        cases.push((format!("B_g{g}"), &un2, B, 100.0, p));
        if matches!(g, 0 | 7 | 8 | 9) {
            for (r, pts) in [("W", W), ("T", T), ("X", X)] {
                cases.push((format!("{r}_g{g}"), &un2, pts, 100.0, p));
            }
        }
        if matches!(g, 0 | 1 | 7 | 8 | 9) {
            cases.push((format!("R1_g{g}"), &en2, R1, 160.0, p));
            cases.push((format!("R2_g{g}"), &en2, R2, 160.0, p));
        }
    }
    let scaled = |target| EncloseParams {
        target,
        scale: 10,
        ..base
    };
    cases.push((
        "R1_s10".into(),
        &en2,
        R1,
        160.0,
        scaled(EncloseTarget::SemiAsTransparent),
    ));
    cases.push((
        "R2_s10".into(),
        &en2,
        R2,
        160.0,
        scaled(EncloseTarget::SemiAsTransparent),
    ));
    cases.push((
        "R1_g1_s10".into(),
        &en2,
        R1,
        160.0,
        scaled(EncloseTarget::Transparent),
    ));
    cases.push((
        "R2_g1_s10".into(),
        &en2,
        R2,
        160.0,
        scaled(EncloseTarget::Transparent),
    ));
    let mut bad = Vec::new();
    for (name, img, pts, size, p) in &cases {
        let got = unfilled(img, 800, 800, &drag(pts), *size, Gap::Normal, p).unwrap();
        let want = alpha(&format!("unfilled/run3/{name}.png"));
        let d = diff(&got, &want);
        if d != 0 {
            bad.push(format!("{name}: {d}"));
        }
    }
    assert_eq!(cases.len(), 60);
    assert!(bad.is_empty(), "{bad:?}");
}
