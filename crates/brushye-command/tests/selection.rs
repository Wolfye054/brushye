//! Selection commands (D-024, S-001): shapes and modes, Deselect/Reselect, Invert, Expand,
//! Shrink, Move, Fill, Delete, masks from a selection, clipped strokes, the Selection pen,
//! undo and journal replay.

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

fn sel(s: &mut Session, x: u32, y: u32) -> Json {
    q(s, &format!("?selection.pixel x={x} y={y}"))
}

fn fails(s: &mut Session, line: &str) {
    assert!(s.run_line(line).is_err(), "{line} should fail");
}

#[test]
fn shapes_modes_and_history() {
    let mut s = Session::new();
    run(&mut s, "doc.new 64 64\nlayer.new L1");
    let hash = q(&mut s, "?doc.hash");
    assert_eq!(q(&mut s, "?selection")["selected"], false);
    run(&mut s, "select.rect x=10 y=10 w=20 h=10");
    assert_eq!(
        q(&mut s, "?selection"),
        json!({"selected": true, "bounds": [10, 10, 30, 20], "can_reselect": false})
    );
    assert_eq!(
        (sel(&mut s, 10, 10), sel(&mut s, 30, 10)),
        (json!(255), json!(0))
    );
    // Add, then subtract a hole, then intersect.
    run(&mut s, "select.rect x=40 y=40 w=5 h=5 mode=add");
    assert_eq!(sel(&mut s, 42, 42), 255);
    run(&mut s, "select.rect x=12 y=12 w=2 h=2 mode=subtract");
    assert_eq!(
        (sel(&mut s, 12, 12), sel(&mut s, 11, 11)),
        (json!(0), json!(255))
    );
    run(&mut s, "select.rect x=0 y=0 w=64 h=15 mode=intersect");
    assert_eq!(
        (sel(&mut s, 42, 42), sel(&mut s, 11, 11)),
        (json!(0), json!(255))
    );
    // Subtracting everything leaves no selection.
    run(&mut s, "select.rect x=0 y=0 w=64 h=64 mode=subtract");
    assert_eq!(q(&mut s, "?selection")["selected"], false);
    // Selection steps are undoable; without a selection the hash is unchanged.
    run(&mut s, "history.undo");
    assert_eq!(q(&mut s, "?selection")["selected"], true);
    for _ in 0..4 {
        run(&mut s, "history.undo");
    }
    assert_eq!(q(&mut s, "?selection")["selected"], false);
    assert_eq!(q(&mut s, "?doc.hash"), hash);
}

#[test]
fn deselect_reselect_invert_expand_shrink_move() {
    let mut s = Session::new();
    run(&mut s, "doc.new 64 64\nlayer.new L1");
    fails(&mut s, "select.none");
    fails(&mut s, "select.reselect");
    fails(&mut s, "select.invert");
    run(&mut s, "select.rect x=20 y=20 w=10 h=10\nselect.none");
    assert_eq!(
        q(&mut s, "?selection"),
        json!({"selected": false, "bounds": null, "can_reselect": true})
    );
    run(&mut s, "select.reselect");
    assert_eq!(q(&mut s, "?selection")["bounds"], json!([20, 20, 30, 30]));
    run(&mut s, "select.expand w=3");
    assert_eq!(q(&mut s, "?selection")["bounds"], json!([17, 17, 33, 33]));
    run(&mut s, "select.shrink w=5 shape=round");
    assert_eq!(q(&mut s, "?selection")["bounds"], json!([22, 22, 28, 28]));
    run(&mut s, "select.move dx=-20 dy=5");
    assert_eq!(q(&mut s, "?selection")["bounds"], json!([2, 27, 8, 33]));
    run(&mut s, "select.invert");
    assert_eq!(sel(&mut s, 63, 63), 255);
    run(&mut s, "select.all");
    assert_eq!(q(&mut s, "?selection")["bounds"], json!([0, 0, 64, 64]));
    // Shrink at the canvas edge (default) eats the border; without it, it doesn't.
    run(&mut s, "select.shrink w=2");
    assert_eq!(q(&mut s, "?selection")["bounds"], json!([2, 2, 62, 62]));
    run(&mut s, "select.all\nselect.shrink w=2 canvas_edge=false");
    assert_eq!(q(&mut s, "?selection")["bounds"], json!([0, 0, 64, 64]));
}

