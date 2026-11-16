//! Enclose and fill against CSP 1.13.2 exports (F-002 run 1). Inputs are our generated images
//! (`data/inputs/en1`, `en2`); each export's alpha is the fill on an empty layer. CSP's lassos
//! were rectangles drawn with the mouse; every shape is at least 10 px from them, so the
//! nominal rectangles stand in exactly.

mod common;

use brushye_core::fill::{EncloseParams, EncloseTarget, enclose};
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

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<[f64; 2]> {
    vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
}

fn diff(got: &[u8], want: &[u8]) -> usize {
    got.iter().zip(want).filter(|(a, b)| a != b).count()
}

/// The region rule with CSP's defaults: regions wholly inside the lasso, outlines included.
#[test]
fn regions_inside_the_lasso() {
    let img = rgba("inputs/en1.png");
    for (name, lasso) in [
        ("L1_A_halfB", rect(40.0, 40.0, 270.0, 180.0)),
        ("L2_nested", rect(40.0, 240.0, 240.0, 440.0)),
        ("L3_gap", rect(280.0, 255.0, 420.0, 400.0)),
        ("L4_gray_ring", rect(460.0, 255.0, 760.0, 405.0)),
        ("L5_empty", rect(100.0, 620.0, 300.0, 760.0)),
        ("L6_row", rect(40.0, 505.0, 335.0, 565.0)),
        ("L7_insideA", rect(80.0, 80.0, 140.0, 140.0)),
        ("L8_small", rect(545.0, 65.0, 615.0, 135.0)),
    ] {
        let got = enclose(&img, 800, 800, &lasso, &EncloseParams::default());
        let want = alpha(&format!("enclose/run1/{name}.png"));
        assert_eq!(diff(&got, &want), 0, "{name}");
    }
}

/// All ten Target color options on line art over transparency, with Area scaling (CSP's
/// default, `T*`) and without (`U*`).
#[test]
fn target_colors() {
    let img = rgba("inputs/en2.png");
    let names = [
        "all",
        "only_transp",
        "surr_transp",
        "only_black",
        "surr_black",
        "only_white",
        "surr_white",
        "semi_default",
        "except_transp",
        "incl_transp",
    ];
    for (i, target) in EncloseTarget::ALL.into_iter().enumerate() {
        for (prefix, scale) in [("T", 10), ("U", 0)] {
            let p = EncloseParams {
                target,
                scale,
                ..EncloseParams::default()
            };
            let got = enclose(&img, 800, 800, &rect(30.0, 30.0, 760.0, 410.0), &p);
            let want = alpha(&format!("enclose/run1/{prefix}{i}_{}.png", names[i]));
            assert_eq!(diff(&got, &want), 0, "{prefix} {}", target.name());
        }
    }
}
