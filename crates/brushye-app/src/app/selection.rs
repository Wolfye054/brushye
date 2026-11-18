//! The Selection area tool on the canvas (U-016, S-001): shape gestures, moving a selection,
//! lasso and polyline, the marching ants and the Selection Launcher. Every change is a
//! `select.*` / `edit.*` command; the Selection pen and Erase selection run as live strokes with
//! `target=selection` (see `begin_stroke`).

use std::sync::Arc;

use brushye_command::commands::{self, Command};
use brushye_command::core::Document;
use brushye_command::core::brush::Aa;
use brushye_command::core::input::Sample;
use brushye_command::core::selection::SelectMode;
use brushye_command::params::{Points, Samples};
use brushye_command::tools::SubKind;
use eframe::egui::{self, Color32, Pos2, Stroke, pos2, vec2};

use super::{App, Dialog};
use crate::icons;
use crate::ui::tools::ToolId;

/// A selection being drawn or moved.
pub(super) struct Gesture {
    kind: SubKind,
    mode: SelectMode,
    aa: bool,
    /// Canvas position of the press and of the pointer now.
    start: [f64; 2],
    cur: [f64; 2],
    /// Lasso path (canvas).
    points: Vec<[f64; 2]>,
    /// Dragging inside the selection moves it (S-001).
    moving: bool,
    /// When the press happened, and each point's time since then in µs (the Paint unfilled
    /// area stroke's samples).
    t0: std::time::Instant,
    times: Vec<u32>,
    /// Drawn with the pen: Enclose and fill applies Stabilization to pen input only (F-002
    /// run 3).
    pen: bool,
}

/// The selection border, computed once per document version.
#[derive(Default)]
pub(super) struct Ants {
    key: usize,
    /// The border as polylines on the pixel grid (canvas pixels).
    paths: Vec<Vec<[i32; 2]>>,
    bounds: Option<[u32; 4]>,
}

impl Ants {
    fn update(&mut self, doc: &Arc<Document>) {
        let key = Arc::as_ptr(doc) as usize;
        if key == self.key {
            return;
        }
        self.key = key;
        self.paths.clear();
        self.bounds = None;
        let Some(s) = &doc.selection else { return };
        let (w, h) = (doc.width as usize, doc.height as usize);
        let v = s.to_dense(doc.width, doc.height);
        self.bounds = brushye_command::core::selection::dense_bounds(&v, doc.width, doc.height);
        // The border runs between pixels on either side of half selected: unit edges of the
        // pixel grid, chained into polylines.
        let inside = |x: isize, y: isize| {
            x >= 0
                && y >= 0
                && (x as usize) < w
                && (y as usize) < h
                && v[y as usize * w + x as usize] >= 128
        };
        let mut segs: Vec<[[i32; 2]; 2]> = Vec::new();
        for y in 0..=h as isize {
            for x in 0..w as isize {
                if inside(x, y) != inside(x, y - 1) {
                    segs.push([[x as i32, y as i32], [x as i32 + 1, y as i32]]);
                }
            }
        }
        for x in 0..=w as isize {
            for y in 0..h as isize {
                if inside(x, y) != inside(x - 1, y) {
                    segs.push([[x as i32, y as i32], [x as i32, y as i32 + 1]]);
                }
            }
        }
        self.paths = chain(&segs);
    }
}

/// Join unit edges that share end points into polylines (so the dashes run on along the
/// border), keeping only the corners.
fn chain(segs: &[[[i32; 2]; 2]]) -> Vec<Vec<[i32; 2]>> {
    use std::collections::HashMap;
    let mut at: HashMap<[i32; 2], Vec<usize>> = HashMap::new();
    for (i, s) in segs.iter().enumerate() {
        at.entry(s[0]).or_default().push(i);
        at.entry(s[1]).or_default().push(i);
    }
    let mut used = vec![false; segs.len()];
    let mut out = Vec::new();
    for start in 0..segs.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let mut path = vec![segs[start][0], segs[start][1]];
        loop {
            let end = path[path.len() - 1];
            let next = at
                .get(&end)
                .and_then(|v| v.iter().copied().find(|&j| !used[j]));
            let Some(j) = next else { break };
            used[j] = true;
            let s = segs[j];
            let p = if s[0] == end { s[1] } else { s[0] };
            // Drop the middle point of a straight run.
            let n = path.len();
            if n >= 2 {
                let (a, b) = (path[n - 2], path[n - 1]);
                if (b[0] - a[0]) * (p[1] - b[1]) == (b[1] - a[1]) * (p[0] - b[0]) {
                    path[n - 1] = p;
                    continue;
                }
            }
            path.push(p);
        }
        out.push(path);
    }
    out
}

