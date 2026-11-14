//! Layer masks against observed CSP 1.13.2 exports (L-008; folder masks L-004 run 2).
//! The masks are rebuilt from the observed readouts (an opaque layer's exported alpha = M).

mod common;

use brushye_core::render::flatten;
use brushye_core::{BlendMode, Document, Layer, LayerId, LayerKind, Mask};
use common::Img;

fn ok<T, E: std::fmt::Display>(r: Result<T, E>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

fn raster(id: u32, img: &Img) -> Layer {
    let mut l = Layer::new(LayerId(id), format!("L{id}"), LayerKind::Raster);
    let rgba: Vec<u8> = img.px.iter().flatten().copied().collect();
    l.put_pixels(
        (img.w as u32, img.h as u32),
        0,
        0,
        img.w as u32,
        img.h as u32,
        &rgba,
    );
    l
}

/// A mask whose values are an image's alpha channel.
fn mask_from_alpha(img: &Img) -> Mask {
    let mut l = Layer::new(LayerId(0), "m", LayerKind::Raster);
    let rgba: Vec<u8> = img.px.iter().flatten().copied().collect();
    l.put_pixels(
        (img.w as u32, img.h as u32),
        0,
        0,
        img.w as u32,
        img.h as u32,
        &rgba,
    );
    let mut m = Mask::new(255);
    m.set_from_layer(&l, (img.w as u32, img.h as u32));
    m
}

fn diff(d: &Document, want: &str) -> (usize, u8) {
    let got = flatten(d);
    let want = Img::load(want);
    let mut n = 0;
    let mut max = 0;
    for (g, w) in got.px.as_chunks::<4>().0.iter().zip(&want.px) {
        // Fully transparent pixels are equal whatever their colour bytes: CSP's PNG export
        // writes (255,255,255,0) there, Brushye (0,0,0,0).
        if g[3] == 0 && w[3] == 0 {
            continue;
        }
        let m = g
            .iter()
            .zip(w)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap_or(0);
        if m > 0 {
            n += 1;
            max = max.max(m);
        }
    }
    (n, max)
}

fn exact(d: &Document, want: &str) {
    assert_eq!(diff(d, want), (0, 0), "{want}");
}

/// base, then `top` (L2) with a mask; `f` adjusts the document.
fn doc(top: &str, mask_from: &str, f: impl FnOnce(&mut Document)) -> Document {
    let base = Img::load("inputs/base.png");
    let mut d = ok(Document::new(256, 512, 72, None));
    ok(d.add_layer(raster(1, &base), None));
    ok(d.add_layer(raster(2, &Img::load(top)), None));
    ok(d.layer_mut(LayerId(2))).mask = Some(mask_from_alpha(&Img::load(mask_from)));
    f(&mut d);
    d
}

const M1: &str = "observed3/mask1_top_alone.png";
const M2: &str = "observed3/mask2_readout_alone.png";

#[test]
fn l008_opaque_layer_with_mask() {
    exact(
        &doc("inputs/top.png", M1, |_| {}),
        "observed3/mask1_normal.png",
    );
    exact(
        &doc("inputs/top.png", M1, |d| {
            ok(d.layer_mut(LayerId(2))).opacity = 50
        }),
        "observed3/mask1_normal_op50.png",
    );
    // top alone (base hidden): the export is the masked layer itself.
    exact(
        &doc("inputs/top.png", M1, |d| {
            ok(d.layer_mut(LayerId(1))).visible = false
        }),
        "observed3/mask1_top_alone.png",
    );
    // A disabled mask is ignored.
    let d = doc("inputs/top.png", M1, |d| {
        if let Some(m) = ok(d.layer_mut(LayerId(2))).mask.as_mut() {
            m.enabled = false;
        }
    });
    let unmasked = Img::load("inputs/top.png");
    let got = flatten(&d);
    assert!(
        got.px
            .as_chunks::<4>()
            .0
            .iter()
            .zip(&unmasked.px)
            .all(|(a, b)| a == b)
    );
}

/// Multiply with a mask equals CSP's own Multiply with that alpha (L-008); Brushye's Multiply
/// with partial alpha is L-002's open case: max 2, partial-alpha pixels only.
#[test]
fn l008_multiply_open_l002() {
    let d = doc("inputs/top.png", M1, |d| {
        ok(d.layer_mut(LayerId(2))).blend = BlendMode::Multiply;
    });
    let (n, max) = diff(&d, "observed3/mask1_multiply.png");
    assert!(max <= 2, "max {max}");
    let m = Img::load(M1);
    let partial = m.px.iter().filter(|p| p[3] > 0 && p[3] < 255).count();
    assert!(n <= partial, "{n} > {partial}");
}

#[test]
fn l008_semi_transparent_layer_with_mask() {
    let ta = "inputs/top_alpha.png";
    exact(&doc(ta, M2, |_| {}), "observed3/mask2_top_alpha_normal.png");
    exact(
        &doc(ta, M2, |d| ok(d.layer_mut(LayerId(2))).opacity = 50),
        "observed3/mask2_top_alpha_normal_op50.png",
    );
    exact(
        &doc(ta, M2, |d| ok(d.layer_mut(LayerId(1))).visible = false),
        "observed3/mask2_top_alpha_alone.png",
    );
}

/// L-004 run 2: a mask on a Normal and on a Through folder [top (Multiply), top_alpha (Screen)].
#[test]
fn l004_folder_masks() {
    for (blend, name) in [
        (BlendMode::Normal, "normal"),
        (BlendMode::Through, "through"),
    ] {
        let base = Img::load("inputs/base.png");
        let mut d = ok(Document::new(256, 512, 72, None));
        ok(d.add_layer(raster(1, &base), None));
        ok(d.add_layer(raster(2, &Img::load("inputs/top.png")), None));
        ok(d.layer_mut(LayerId(2))).blend = BlendMode::Multiply;
        ok(d.group(Layer::new(LayerId(4), "F", LayerKind::Folder), LayerId(2)));
        ok(d.add_layer(
            raster(3, &Img::load("inputs/top_alpha.png")),
            Some(LayerId(2)),
        ));
        ok(d.layer_mut(LayerId(3))).blend = BlendMode::Screen;
        let f = ok(d.layer_mut(LayerId(4)));
        f.blend = blend;
        f.mask = Some(mask_from_alpha(&Img::load(M2)));
        exact(&d, &format!("observed3/folder_{name}_mask.png"));
    }
}

/// L-008 run 3: masks in clipping groups, folder masks with opacity, ungroup of a masked
/// folder. base, A = top_alpha, T = top; masks are the standard eraser bands (M2).
mod run3 {
    use super::*;

    fn stack(f: impl FnOnce(&mut Document)) -> Document {
        let mut d = ok(Document::new(256, 512, 72, None));
        ok(d.add_layer(raster(1, &Img::load("inputs/base.png")), None));
        ok(d.add_layer(raster(2, &Img::load("inputs/top_alpha.png")), None));
        ok(d.add_layer(raster(3, &Img::load("inputs/top.png")), None));
        f(&mut d);
        d
    }
    fn mask(d: &mut Document, id: u32) {
        ok(d.layer_mut(LayerId(id))).mask = Some(mask_from_alpha(&Img::load(M2)));
    }

    #[test]
    fn masks_in_clipping_groups() {
        exact(
            &stack(|d| {
                ok(d.layer_mut(LayerId(3))).clip = true;
                mask(d, 2);
            }),
            "observed3/mask3_clip_masked_base.png",
        );
        for (enabled, want) in [
            (true, "observed3/mask3_clip_masked_clipped_screen.png"),
            (false, "observed3/mask3_clip_screen_mask_disabled.png"),
        ] {
            exact(
                &stack(|d| {
                    let t = ok(d.layer_mut(LayerId(3)));
                    t.clip = true;
                    t.blend = BlendMode::Screen;
                    mask(d, 3);
                    if let Some(m) = ok(d.layer_mut(LayerId(3))).mask.as_mut() {
                        m.enabled = enabled;
                    }
                }),
                want,
            );
        }
    }

    #[test]
    fn folder_masks_with_opacity_and_ungroup() {
        for (blend, want) in [
            (BlendMode::Normal, "observed3/mask3_folder_mask_op50.png"),
            (BlendMode::Through, "observed3/mask3_through_mask_op50.png"),
        ] {
            exact(
                &stack(|d| {
                    ok(d.layer_mut(LayerId(3))).visible = false;
                    ok(d.group(Layer::new(LayerId(5), "F", LayerKind::Folder), LayerId(2)));
                    let f = ok(d.layer_mut(LayerId(5)));
                    f.blend = blend;
                    f.opacity = 50;
                    mask(d, 5);
                }),
                want,
            );
        }
        // Ungroup discards the folder (its mask and opacity): base + top_alpha.
        exact(
            &stack(|d| ok(d.layer_mut(LayerId(3))).visible = false),
            "observed3/mask3_ungroup_masked_folder.png",
        );
    }
}
