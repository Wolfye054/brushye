//! Fill tool and Auto select commands (F-001): fill.area, select.auto, undo and replay.

use brushye_command::Session;
use serde_json::{Value as Json, json};

fn run(s: &mut Session, script: &str) {
    s.run_script(script)
        .unwrap_or_else(|e| panic!("line {}: {}", e.line, e.msg));
}

fn q(s: &mut Session, query: &str) -> Json {
    s.run_line(query)
        .unwrap_or_else(|e| panic!("{query}: {e}"))
        .unwrap_or(Json::Null)
}

/// White paper layer L1 with a closed black square outline (10..30), and an empty L2.
const SETUP: &str = "doc.new 60 40
layer.new L1
layer.fill L1 color=#ffffff
layer.pixels L1 x=10 y=10 w=20 h=1 data=b64:AAAA/wAAAP8AAAD/AAAA/wAAAP8AAAD/AAAA/wAAAP8AAAD/AAAA/wAAAP8AAAD/AAAA/wAAAP8AAAD/AAAA/wAAAP8AAAD/AAAA/wAAAP8=
layer.pixels L1 x=10 y=29 w=20 h=1 data=b64:AAAA/wAAAP8AAAD/AAAA/wAAAP8AAAD/AAAA/wAAAP8AAAD/AAAA/wAAAP8AAAD/AAAA/wAAAP8AAAD/AAAA/wAAAP8AAAD/AAAA/wAAAP8=
layer.new L2";

fn sides(s: &mut Session) {
    // Left and right sides of the square, one pixel at a time.
    for y in 10..30 {
        run(
            s,
            &format!(
                "layer.pixels L1 x=10 y={y} w=1 h=1 data=b64:AAAA/w==\nlayer.pixels L1 x=29 y={y} w=1 h=1 data=b64:AAAA/w=="
            ),
        );
    }
}

#[test]
fn fill_inside_a_closed_shape() {
    let mut s = Session::new();
    run(&mut s, SETUP);
    sides(&mut s);
    // Refer other layers onto the empty L2: the inside of the square only.
    run(
        &mut s,
        "fill.area L2 x=20 y=20 color=#ff0000 refer=all aa=false",
    );
    assert_eq!(
        q(&mut s, "?layer.pixel L2 x=20 y=20"),
        json!([255, 0, 0, 255])
    );
    assert_eq!(q(&mut s, "?layer.pixel L2 x=5 y=5")[3], 0);
    assert_eq!(q(&mut s, "?layer.pixel L2 x=10 y=20")[3], 0);
    // Not connected: every white pixel.
    run(
        &mut s,
        "history.undo\nfill.area L2 x=20 y=20 color=#ff0000 refer=all aa=false connected=false",
    );
    assert_eq!(q(&mut s, "?layer.pixel L2 x=5 y=5")[3], 255);
    // Refer only to the layer itself (L2 is empty: everything is one colour).
    run(
        &mut s,
        "history.undo\nfill.area L2 x=20 y=20 color=#00ff00 opacity=50",
    );
    assert_eq!(
        q(&mut s, "?layer.pixel L2 x=5 y=5"),
        json!([0, 255, 0, 127])
    );
    // On L1 itself, Normal over white.
    run(
        &mut s,
        "history.undo\nfill.area L1 x=20 y=20 color=#000000 aa=false opacity=50",
    );
    assert_eq!(
        q(&mut s, "?layer.pixel L1 x=20 y=20"),
        json!([128, 128, 128, 255])
    );
}

#[test]
fn close_gap_and_auto_select() {
    let mut s = Session::new();
    run(&mut s, SETUP);
    sides(&mut s);
    // A 2 px gap in the top side.
    run(
        &mut s,
        "layer.pixels L1 x=19 y=10 w=2 h=1 data=b64://////////8=",
    );
    run(
        &mut s,
        "fill.area L2 x=20 y=20 color=#ff0000 refer=all aa=false",
    );
    assert_eq!(
        q(&mut s, "?layer.pixel L2 x=5 y=5")[3],
        255,
        "leaks through the gap"
    );
    // Close gap level 2 holds gaps up to 3 px (F-001).
    run(
        &mut s,
        "history.undo\nfill.area L2 x=20 y=20 color=#ff0000 refer=all aa=false close_gap=2",
    );
    assert_eq!(q(&mut s, "?layer.pixel L2 x=5 y=5")[3], 0, "held");
    assert_eq!(q(&mut s, "?layer.pixel L2 x=20 y=20")[3], 255);
    // Auto select selects the same area as the fill.
    run(
        &mut s,
        "select.auto L2 x=20 y=20 refer=all aa=false close_gap=2",
    );
    assert_eq!(q(&mut s, "?selection")["bounds"], json!([11, 11, 29, 29]));
    run(&mut s, "select.auto L2 x=5 y=5 refer=all aa=false mode=add");
    assert_eq!(q(&mut s, "?selection.pixel x=5 y=5"), 255);
    // Errors.
    assert!(s.run_line("fill.area L2 x=99 y=5 color=#ff0000").is_err());
    assert!(
        s.run_line("fill.area L2 x=5 y=5 color=#ff0000 close_gap=6")
            .is_err()
    );
    assert!(
        s.run_line("fill.area L2 x=5 y=5 color=#ff0000 tolerance=101")
            .is_err()
    );
    // The journal replays to the same document.
    let r = Session::replay(&s.journal_text()).unwrap_or_else(|e| panic!("{}", e.msg));
    let h = |s: &Session| s.document().map(|d| d.content_hash());
    assert_eq!(h(&r), h(&s));
}