impl App {
    /// The current Selection area sub tool: kind, mode and whether it anti-aliases.
    fn selection_subtool(&self) -> Option<(SubKind, SelectMode, bool)> {
        if self.ui.tools.tool != ToolId::Selection {
            return None;
        }
        let s = self.ui.tools.sets.get(&ToolId::Selection)?.current()?;
        Some((
            s.kind,
            s.preset.mode.unwrap_or_default(),
            s.preset.aa != Some(Aa::None),
        ))
    }

    /// The Selection pen / Erase selection mode when that sub tool is current.
    pub(super) fn selection_brush_mode(&self) -> Option<SelectMode> {
        let (kind, mode, _) = self.selection_subtool()?;
        kind.is_selection_brush().then_some(mode)
    }

    /// A press with the Selection area tool (not its brushes). Returns whether it was used.
    pub(super) fn selection_press(&mut self, pos: Pos2) -> bool {
        let Some((kind, mode, aa)) = self.selection_subtool() else {
            return false;
        };
        if kind.is_selection_brush() {
            return false;
        }
        let Some(doc) = self.doc() else { return true };
        let c = self.to_canvas(pos);
        match kind {
            SubKind::Polyline => {
                // Click to add corners; click the first corner, double-click or Enter to close.
                let k = f64::from(self.ppp);
                let near = |p: [f64; 2]| {
                    let s = self.view.to_screen(p);
                    let q = self.screen_px(pos);
                    (s[0] - q[0]).hypot(s[1] - q[1]) <= 6.0 * k
                };
                let close = self.polyline.len() >= 3
                    && (near(self.polyline[0]) || self.polyline.last().is_some_and(|&p| near(p)));
                if close {
                    self.finish_polyline(mode, aa);
                } else {
                    self.polyline.push(c);
                }
            }
            SubKind::ShrinkSelection => {
                self.status = "Shrink selection is not available yet".into();
            }
            _ => {
                let inside = c[0] >= 0.0
                    && c[1] >= 0.0
                    && (c[0] as u32) < doc.width
                    && (c[1] as u32) < doc.height
                    && doc
                        .selection
                        .as_ref()
                        .is_some_and(|s| s.value(c[0] as u32, c[1] as u32) > 0);
                self.sel = Some(Gesture {
                    kind,
                    mode,
                    aa,
                    start: c,
                    cur: c,
                    points: vec![c],
                    moving: mode == SelectMode::New && inside,
                    t0: std::time::Instant::now(),
                    times: vec![0],
                    pen: self.pen.near,
                });
            }
        }
        true
    }

    pub(super) fn selection_motion(&mut self, pos: Pos2) {
        let c = self.to_canvas(pos);
        if let Some(g) = self.sel.as_mut() {
            g.cur = c;
            if matches!(
                g.kind,
                SubKind::Lasso | SubKind::FillEnclose | SubKind::FillUnfilled
            ) && g
                .points
                .last()
                .is_none_or(|p| (p[0] - c[0]).hypot(p[1] - c[1]) >= 1.0)
            {
                g.points.push(c);
                g.times
                    .push(g.t0.elapsed().as_micros().min(u128::from(u32::MAX)) as u32);
            }
        }
    }