#[test]
fn fill_delete_and_masks_use_the_selection() {
    let mut s = Session::new();
    run(
        &mut s,
        "doc.new 64 64\nlayer.new L1\nlayer.new L2\nlayer.fill L2 color=#00ff00",
    );
    run(
        &mut s,
        "select.rect x=10 y=10 w=4 h=4\nedit.fill L1 color=#ff0000",
    );
    assert_eq!(
        q(&mut s, "?layer.pixel L1 x=10 y=10"),
        json!([255, 0, 0, 255])
    );
    assert_eq!(q(&mut s, "?layer.pixel L1 x=9 y=10"), json!([0, 0, 0, 0]));
    run(&mut s, "edit.delete L2");
    assert_eq!(q(&mut s, "?layer.pixel L2 x=11 y=11")[3], 0);
    assert_eq!(q(&mut s, "?layer.pixel L2 x=20 y=20")[3], 255);
    // Mask Outside Selection: M = S; Mask Selection: M = 255 − S.
    run(&mut s, "mask.new L2 fill=outside");
    assert_eq!(q(&mut s, "?layer.mask_pixel L2 x=11 y=11"), 255);
    assert_eq!(q(&mut s, "?layer.mask_pixel L2 x=20 y=20"), 0);
    run(&mut s, "mask.delete L2\nmask.new L2 fill=selection");
    assert_eq!(q(&mut s, "?layer.mask_pixel L2 x=11 y=11"), 0);
    assert_eq!(q(&mut s, "?layer.mask_pixel L2 x=20 y=20"), 255);
    // Without a selection, Fill and Delete act on the whole layer.
    run(&mut s, "select.none\nedit.delete L2");
    assert_eq!(q(&mut s, "?layer.pixel L2 x=20 y=20")[3], 0);
    run(&mut s, "edit.fill L1 color=#0000ff");
    assert_eq!(
        q(&mut s, "?layer.pixel L1 x=40 y=40"),
        json!([0, 0, 255, 255])
    );
    fails(&mut s, "edit.fill L1 color=#0000ff80");
}

#[test]
fn strokes_are_clipped_and_the_selection_pen_selects() {
    let mut s = Session::new();
    run(
        &mut s,
        "doc.new 100 40\nlayer.new L1\nselect.rect x=0 y=0 w=50 h=40",
    );
    run(
        &mut s,
        "brush.stroke L1 color=#000000 size=10 samples=[10,20,1,0;90,20,1,100000]",
    );
    assert_eq!(q(&mut s, "?layer.pixel L1 x=40 y=20")[3], 255);
    assert_eq!(q(&mut s, "?layer.pixel L1 x=60 y=20")[3], 0);
    // Selection pen (add) and Erase selection (subtract); the layer is untouched.
    run(&mut s, "select.none");
    let before = q(&mut s, "?doc.hash");
    run(
        &mut s,
        "brush.stroke L1 color=#000000 size=6 aa=none target=selection samples=[60,10,1,0;90,10,1,100000]",
    );
    assert_eq!(
        (sel(&mut s, 75, 10), sel(&mut s, 75, 30)),
        (json!(255), json!(0))
    );
    run(
        &mut s,
        "brush.stroke L1 color=#000000 size=6 aa=none target=selection mode=subtract samples=[75,0,1,0;75,39,1,100000]",
    );
    assert_eq!(
        (sel(&mut s, 75, 10), sel(&mut s, 65, 10)),
        (json!(0), json!(255))
    );
    assert_eq!(q(&mut s, "?layer.pixel L1 x=75 y=10")[3], 0);
    assert_ne!(q(&mut s, "?doc.hash"), before);
    fails(
        &mut s,
        "brush.stroke L1 color=#000000 size=6 mode=add samples=[60,10,1,0]",
    );
}

#[test]
fn journal_replays_selections() {
    let mut s = Session::new();
    run(
        &mut s,
        "doc.new 80 60\nlayer.new L1\nselect.ellipse x=10 y=5 w=50 h=40\n\
         select.polygon points=[5,5;70,10;40,55] aa=none mode=add\nselect.expand w=2 shape=round\n\
         brush.stroke L1 color=#336699 size=20 samples=[0,30,1,0;80,30,1,100000]\nselect.none",
    );
    let r = Session::replay(&s.journal_text()).unwrap_or_else(|e| panic!("{}", e.msg));
    let h = |s: &Session| s.document().map(|d| d.content_hash());
    assert_eq!(h(&r), h(&s));
}
