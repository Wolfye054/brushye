//! Layer mask commands (D-023, L-008): create, paint, settings, apply, delete, undo, replay.

use brushye_command::Session;
use serde_json::Value as Json;

fn run(s: &mut Session, script: &str) {
    s.run_script(script)
        .unwrap_or_else(|e| panic!("line {}: {}", e.line, e.msg));
}

fn q(s: &mut Session, query: &str) -> Json {
    s.run_line(query)
        .unwrap_or_else(|e| panic!("{query}: {e}"))
        .unwrap_or(Json::Null)
}

const SETUP: &str = "doc.new 64 64
layer.new L1
layer.fill L1 color=#ffffff
layer.new L2
layer.fill L2 color=#ff000080";

#[test]
fn create_paint_apply_and_undo() {
    let mut s = Session::new();
    run(&mut s, SETUP);
    run(&mut s, "mask.new L2");
    // Create layer mask selects the mask (U-014); a new mask reveals everything.
    assert_eq!(q(&mut s, "?doc.info")["target"], "mask");
    assert_eq!(q(&mut s, "?layer.mask_pixel L2 x=10 y=10"), 255);
    let m = &q(&mut s, "?layer.get L2")["mask"];
    assert_eq!(
        (
            m["enabled"].clone(),
            m["linked"].clone(),
            m["show_area"].clone()
        ),
        (true.into(), true.into(), false.into())
    );

    // The eraser on the mask hides; opaque ink (any colour, even black) reveals again.
    run(
        &mut s,
        "brush.stroke L2 color=#000000 size=20 ink=erase samples=[0,32,1,0;64,32,1,1000] target=mask",
    );
    assert_eq!(q(&mut s, "?layer.mask_pixel L2 x=32 y=32"), 0);
    assert_eq!(
        q(&mut s, "?pixel x=32 y=32"),
        serde_json::json!([255, 255, 255, 255])
    );
    assert_eq!(
        q(&mut s, "?layer.pixel L2 x=32 y=32"),
        serde_json::json!([255, 0, 0, 128])
    );
    run(
        &mut s,
        "brush.stroke L2 color=#000000 size=8 samples=[0,32,1,0;64,32,1,1000] target=mask",
    );
    assert_eq!(q(&mut s, "?layer.mask_pixel L2 x=32 y=32"), 255);
    run(&mut s, "history.undo");
    assert_eq!(q(&mut s, "?layer.mask_pixel L2 x=32 y=32"), 0);

    // Settings; same-kind changes in a row merge into one History step (H-002).
    let steps = |s: &mut Session| q(s, "?history")["steps"].as_array().map(Vec::len);
    run(&mut s, "mask.set L2 enabled=false");
    let before = steps(&mut s);
    run(
        &mut s,
        "mask.set L2 enabled=true\nmask.set L2 show_area=true",
    );
    assert_eq!(steps(&mut s), before.map(|n| n + 1));
    run(&mut s, "mask.set L2 enabled=false");
    assert_eq!(q(&mut s, "?pixel x=32 y=32"), q(&mut s, "?pixel x=32 y=5"));
    run(&mut s, "mask.set L2 enabled=true");

    // Apply: alpha becomes floor(a*M/255), the mask goes, the layer is the target again.
    run(&mut s, "mask.apply L2");
    assert_eq!(q(&mut s, "?layer.get L2")["mask"], Json::Null);
    assert_eq!(q(&mut s, "?layer.pixel L2 x=32 y=32")[3], 0);
    assert_eq!(q(&mut s, "?layer.pixel L2 x=32 y=5")[3], 128);
    assert_eq!(q(&mut s, "?doc.info")["target"], "layer");

    // The journal replays to the same document.
    let hash = q(&mut s, "?doc.hash");
    let mut r =
        Session::replay(&s.journal_text()).unwrap_or_else(|e| panic!("{}: {}", e.line, e.msg));
    assert_eq!(q(&mut r, "?doc.hash"), hash);
}

#[test]
fn hide_all_masks_folders_and_errors() {
    let mut s = Session::new();
    run(&mut s, SETUP);
    run(&mut s, "mask.new L2 fill=hide");
    assert_eq!(
        q(&mut s, "?pixel x=3 y=3"),
        serde_json::json!([255, 255, 255, 255])
    );
    run(&mut s, "mask.delete L2\nlayer.select L2");
    assert_eq!(
        q(&mut s, "?pixel x=3 y=3"),
        serde_json::json!([255, 127, 127, 255])
    );
    run(&mut s, "layer.group L5 layer=L2\nmask.new L5 fill=hide");
    assert_eq!(
        q(&mut s, "?pixel x=3 y=3"),
        serde_json::json!([255, 255, 255, 255])
    );
    for bad in [
        "mask.new L5",
        "mask.apply L5",
        "mask.delete L1",
        "mask.set L1 enabled=false",
        "mask.set L5",
        "layer.select L1 target=mask",
        "brush.stroke L1 color=#000000 size=3 samples=[1,1,1,0] target=mask",
    ] {
        assert!(s.run_line(bad).is_err(), "{bad} should fail");
    }
}