    pub(super) fn selection_release(&mut self) {
        let Some(g) = self.sel.take() else { return };
        let round = |v: f64| (v * 100.0).round() / 100.0;
        if g.moving {
            let (dx, dy) = (
                (g.cur[0] - g.start[0]).round() as i64,
                (g.cur[1] - g.start[1]).round() as i64,
            );
            if dx != 0 || dy != 0 {
                self.exec(Command::SelectMove(commands::SelectMove { dx, dy }));
            }
            return;
        }
        let mode = Some(g.mode);
        let aa = Some(if g.aa { Aa::Middle } else { Aa::None });
        match g.kind {
            SubKind::Rectangle | SubKind::Ellipse => {
                // The box of whole pixels between the press and the release (S-001).
                let (x0, x1) = (
                    g.start[0].min(g.cur[0]).floor(),
                    g.start[0].max(g.cur[0]).floor(),
                );
                let (y0, y1) = (
                    g.start[1].min(g.cur[1]).floor(),
                    g.start[1].max(g.cur[1]).floor(),
                );
                if x1 <= x0 || y1 <= y0 {
                    // A click without a drag outside the selection deselects (New mode,
                    // S-001 run 3); inside it, it keeps the selection (handled as a move).
                    if g.mode == SelectMode::New {
                        self.select_none();
                    }
                    return;
                }
                let cmd = if g.kind == SubKind::Rectangle {
                    Command::SelectRect(commands::SelectRect {
                        x: x0 as i64,
                        y: y0 as i64,
                        w: (x1 - x0) as u32,
                        h: (y1 - y0) as u32,
                        mode,
                    })
                } else {
                    Command::SelectEllipse(commands::SelectEllipse {
                        x: x0,
                        y: y0,
                        w: x1 - x0,
                        h: y1 - y0,
                        aa,
                        mode,
                    })
                };
                self.exec(cmd);
            }
            SubKind::FillEnclose if g.points.len() >= 3 => {
                let points = g
                    .points
                    .iter()
                    .map(|p| [round(p[0]), round(p[1])])
                    .collect();
                self.enclose_fill(Points(points), g.pen);
            }
            SubKind::FillUnfilled => {
                let samples = g
                    .points
                    .iter()
                    .zip(&g.times)
                    .map(|(p, &t_us)| Sample {
                        x: round(p[0]),
                        y: round(p[1]),
                        t_us,
                        pressure: None,
                        tilt: None,
                        twist: None,
                        eraser: false,
                        barrel: [false; 2],
                    })
                    .collect();
                self.unfilled_fill(Samples(samples));
            }
            SubKind::Lasso if g.points.len() >= 3 => {
                let points = g
                    .points
                    .iter()
                    .map(|p| [round(p[0]), round(p[1])])
                    .collect();
                self.exec(Command::SelectPolygon(commands::SelectPolygon {
                    points: Points(points),
                    aa,
                    mode,
                }));
            }
            _ => {}
        }
    }

    fn finish_polyline(&mut self, mode: SelectMode, aa: bool) {
        let pts = std::mem::take(&mut self.polyline);
        if pts.len() >= 3 {
            let round = |v: f64| (v * 100.0).round() / 100.0;
            self.exec(Command::SelectPolygon(commands::SelectPolygon {
                points: Points(pts.iter().map(|p| [round(p[0]), round(p[1])]).collect()),
                aa: Some(if aa { Aa::Middle } else { Aa::None }),
                mode: Some(mode),
            }));
        }
    }

