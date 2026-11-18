//! Enclose and fill against CSP 1.13.2 exports (F-002 runs 1, 2). Inputs are our generated images
//! (`data/inputs/en1`, `en2`, `un2`, `wh`); each export's alpha is the fill on an empty layer.
//! CSP's lassos were drawn with the mouse (runs 1, 2: rectangles, 8 events per side, see
//! [`rect`]) or the pen (run 3); the lasso is the events' B-spline ([`lasso`]).

mod common;

use brushye_core::fill::{EncloseParams, EncloseTarget, enclose, lasso};
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

/// The mouse events of a rectangular lasso as the harness drew it (`h.sh` `lasso`): press at
/// (x0, y0), then 8 moves along each side clockwise and back to the start, each at
/// `corner + (next − corner)·k / 8` in integers (truncated toward zero), then release. Mouse
/// input is not stabilized (run 3), so the polygon is the events' B-spline.
fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<[f64; 2]> {
    let c = [[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]].map(|[x, y]| [x as i64, y as i64]);
    let mut ev = vec![c[0]];
    for s in c.windows(2) {
        for k in 1..=8 {
            ev.push([
                s[0][0] + (s[1][0] - s[0][0]) * k / 8,
                s[0][1] + (s[1][1] - s[0][1]) * k / 8,
            ]);
        }
    }
    let ev: Vec<[f64; 2]> = ev.iter().map(|&[x, y]| [x as f64, y as f64]).collect();
    lasso(&ev, 0)
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

/// A capture's name, input, lasso and settings.
type Case<'a> = (String, &'a [u8], &'a [[f64; 2]], EncloseParams);

/// Run 2: Target color and Tolerance on F-003's `un2` (rows A / B, plain white W, plain
/// transparent T, the white/transparent boundary X) and `en2` (R1, R2), Area scaling off.
/// `_t<t>`: the default target at tolerance t; `_g<i>`: Target color item i at tolerance 80.
/// Exact, except "All enclosed areas including transparency" (item 9), which adds the opaque
/// pixels inside the lasso: the lasso's own edge, within 4 px (run 3's model).
#[test]
fn target_color_and_tolerance() {
    let a = rect(20.0, 80.0, 795.0, 185.0);
    let b = rect(20.0, 480.0, 795.0, 585.0);
    let w = rect(100.0, 200.0, 700.0, 300.0);
    let t = rect(100.0, 650.0, 700.0, 750.0);
    let x = rect(350.0, 350.0, 450.0, 450.0);
    let r1 = rect(40.0, 40.0, 740.0, 200.0);
    let r2 = rect(40.0, 240.0, 420.0, 400.0);
    let un2 = rgba("inputs/un2.png");
    let en2 = rgba("inputs/en2.png");
    let base = EncloseParams {
        scale: 0,
        ..EncloseParams::default()
    };
    let mut cases: Vec<Case> = Vec::new();
    for tv in [30, 100, 80, 60, 40, 20, 5, 0] {
        let p = EncloseParams {
            tolerance: f64::from(tv),
            ..base
        };
        cases.push((format!("A_t{tv}"), &un2, &a, p));
        cases.push((format!("B_t{tv}"), &un2, &b, p));
    }
    for (g, target) in EncloseTarget::ALL.into_iter().enumerate() {
        let p = EncloseParams {
            target,
            tolerance: 80.0,
            ..base
        };
        cases.push((format!("A_g{g}"), &un2, &a, p));
        cases.push((format!("B_g{g}"), &un2, &b, p));
        if matches!(g, 0 | 7 | 8 | 9) {
            for (r, pts) in [("W", &w), ("T", &t), ("X", &x)] {
                cases.push((format!("{r}_g{g}"), &un2, pts, p));
            }
            cases.push((format!("R1_g{g}"), &en2, &r1, p));
            cases.push((format!("R2_g{g}"), &en2, &r2, p));
        }
    }
    assert_eq!(cases.len(), 56);
    let mut bad = Vec::new();
    for (name, img, pts, p) in &cases {
        let got = enclose(img, 800, 800, pts, p);
        let want = alpha(&format!("enclose/run2/{name}.png"));
        let d = diff(&got, &want);
        let allowed = if name.ends_with("_g9") { 4 } else { 0 };
        if d > allowed {
            bad.push(format!("{name}: {d} differ"));
        }
        if d > 0 {
            eprintln!("{name}: {d}");
        }
    }
    assert!(bad.is_empty(), "{bad:?}");
}

/// Run 3: the lasso's shape. Plain white with "All enclosed areas including transparency", so
/// the fill is the lasso's inside; mouse (`S…`, never stabilized) and pen (`P<S>_…`) lassos
/// with their logged events. Within 10 px for S ≤ 20; S 50 and 100 are open (as for brushes,
/// B-007) and skipped.
#[test]
fn lasso_shape_and_stabilization() {
    let img = rgba("inputs/wh.png");
    let p = EncloseParams {
        target: EncloseTarget::EnclosedIncludingTransparent,
        scale: 0,
        ..EncloseParams::default()
    };
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../specs/behavior/fill/data/enclose/run3/events.txt"
    );
    let text = std::fs::read_to_string(path).unwrap();
    let mut bad = Vec::new();
    let mut n = 0;
    for line in text.lines() {
        let (name, ev) = line.split_once(' ').unwrap();
        let events: Vec<[f64; 2]> = ev
            .split(';')
            .map(|e| {
                let (x, y) = e.split_once(',').unwrap();
                [x.parse().unwrap(), y.parse().unwrap()]
            })
            .collect();
        let stab: u8 = if name.starts_with('P') {
            name[1..name.find('_').unwrap()]
                .trim_end_matches('b')
                .parse()
                .unwrap()
        } else {
            0
        };
        if stab >= 50 {
            continue;
        }
        let got = enclose(&img, 800, 800, &lasso(&events, stab), &p);
        let want = alpha(&format!("enclose/run3/{name}.png"));
        let d = diff(&got, &want);
        if d > 10 {
            bad.push(format!("{name}: {d}"));
        }
        n += 1;
    }
    assert_eq!(n, 18);
    assert!(bad.is_empty(), "{bad:?}");
}