#[test]
fn area_scaling() {
    let mut s = Session::new();
    run(&mut s, SETUP);
    sides(&mut s);
    // Inside the square (11..29 is white): grow by 2 px covers the 1 px outline and one more.
    run(
        &mut s,
        "fill.area L2 x=20 y=20 color=#ff0000 refer=all aa=false scale=2 scale_mode=rectangle",
    );
    assert_eq!(q(&mut s, "?layer.pixel L2 x=10 y=20")[3], 255);
    assert_eq!(q(&mut s, "?layer.pixel L2 x=9 y=20")[3], 255);
    assert_eq!(q(&mut s, "?layer.pixel L2 x=8 y=20")[3], 0);
    // Shrink by 3 px.
    run(
        &mut s,
        "history.undo\nfill.area L2 x=20 y=20 color=#ff0000 refer=all aa=false scale=-3 scale_mode=round",
    );
    assert_eq!(q(&mut s, "?layer.pixel L2 x=13 y=20")[3], 0);
    assert_eq!(q(&mut s, "?layer.pixel L2 x=14 y=20")[3], 255);
    // To darkest pixel (the default mode): into the 1 px outline, not past it.
    run(
        &mut s,
        "history.undo\nfill.area L2 x=20 y=20 color=#ff0000 refer=all aa=false scale=10",
    );
    assert_eq!(q(&mut s, "?layer.pixel L2 x=10 y=20")[3], 255);
    assert_eq!(q(&mut s, "?layer.pixel L2 x=9 y=20")[3], 0);
}

#[test]
fn refer_other_layers_defaults_to_darkest_pixel() {
    let mut s = Session::new();
    // Fill: Refer other layers (item 1) and Auto select: Refer to all layers (item 1).
    for tool in ["fill", "auto_select"] {
        let txt = q(&mut s, &format!("?subtool tool={tool} item=1")).to_string();
        assert!(txt.contains("darkest"), "{tool}: {txt}");
        let txt = q(&mut s, &format!("?subtool tool={tool} item=0")).to_string();
        assert!(!txt.contains("darkest"), "{tool}: {txt}");
    }
}

#[test]
fn enclose_and_fill() {
    let mut s = Session::new();
    run(&mut s, SETUP);
    sides(&mut s);
    // A lasso around the square: its outline and inside are regions wholly inside the lasso;
    // the white background reaches past it (F-002).
    run(
        &mut s,
        "fill.enclose L2 points=[5,5;45,5;45,35;5,35] color=#ff0000",
    );
    assert_eq!(
        q(&mut s, "?layer.pixel L2 x=20 y=20"),
        json!([255, 0, 0, 255])
    );
    assert_eq!(q(&mut s, "?layer.pixel L2 x=10 y=20")[3], 255, "outline");
    assert_eq!(q(&mut s, "?layer.pixel L2 x=5 y=5")[3], 0, "background");
    // Only black: the outline alone.
    run(
        &mut s,
        "history.undo\nfill.enclose L2 points=[5,5;45,5;45,35;5,35] color=#ff0000 target=black",
    );
    assert_eq!(q(&mut s, "?layer.pixel L2 x=20 y=20")[3], 0);
    assert_eq!(q(&mut s, "?layer.pixel L2 x=10 y=20")[3], 255);
    // A lasso cutting the square fills nothing of it.
    run(
        &mut s,
        "history.undo\nfill.enclose L2 points=[5,5;20,5;20,35;5,35] color=#ff0000",
    );
    assert_eq!(q(&mut s, "?layer.pixel L2 x=15 y=20")[3], 0);
    // Errors.
    assert!(
        s.run_line("fill.enclose L2 points=[5,5;45,5] color=#ff0000")
            .is_err()
    );
    assert!(
        s.run_line("fill.enclose L2 points=[5,5;45,5;45,35] color=#ff0000 tolerance=120")
            .is_err()
    );
    let r = Session::replay(&s.journal_text()).unwrap_or_else(|e| panic!("{}", e.msg));
    let h = |s: &Session| s.document().map(|d| d.content_hash());
    assert_eq!(h(&r), h(&s));
}

#[test]
fn paint_unfilled_area() {
    let mut s = Session::new();
    run(&mut s, SETUP);
    sides(&mut s);
    // A tap of a 40 px brush on the square (10..30): its outline and inside lie wholly inside
    // the stroke's footprint, the white background does not (F-003).
    run(
        &mut s,
        "fill.unfilled L2 color=#ff0000 size=40 samples=[20,20,1,0]",
    );
    assert_eq!(
        q(&mut s, "?layer.pixel L2 x=20 y=20"),
        json!([255, 0, 0, 255])
    );
    assert_eq!(q(&mut s, "?layer.pixel L2 x=10 y=20")[3], 255, "outline");
    assert_eq!(q(&mut s, "?layer.pixel L2 x=5 y=5")[3], 0, "background");
    // A brush narrower than the square fills nothing of it.
    run(
        &mut s,
        "history.undo\nfill.unfilled L2 color=#ff0000 size=10 samples=[15,20,1,0;25,20,1,100000]",
    );
    assert_eq!(q(&mut s, "?layer.pixel L2 x=20 y=20")[3], 0);
    // Errors.
    assert!(
        s.run_line("fill.unfilled L2 color=#ff0000 size=0 samples=[20,20,1,0]")
            .is_err()
    );
    assert!(
        s.run_line("fill.unfilled L2 color=#ff0000 size=20 samples=[20,20,1,0] tolerance=120")
            .is_err()
    );
    let r = Session::replay(&s.journal_text()).unwrap_or_else(|e| panic!("{}", e.msg));
    let h = |s: &Session| s.document().map(|d| d.content_hash());
    assert_eq!(h(&r), h(&s));
}