    /// Enter closes a polyline, Escape drops it.
    pub(super) fn selection_keys(&mut self, ui: &egui::Ui) {
        if self.polyline.is_empty() {
            return;
        }
        let (enter, esc) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::Escape),
            )
        });
        if esc
            || self
                .selection_subtool()
                .is_none_or(|s| s.0 != SubKind::Polyline)
        {
            self.polyline.clear();
        } else if enter && let Some((_, mode, aa)) = self.selection_subtool() {
            self.finish_polyline(mode, aa);
        }
    }

    fn screen(&self, c: [f64; 2]) -> Pos2 {
        let k = f64::from(self.ppp);
        let s = self.view.to_screen(c);
        pos2((s[0] / k) as f32, (s[1] / k) as f32)
    }

    /// Marching ants, the shape being drawn and the Selection Launcher.
    pub(super) fn selection_overlay(&mut self, ui: &egui::Ui, doc: &Arc<Document>) {
        self.ants.update(doc);
        let painter = ui.painter_at(self.canvas_rect);
        let time = ui.input(|i| i.time);
        let offset = ((time * 8.0) % 8.0) as f32;
        let ants = |painter: &egui::Painter, pts: &[Pos2]| {
            painter.add(egui::Shape::line(
                pts.to_vec(),
                Stroke::new(1.0, Color32::WHITE),
            ));
            painter.extend(egui::Shape::dashed_line_with_offset(
                pts,
                Stroke::new(1.0, Color32::BLACK),
                &[4.0],
                &[4.0],
                offset,
            ));
        };
        let shift = match &self.sel {
            Some(g) if g.moving => [
                (g.cur[0] - g.start[0]).round(),
                (g.cur[1] - g.start[1]).round(),
            ],
            _ => [0.0, 0.0],
        };
        if !self.ants.paths.is_empty() {
            for path in &self.ants.paths {
                let pts: Vec<Pos2> = path
                    .iter()
                    .map(|p| self.screen([f64::from(p[0]) + shift[0], f64::from(p[1]) + shift[1]]))
                    .collect();
                ants(&painter, &pts);
            }
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(120));
        }
        // The shape being drawn.
        if let Some(g) = &self.sel
            && !g.moving
        {
            match g.kind {
                SubKind::Rectangle | SubKind::Ellipse => {
                    let (x0, x1) = (
                        g.start[0].min(g.cur[0]).floor(),
                        g.start[0].max(g.cur[0]).floor(),
                    );
                    let (y0, y1) = (
                        g.start[1].min(g.cur[1]).floor(),
                        g.start[1].max(g.cur[1]).floor(),
                    );
                    let pts: Vec<Pos2> = if g.kind == SubKind::Rectangle {
                        [[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]]
                            .iter()
                            .map(|&p| self.screen(p))
                            .collect()
                    } else {
                        let (cx, cy, rx, ry) = (
                            (x0 + x1) / 2.0,
                            (y0 + y1) / 2.0,
                            (x1 - x0) / 2.0,
                            (y1 - y0) / 2.0,
                        );
                        (0..=64)
                            .map(|i| {
                                let t = f64::from(i) / 64.0 * std::f64::consts::TAU;
                                self.screen([cx + rx * t.cos(), cy + ry * t.sin()])
                            })
                            .collect()
                    };
                    ants(&painter, &pts);
                }
                SubKind::Lasso => {
                    let pts: Vec<Pos2> = g.points.iter().map(|&p| self.screen(p)).collect();
                    ants(&painter, &pts);
                }
                SubKind::FillEnclose => {
                    // The lasso as it will fill: the events' B-spline, stabilized for the pen
                    // (F-002 run 3).
                    let stab = if g.pen {
                        self.ui
                            .tools
                            .sets
                            .get(&ToolId::Fill)
                            .and_then(|s| s.current())
                            .and_then(|s| s.preset.stabilization)
                            .unwrap_or(0)
                    } else {
                        0
                    };
                    let pts: Vec<Pos2> = brushye_command::core::fill::lasso_live(&g.points, stab)
                        .iter()
                        .map(|&p| self.screen(p))
                        .collect();
                    ants(&painter, &pts);
                }
                SubKind::FillUnfilled => {
                    // The stroke being drawn, as wide as the brush.
                    let size = self
                        .ui
                        .tools
                        .sets
                        .get(&ToolId::Fill)
                        .and_then(|s| s.current())
                        .map_or(20.0, |s| s.preset.size);
                    let width = (size * self.view.zoom / f64::from(self.ppp)) as f32;
                    let color = Color32::from_rgba_unmultiplied(255, 40, 40, 90);
                    let pts: Vec<Pos2> = g.points.iter().map(|&p| self.screen(p)).collect();
                    if pts.len() == 1 {
                        painter.circle_filled(pts[0], width / 2.0, color);
                    } else {
                        painter.add(egui::Shape::line(pts, Stroke::new(width.max(1.0), color)));
                    }
                }
                _ => {}
            }
        }
        if !self.polyline.is_empty() {
            let mut pts: Vec<Pos2> = self.polyline.iter().map(|&p| self.screen(p)).collect();
            if let Some(c) = self.cursor {
                pts.push(self.screen(c));
            }
            ants(&painter, &pts);
            ui.ctx().request_repaint();
        }
        if self.sel.is_some() {
            ui.ctx().request_repaint();
        }
        if self.sel.is_none()
            && self.live.is_none()
            && let Some(b) = self.ants.bounds
        {
            self.launcher(ui, b);
        }
    }

    /// The Selection Launcher (U-016): a bar below the selection's bounding box, left-aligned
    /// with it. Buttons Brushye doesn't have yet are disabled.
    fn launcher(&mut self, ui: &egui::Ui, b: [u32; 4]) {
        let corners = [[b[0], b[1]], [b[2], b[1]], [b[0], b[3]], [b[2], b[3]]]
            .map(|[x, y]| self.screen([f64::from(x), f64::from(y)]));
        let left = corners.iter().map(|p| p.x).fold(f32::MAX, f32::min);
        let bottom = corners.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        let r = self.canvas_rect;
        let size = vec2(13.0 * 24.0 + 10.0, 26.0);
        let at = pos2(
            left.clamp(r.left(), (r.right() - size.x).max(r.left())),
            (bottom + 6.0).clamp(r.top(), (r.bottom() - size.y).max(r.top())),
        );
        let c = self.ui.colors();
        let can_edit = self.can_edit_layer() && !self.active_is_folder();
        let mut clicked = None;
        egui::Area::new(ui.scope_id().with("selection-launcher"))
            .fixed_pos(at)
            .order(egui::Order::Middle)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(Color32::from_gray(48))
                    .inner_margin(2.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            let buttons: [(&str, &str, bool, u8); 12] = [
                                (icons::SELECTION_SLASH, "Deselect", true, 1),
                                (icons::CROP, "Crop", false, 0),
                                (icons::SELECTION_INVERSE, "Invert selected area", true, 2),
                                (icons::ARROWS_OUT_SIMPLE, "Expand selected area", true, 3),
                                (icons::ARROWS_IN_SIMPLE, "Shrink selected area", true, 4),
                                (icons::TRASH, "Delete", can_edit, 5),
                                (
                                    icons::SELECTION_BACKGROUND,
                                    "Delete outside selection",
                                    false,
                                    0,
                                ),
                                (icons::SCISSORS, "Cut and paste", false, 0),
                                (icons::COPY, "Copy and paste", false, 0),
                                (icons::CORNERS_OUT, "Scale/Rotate", false, 0),
                                (icons::PAINT_BUCKET, "Fill", can_edit, 6),
                                (icons::CHECKERBOARD, "New tone", false, 0),
                            ];
                            for (icon, tip, enabled, id) in buttons {
                                if crate::ui::widgets::icon_button(
                                    ui,
                                    c,
                                    icon,
                                    tip,
                                    vec2(24.0, 22.0),
                                    enabled,
                                    false,
                                )
                                .clicked()
                                {
                                    clicked = Some(id);
                                }
                            }
                            ui.separator();
                            crate::ui::widgets::icon_button(
                                ui,
                                c,
                                icons::GEAR,
                                "Selection launcher settings",
                                vec2(24.0, 22.0),
                                false,
                                false,
                            );
                        });
                    });
            });
        match clicked {
            Some(1) => self.select_none(),
            Some(2) => self.select_invert(),
            Some(3) => {
                self.dialog = Some(Dialog::Expand {
                    width: 1,
                    round: false,
                })
            }
            Some(4) => {
                self.dialog = Some(Dialog::Shrink {
                    width: 1,
                    round: false,
                    edge: true,
                });
            }
            Some(5) => self.clear_layer(),
            Some(6) => self.fill_layer(),
            _ => {}
        }
    }

    // ------------------------------------------------------------ Select menu

    pub(crate) fn has_selection(&self) -> bool {
        self.doc().is_some_and(|d| d.selection.is_some())
    }

    pub(crate) fn can_reselect(&self) -> bool {
        self.doc().is_some_and(|d| d.last_selection.is_some())
    }

    pub(crate) fn select_all(&mut self) {
        self.exec(Command::SelectAll(commands::SelectAll {}));
    }

    pub(crate) fn select_none(&mut self) {
        if self.has_selection() {
            self.exec(Command::SelectNone(commands::SelectNone {}));
        }
    }

    pub(crate) fn reselect(&mut self) {
        if self.can_reselect() {
            self.exec(Command::SelectReselect(commands::SelectReselect {}));
        }
    }

    pub(crate) fn select_invert(&mut self) {
        if self.has_selection() {
            self.exec(Command::SelectInvert(commands::SelectInvert {}));
        }
    }

    pub(crate) fn select_expand(&mut self, w: u32, round: bool) {
        self.exec(Command::SelectExpand(commands::SelectExpand {
            w,
            shape: Some(shape(round)),
        }));
    }

    pub(crate) fn select_blur(&mut self, strength: f64) {
        self.exec(Command::SelectBlur(commands::SelectBlur { strength }));
    }

    /// Layer > Selection from Layer on the active layer.
    pub(crate) fn selection_from_layer(
        &mut self,
        op: brushye_command::core::selection::LayerSelOp,
    ) {
        if let Some(layer) = self.session.active_layer() {
            self.exec(Command::SelectFromLayer(commands::SelectFromLayer {
                layer,
                op: Some(op),
            }));
        }
    }

    pub(crate) fn select_shrink(&mut self, w: u32, round: bool, edge: bool) {
        self.exec(Command::SelectShrink(commands::SelectShrink {
            w,
            shape: Some(shape(round)),
            canvas_edge: Some(edge),
        }));
    }
}

fn shape(round: bool) -> commands::MorphShape {
    if round {
        commands::MorphShape::Round
    } else {
        commands::MorphShape::Square
    }
}

impl App {
    /// While the Selection pen or Erase selection draws: its path as a translucent band (the
    /// selection itself changes when the stroke ends).
    pub(super) fn selection_stroke_preview(&self, ui: &egui::Ui) {
        let Some(l) = &self.live else { return };
        let Some(mode) = l.selection else { return };
        let pts: Vec<Pos2> = l
            .stroke
            .samples()
            .iter()
            .map(|s| self.screen([s.x, s.y]))
            .collect();
        let width = (l.preset.size * self.view.zoom / f64::from(self.ppp)) as f32;
        let color = if mode == SelectMode::Subtract {
            Color32::from_rgba_unmultiplied(40, 40, 255, 90)
        } else {
            Color32::from_rgba_unmultiplied(255, 40, 40, 90)
        };
        let painter = ui.painter_at(self.canvas_rect);
        if pts.len() == 1 {
            painter.circle_filled(pts[0], width / 2.0, color);
        } else {
            painter.add(egui::Shape::line(pts, Stroke::new(width.max(1.0), color)));
        }
    }
}

impl App {
    /// The end of an Enclose and fill lasso: one `fill.enclose` with the sub tool's settings.
    fn enclose_fill(&mut self, points: Points, pen: bool) {
        let Some(sub) = self
            .ui
            .tools
            .sets
            .get(&ToolId::Fill)
            .and_then(|s| s.current())
            .cloned()
        else {
            return;
        };
        let Some(layer) = self.session.active_layer() else {
            self.status = "Select a layer".into();
            return;
        };
        let Some(rgb) = self.ui.colour.drawing() else {
            self.status = "Fill with the transparent colour is not available yet".into();
            return;
        };
        let p = &sub.preset;
        let cmd = Command::FillEnclose(commands::FillEnclose {
            layer,
            points,
            color: brushye_command::core::Rgba8([rgb[0], rgb[1], rgb[2], 255]),
            // CSP stabilizes the lasso for pen input only (F-002 run 3).
            stabilization: if pen { p.stabilization } else { None },
            target: p.target,
            tolerance: p.tolerance,
            aa: Some(p.aa.is_some_and(|a| a != Aa::None)),
            // Area scaling off leaves no value in the preset; the command's default is 10.
            scale: Some(p.scale.unwrap_or(0)),
            scale_mode: p.scale_mode,
            opacity: p.opacity,
            refer: Some(commands::FillRef::All),
        });
        if self.exec(cmd) {
            self.ui.colour.used();
        }
    }

    /// The end of a Paint unfilled area stroke: one `fill.unfilled` with the sub tool's
    /// settings.
    fn unfilled_fill(&mut self, samples: Samples) {
        let Some(sub) = self
            .ui
            .tools
            .sets
            .get(&ToolId::Fill)
            .and_then(|s| s.current())
            .cloned()
        else {
            return;
        };
        let Some(layer) = self.session.active_layer() else {
            self.status = "Select a layer".into();
            return;
        };
        let Some(rgb) = self.ui.colour.drawing() else {
            self.status = "Fill with the transparent colour is not available yet".into();
            return;
        };
        let p = &sub.preset;
        let cmd = Command::FillUnfilled(commands::FillUnfilled {
            layer,
            color: brushye_command::core::Rgba8([rgb[0], rgb[1], rgb[2], 255]),
            size: p.size,
            samples,
            gap: p.gap,
            target: p.target,
            tolerance: p.tolerance,
            aa: Some(p.aa.is_some_and(|a| a != Aa::None)),
            scale: Some(p.scale.unwrap_or(0)),
            scale_mode: p.scale_mode,
            opacity: p.opacity,
            refer: Some(commands::FillRef::All),
        });
        if self.exec(cmd) {
            self.ui.colour.used();
        }
    }

    /// A click with the Fill or Auto select tool (F-001): one `fill.area` or `select.auto`.
    pub(super) fn fill_press(&mut self, pos: Pos2) {
        let tool = self.ui.tools.tool;
        let Some(sub) = self
            .ui
            .tools
            .sets
            .get(&tool)
            .and_then(|s| s.current())
            .cloned()
        else {
            return;
        };
        if matches!(sub.kind, SubKind::FillEnclose | SubKind::FillUnfilled) {
            // Enclose and fill: draw a lasso (F-002); Paint unfilled area: a brush stroke
            // (F-003). The release fills.
            let c = self.to_canvas(pos);
            self.sel = Some(Gesture {
                kind: sub.kind,
                mode: SelectMode::New,
                aa: false,
                start: c,
                cur: c,
                points: vec![c],
                moving: false,
                t0: std::time::Instant::now(),
                times: vec![0],
                pen: self.pen.near,
            });
            return;
        }
        let Some(reads_all) = sub.kind.fill_reads_all() else {
            self.status = format!("“{}” is not available yet", sub.name);
            return;
        };
        let Some(doc) = self.doc() else { return };
        let c = self.to_canvas(pos);
        if c[0] < 0.0 || c[1] < 0.0 || c[0] >= f64::from(doc.width) || c[1] >= f64::from(doc.height)
        {
            return;
        }
        let (x, y) = (c[0] as u32, c[1] as u32);
        let Some(layer) = self.session.active_layer() else {
            self.status = "Select a layer".into();
            return;
        };
        let p = &sub.preset;
        let refer = Some(if reads_all {
            commands::FillRef::All
        } else {
            commands::FillRef::Layer
        });
        let aa = Some(p.aa != Some(Aa::None));
        let cmd = if tool == ToolId::Fill {
            let Some(rgb) = self.ui.colour.drawing() else {
                self.status = "Fill with the transparent colour is not available yet".into();
                return;
            };
            Command::FillArea(commands::FillArea {
                layer,
                x,
                y,
                color: brushye_command::core::Rgba8([rgb[0], rgb[1], rgb[2], 255]),
                tolerance: p.tolerance,
                connected: p.connected,
                close_gap: p.close_gap,
                aa,
                scale: p.scale,
                scale_mode: p.scale_mode,
                opacity: p.opacity,
                refer,
            })
        } else {
            Command::SelectAuto(commands::SelectAuto {
                layer,
                x,
                y,
                tolerance: p.tolerance,
                connected: p.connected,
                close_gap: p.close_gap,
                aa,
                scale: p.scale,
                scale_mode: p.scale_mode,
                refer,
                mode: p.mode,
            })
        };
        if self.exec(cmd) && tool == ToolId::Fill {
            self.ui.colour.used();
        }
    }
}
