//! The executor: the only place session state changes (D-010/D-011), with snapshot history
//! (D-013) and the session journal.
//!
//! - History is a list of immutable document versions and a position; undo/redo move the
//!   position. An undoable command clones the current version (sharing unchanged layers and
//!   tiles), edits it, and pushes it, discarding any redo branch.
//! - Consecutive changes of the same kind merge into one step (CSP, spec H-001). Live, the
//!   executor decides; the journal records the decision as `@n+`, and replay applies exactly
//!   the recorded grouping, never re-deciding.
//! - Layer visibility is journaled but not undoable (CSP): it is applied to every version in
//!   history, so undo/redo never change it.
//! - A command that fails changes nothing and is not journaled.

use std::sync::Arc;

use brushye_core::brush::{Aa, BrushSettings, Curve, InkMode, ink};
use brushye_core::document::DocError;
use brushye_core::selection::{self as sel, SelectMode, Shape};
use brushye_core::{BlendMode, Document, Grid8, Layer, LayerId, LayerKind, Rgba8, TILE_SIZE};
use serde_json::{Value as Json, json};

use crate::commands::{self, Class, Command, MaskFill, MorphShape, Paper, Place, Query, Target};
use crate::params::{Arg, value_text};
use crate::syntax::{Line, RawLine, parse_line, print_line};

/// Journal format version (docs/command-format.md).
pub const JOURNAL_FORMAT: u32 = 1;
/// Default undo limit (CSP's maximum "Undo levels", spec H-001).
pub const DEFAULT_UNDO_LIMIT: usize = 200;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecError(pub String);

impl std::fmt::Display for ExecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ExecError {}

impl From<DocError> for ExecError {
    fn from(e: DocError) -> Self {
        ExecError(e.to_string())
    }
}

impl From<String> for ExecError {
    fn from(e: String) -> Self {
        ExecError(e)
    }
}

/// An error at a line of a script or journal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineError {
    pub line: usize,
    pub msg: String,
}

impl std::fmt::Display for LineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.msg)
    }
}

impl std::error::Error for LineError {}

#[derive(Clone, Debug)]
struct Step {
    label: &'static str,
    doc: Arc<Document>,
}

/// What can still merge into the current undo step.
#[derive(Clone, Debug, PartialEq, Eq)]
struct MergeKey {
    cmd: &'static str,
    layer: LayerId,
    keys: Vec<&'static str>,
}

fn merge_key(cmd: &Command) -> Option<MergeKey> {
    match cmd {
        Command::LayerSet(p) => {
            let mut keys = Vec::new();
            if p.name.is_some() {
                keys.push("name");
            }
            if p.opacity.is_some() {
                keys.push("opacity");
            }
            if p.blend.is_some() {
                keys.push("blend");
            }
            if p.locked.is_some() {
                keys.push("locked");
            }
            if p.lock_alpha.is_some() {
                keys.push("lock_alpha");
            }
            if p.clip.is_some() {
                keys.push("clip");
            }
            Some(MergeKey {
                cmd: "layer.set",
                layer: p.layer,
                keys,
            })
        }
        // Consecutive changes of the same mask setting merge (H-002).
        Command::MaskSet(p) => {
            let mut keys = Vec::new();
            if p.enabled.is_some() {
                keys.push("enabled");
            }
            if p.linked.is_some() {
                keys.push("linked");
            }
            if p.show_area.is_some() {
                keys.push("show_area");
            }
            Some(MergeKey {
                cmd: "mask.set",
                layer: p.layer,
                keys,
            })
        }
        _ => None,
    }
}

/// One editing session: current document, undo history, active layer and journal.
#[derive(Clone, Debug)]
pub struct Session {
    steps: Vec<Step>,
    pos: usize,
    active: Option<LayerId>,
    /// The selected layer's mask, not its pixels, is the paint target (app state).
    active_mask: bool,
    journal: Vec<RawLine>,
    undo_limit: usize,
    merge_open: Option<MergeKey>,
    /// Global pen pressure curve (application setting, `input.pressure_curve`).
    pressure_curve: Curve,
    /// Content hash of the document this session was opened from (`None`: started by
    /// `doc.new`). Recorded in the journal header so replay can check it gets the same base.
    base: Option<u64>,
    /// Tools, sub tools and colour chips (D-018); kept across `doc.new`.
    tools: crate::tools::ToolState,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    pub fn new() -> Self {
        Self {
            steps: Vec::new(),
            pos: 0,
            active: None,
            active_mask: false,
            journal: Vec::new(),
            undo_limit: DEFAULT_UNDO_LIMIT,
            merge_open: None,
            pressure_curve: Curve::linear(),
            base: None,
            tools: crate::tools::ToolState::default(),
        }
    }

    /// A session that starts from an existing document (e.g. one opened from a `.brsh` file).
    /// History starts at that document; the journal header records its content hash.
    pub fn from_document(doc: Document, active: Option<LayerId>) -> Self {
        let base = doc.content_hash();
        let mut s = Self::new();
        s.steps = vec![Step {
            label: "open",
            doc: Arc::new(doc),
        }];
        s.active = active;
        s.base = Some(base);
        s.fix_active();
        s
    }

    /// The global pen pressure curve (`input.pressure_curve`).
    pub fn pressure_curve(&self) -> &Curve {
        &self.pressure_curve
    }

    /// Tools, sub tools and colour chips (`tool.*`, `subtool.*`, `color.*`).
    pub fn tools(&self) -> &crate::tools::ToolState {
        &self.tools
    }

    /// Restore saved tool state (user settings at start-up, or carried to a new session).
    pub fn set_tools(&mut self, tools: crate::tools::ToolState) {
        let generation = self.tools.generation.max(tools.generation) + 1;
        self.tools = tools;
        self.tools.generation = generation;
    }

    /// Number of journaled commands (cheap; for "unsaved changes" tracking).
    pub fn journal_len(&self) -> usize {
        self.journal.len()
    }

    /// Content hash of the base document, if the session was opened from one.
    pub fn base_hash(&self) -> Option<u64> {
        self.base
    }

    /// Undo limit in steps, 1..=200 like CSP (values are clamped).
    pub fn set_undo_limit(&mut self, n: usize) {
        self.undo_limit = n.clamp(1, DEFAULT_UNDO_LIMIT);
        self.enforce_limit();
    }

    /// The current document version (immutable; share it freely).
    pub fn document(&self) -> Option<Arc<Document>> {
        self.steps.get(self.pos).map(|s| s.doc.clone())
    }

    pub fn active_layer(&self) -> Option<LayerId> {
        self.active
    }

    /// Whether the selected layer's mask is the paint target (`layer.select … target=mask`).
    pub fn mask_selected(&self) -> bool {
        self.active_mask
    }

    fn doc(&self) -> Result<&Arc<Document>, ExecError> {
        self.steps
            .get(self.pos)
            .map(|s| &s.doc)
            .ok_or_else(|| ExecError("no document (run doc.new first)".into()))
    }

    /// Execute a command live: the executor decides merging. On success it is journaled.
    pub fn execute(&mut self, cmd: Command) -> Result<(), ExecError> {
        if cmd.class() == Class::Tool {
            // Not journaled: tool state never changes the document (D-018).
            return self.tools.apply(&cmd).map_err(ExecError);
        }
        let merge = cmd.class() == Class::Undoable
            && self.merge_open.is_some()
            && self.merge_open == merge_key(&cmd)
            && self.pos + 1 == self.steps.len();
        self.apply(&cmd, merge)?;
        let mut raw = cmd.to_raw();
        raw.seq = Some(self.journal.len() as u64 + 1);
        raw.merge = merge;
        self.journal.push(raw);
        Ok(())
    }

    fn apply(&mut self, cmd: &Command, merge: bool) -> Result<(), ExecError> {
        match cmd.class() {
            Class::Session => self.apply_session(cmd),
            Class::Undoable => self.apply_undoable(cmd, merge),
            Class::AppState => self.apply_app_state(cmd),
            Class::Tool => Err(ExecError(format!(
                "{} is a tool command; tool commands aren't journaled",
                cmd.name()
            ))),
            Class::History => {
                match cmd {
                    Command::HistoryUndo(_) => self.pos = self.pos.saturating_sub(1),
                    Command::HistoryRedo(_) => {
                        if self.pos + 1 < self.steps.len() {
                            self.pos += 1;
                        }
                    }
                    _ => {
                        return Err(ExecError(format!(
                            "{} is not a history command",
                            cmd.name()
                        )));
                    }
                }
                self.merge_open = None;
                self.fix_active();
                Ok(())
            }
        }
    }

    fn apply_session(&mut self, cmd: &Command) -> Result<(), ExecError> {
        let Command::DocNew(p) = cmd else {
            return Err(ExecError(format!(
                "{} is not a session command",
                cmd.name()
            )));
        };
        let paper = match p.paper {
            None => Some(Rgba8::WHITE),
            Some(Paper::None) => None,
            Some(Paper::Color(c)) => Some(c),
        };
        let doc = Document::new(p.width, p.height, p.dpi.unwrap_or(72), paper)?;
        self.steps = vec![Step {
            label: "doc.new",
            doc: Arc::new(doc),
        }];
        self.pos = 0;
        self.active = None;
        self.merge_open = None;
        Ok(())
    }

    fn apply_undoable(&mut self, cmd: &Command, merge: bool) -> Result<(), ExecError> {
        // After Ungroup, CSP's edit target is the folder's topmost child (U-014).
        // After a delete, CSP selects the layer directly below the deleted one (U-014 run 2).
        let below_deleted = match cmd {
            Command::LayerDelete(p) => {
                let d = self.doc()?;
                d.subtree(p.layer)
                    .ok()
                    .and_then(|r| r.start.checked_sub(1))
                    .map(|i| d.layers()[i].id)
            }
            _ => None,
        };
        let ungrouped_top = match cmd {
            Command::LayerUngroup(p) => self
                .doc()?
                .layers()
                .iter()
                .rev()
                .find(|l| l.parent == Some(p.folder))
                .map(|l| l.id),
            _ => None,
        };
        let mut d = Document::clone(self.doc()?);
        edit(&mut d, cmd, &self.pressure_curve)?;
        let doc = Arc::new(d);
        if merge {
            if self.pos == 0 {
                return Err(ExecError("nothing to merge into".into()));
            }
            self.steps.truncate(self.pos + 1);
            if let Some(s) = self.steps.get_mut(self.pos) {
                s.doc = doc;
            }
        } else {
            self.steps.truncate(self.pos + 1);
            self.steps.push(Step {
                label: cmd.name(),
                doc,
            });
            self.pos += 1;
            self.enforce_limit();
        }
        self.merge_open = merge_key(cmd);
        match cmd {
            Command::LayerNew(p) => self.active = Some(p.layer),
            // Ctrl+G selects the new folder (U-014).
            Command::LayerGroup(p) => self.active = Some(p.folder),
            Command::LayerUngroup(_) => self.active = ungrouped_top.or(self.active),
            Command::LayerDelete(p) if self.active == Some(p.layer) || self.active.is_none() => {
                self.active = below_deleted;
            }
            // After Create layer mask the mask is the paint target (U-014).
            Command::MaskNew(p) => {
                self.active = Some(p.layer);
                self.active_mask = true;
            }
            _ => {}
        }
        self.fix_active();
        Ok(())
    }

    fn apply_app_state(&mut self, cmd: &Command) -> Result<(), ExecError> {
        match cmd {
            Command::LayerShow(p) => {
                self.doc()?
                    .layer(p.layer)
                    .ok_or(DocError::NoSuchLayer(p.layer))?;
                // Not undoable: apply to every version that has this layer.
                for s in &mut self.steps {
                    if s.doc.layer(p.layer).is_some_and(|l| l.visible != p.visible) {
                        let mut d = Document::clone(&s.doc);
                        d.layer_mut(p.layer)?.visible = p.visible;
                        s.doc = Arc::new(d);
                    }
                }
                Ok(())
            }
            Command::LayerExpand(p) => {
                let d = self.doc()?;
                let f = d.layer(p.folder).ok_or(DocError::NoSuchLayer(p.folder))?;
                if !f.is_folder() {
                    return Err(DocError::NotAFolder(p.folder).into());
                }
                // Not a History step (H-002): apply to every version that has this folder.
                for s in &mut self.steps {
                    if s.doc.layer(p.folder).is_some_and(|l| l.expanded != p.open) {
                        let mut d = Document::clone(&s.doc);
                        d.layer_mut(p.folder)?.expanded = p.open;
                        s.doc = Arc::new(d);
                    }
                }
                Ok(())
            }
            Command::InputPressureCurve(p) => {
                self.pressure_curve = p.points.clone();
                Ok(())
            }
            Command::LayerSelect(p) => {
                let l = self
                    .doc()?
                    .layer(p.layer)
                    .ok_or(DocError::NoSuchLayer(p.layer))?;
                let mask = p.target == Some(Target::Mask);
                if mask && l.mask.is_none() {
                    return Err(ExecError(format!("{} has no mask", p.layer)));
                }
                self.active = Some(p.layer);
                self.active_mask = mask;
                Ok(())
            }
            _ => Err(ExecError(format!(
                "{} is not an app-state command",
                cmd.name()
            ))),
        }
    }

    fn enforce_limit(&mut self) {
        while self.steps.len() > self.undo_limit + 1 {
            self.steps.remove(0);
            self.pos = self.pos.saturating_sub(1);
        }
    }

    /// The active layer must exist in the current version (it may not after undo/redo).
    fn fix_active(&mut self) {
        if let (Some(a), Some(d)) = (self.active, self.steps.get(self.pos))
            && d.doc.layer(a).is_none()
        {
            self.active = None;
        }
        // The mask target needs a mask.
        let has_mask = self
            .active
            .zip(self.steps.get(self.pos))
            .and_then(|(a, d)| d.doc.layer(a).map(|l| l.mask.is_some()))
            .unwrap_or(false);
        if !has_mask {
            self.active_mask = false;
        }
    }

    /// Answer a query (never journaled, never changes state).
    pub fn query(&self, q: &Query) -> Result<Json, ExecError> {
        Ok(match q {
            Query::Commands(_) => commands::registry_json(),
            Query::Tools(_) => self.tools.tools_json(),
            Query::Color(_) => self.tools.color_json(),
            Query::Subtool(p) => self
                .tools
                .subtool_json(self.tools.target(p.tool, p.group, p.item)?),
            Query::InputPressureCurve(_) => Json::Array(
                self.pressure_curve
                    .points()
                    .iter()
                    .map(|p| json!(p))
                    .collect(),
            ),
            Query::History(_) => json!({
                "position": self.pos,
                "steps": self.steps.iter().map(|s| s.label).collect::<Vec<_>>(),
                "can_undo": self.pos > 0,
                "can_redo": self.pos + 1 < self.steps.len(),
                "limit": self.undo_limit,
            }),
            Query::DocInfo(_) => {
                let d = self.doc()?;
                json!({
                    "width": d.width, "height": d.height, "dpi": d.dpi,
                    "paper": d.paper.map(|p| p.to_json()),
                    "layers": d.layers().len(),
                    "active": self.active.map(|a| a.to_string()),
                    "target": if self.active_mask { "mask" } else { "layer" },
                })
            }
            Query::LayerList(_) => {
                Json::Array(self.doc()?.layers().iter().map(|l| layer_json(l)).collect())
            }
            Query::LayerGet(p) => layer_json(
                self.doc()?
                    .layer(p.layer)
                    .ok_or(DocError::NoSuchLayer(p.layer))?,
            ),
            Query::DocHash(_) => Json::String(format!("{:016x}", self.doc()?.content_hash())),
            Query::Pixel(p) => {
                let d = self.doc()?;
                in_canvas(d, p.x, p.y)?;
                // Renders the whole document; fine for queries, the app renders incrementally.
                let img = brushye_core::render::flatten(d);
                json!(img.pixel(p.x, p.y))
            }
            Query::MaskPixel(p) => {
                let d = self.doc()?;
                in_canvas(d, p.x, p.y)?;
                let l = d.layer(p.layer).ok_or(DocError::NoSuchLayer(p.layer))?;
                let m = l
                    .mask
                    .as_ref()
                    .ok_or_else(|| ExecError(format!("{} has no mask", p.layer)))?;
                json!(m.value(p.x, p.y))
            }
            Query::Selection(_) => {
                let d = self.doc()?;
                json!({
                    "selected": d.selection.is_some(),
                    "bounds": d.selection.as_ref().and_then(|s| s.bounds(d.width, d.height)),
                    "can_reselect": d.last_selection.is_some(),
                })
            }
            Query::SelectionPixel(p) => {
                let d = self.doc()?;
                in_canvas(d, p.x, p.y)?;
                json!(d.selection.as_ref().map_or(0, |s| s.value(p.x, p.y)))
            }
            Query::LayerPixel(p) => {
                let d = self.doc()?;
                in_canvas(d, p.x, p.y)?;
                json!(
                    d.layer(p.layer)
                        .ok_or(DocError::NoSuchLayer(p.layer))?
                        .pixel(p.x, p.y)
                )
            }
        })
    }

    /// Run one text line live (script mode): a command, a query (returns `Some`), a comment or
    /// a blank line. Journal lines (`@n …`) are rejected here; use [`Session::replay`].
    pub fn run_line(&mut self, text: &str) -> Result<Option<Json>, ExecError> {
        match parse_line(text).map_err(|e| ExecError(e.to_string()))? {
            Line::Blank | Line::Comment(_) => Ok(None),
            Line::Item(raw) if raw.query => {
                let q = Query::from_raw(&raw)?;
                self.query(&q).map(Some)
            }
            Line::Item(raw) => {
                if raw.seq.is_some() {
                    return Err(ExecError(
                        "journal lines (@n) can only be replayed, not run as a script".into(),
                    ));
                }
                let raw = commands::migrate(raw)?;
                self.execute(Command::from_raw(&raw)?)?;
                Ok(None)
            }
        }
    }

    /// Run a whole script; stops at the first error. Query results are returned in order.
    pub fn run_script(&mut self, text: &str) -> Result<Vec<Json>, LineError> {
        let mut out = Vec::new();
        for (i, l) in text.lines().enumerate() {
            match self.run_line(l) {
                Ok(Some(j)) => out.push(j),
                Ok(None) => {}
                Err(e) => {
                    return Err(LineError {
                        line: i + 1,
                        msg: e.0,
                    });
                }
            }
        }
        Ok(out)
    }

    /// The journal so far, as text (header + one line per executed command).
    pub fn journal_text(&self) -> String {
        let mut s = format!(
            "#brushye-journal v{JOURNAL_FORMAT} app={}",
            env!("CARGO_PKG_VERSION")
        );
        if let Some(b) = self.base {
            s.push_str(&format!(" base={b:016x}"));
        }
        s.push('\n');
        for raw in &self.journal {
            s.push_str(&print_line(raw));
            s.push('\n');
        }
        s
    }

    /// Rebuild a session from a journal: the header is required, sequence numbers must run
    /// 1, 2, 3 …, old command versions are migrated, and recorded merges are applied as written.
    pub fn replay(text: &str) -> Result<Session, LineError> {
        Self::replay_from(Session::new(), text)
    }

    /// Replay a journal recorded on top of an opened document (`base=` in its header). The
    /// document must be the same one: its content hash is checked.
    pub fn replay_on(
        base: Document,
        active: Option<LayerId>,
        text: &str,
    ) -> Result<Session, LineError> {
        Self::replay_from(Session::from_document(base, active), text)
    }

    fn replay_from(mut s: Session, text: &str) -> Result<Session, LineError> {
        let mut lines = text.lines().enumerate();
        let err = |line: usize, msg: String| LineError { line, msg };
        match lines.next() {
            Some((_, h)) if h.starts_with("#brushye-journal ") => {
                let mut words = h.split_whitespace().skip(1);
                let v = words.next().unwrap_or("");
                if v != format!("v{JOURNAL_FORMAT}") {
                    return Err(err(
                        1,
                        format!(
                            "unsupported journal format '{v}' (this Brushye reads v{JOURNAL_FORMAT})"
                        ),
                    ));
                }
                let base = words
                    .find_map(|w| w.strip_prefix("base="))
                    .map(|b| {
                        u64::from_str_radix(b, 16)
                            .map_err(|_| err(1, format!("bad base hash '{b}'")))
                    })
                    .transpose()?;
                match (base, s.base) {
                    (None, None) => {}
                    (Some(_), None) => {
                        return Err(err(
                            1,
                            "this journal continues a saved document (base= in the header); replay it on that document".into(),
                        ));
                    }
                    (None, Some(_)) => {
                        return Err(err(
                            1,
                            "this journal starts with doc.new; replay it without a base document"
                                .into(),
                        ));
                    }
                    (Some(want), Some(have)) if want != have => {
                        return Err(err(
                            1,
                            format!(
                                "base document differs: journal expects {want:016x}, got {have:016x}"
                            ),
                        ));
                    }
                    _ => {}
                }
            }
            _ => return Err(err(1, "missing '#brushye-journal v1 …' header".into())),
        }
        for (i, l) in lines {
            let n = i + 1;
            let raw = match parse_line(l).map_err(|e| err(n, e.to_string()))? {
                Line::Blank | Line::Comment(_) => continue,
                Line::Item(r) => r,
            };
            if raw.query {
                return Err(err(n, "queries don't belong in a journal".into()));
            }
            let expected = s.journal.len() as u64 + 1;
            if raw.seq != Some(expected) {
                return Err(err(n, format!("expected sequence number @{expected}")));
            }
            let merge = raw.merge;
            let raw = commands::migrate(raw).map_err(|e| err(n, e))?;
            let cmd = Command::from_raw(&raw).map_err(|e| err(n, e))?;
            if merge && cmd.class() != Class::Undoable {
                return Err(err(n, format!("{} can't join an undo step", cmd.name())));
            }
            s.apply(&cmd, merge).map_err(|e| err(n, e.0))?;
            let mut rec = cmd.to_raw();
            rec.seq = Some(expected);
            rec.merge = merge;
            s.journal.push(rec);
        }
        Ok(s)
    }
}

fn in_canvas(d: &Document, x: u32, y: u32) -> Result<(), ExecError> {
    if x < d.width && y < d.height {
        Ok(())
    } else {
        Err(ExecError(format!(
            "({x}, {y}) is outside the {}×{} canvas",
            d.width, d.height
        )))
    }
}

/// Write pixels honouring the layer's locks (lock transparent pixels keeps alpha, spec L-006).
fn write_pixels(
    d: &mut Document,
    id: LayerId,
    x: i64,
    y: i64,
    w: u32,
    h: u32,
    rgba: &[u8],
) -> Result<(), ExecError> {
    let bounds = (d.width, d.height);
    let l = d.layer_mut(id)?;
    if l.locked {
        return Err(DocError::LayerLocked(id).into());
    }
    if l.is_folder() {
        return Err(ExecError(format!(
            "{id}: a folder has no pixels of its own"
        )));
    }
    if l.lock_alpha {
        let mut kept = rgba.to_vec();
        for row in 0..h {
            for col in 0..w {
                let (cx, cy) = (x + i64::from(col), y + i64::from(row));
                let i = ((row * w + col) * 4) as usize;
                let old =
                    if cx >= 0 && cy >= 0 && cx < i64::from(bounds.0) && cy < i64::from(bounds.1) {
                        l.pixel(cx as u32, cy as u32)
                    } else {
                        [0; 4]
                    };
                // Keep alpha; fully transparent pixels stay exactly as they were.
                if old[3] == 0 {
                    kept[i..i + 4].copy_from_slice(&old);
                } else {
                    kept[i + 3] = old[3];
                }
            }
        }
        l.put_pixels(bounds, x, y, w, h, &kept);
    } else {
        l.put_pixels(bounds, x, y, w, h, rgba);
    }
    Ok(())
}

fn layer_json(l: &Layer) -> Json {
    json!({
        "id": l.id.to_string(),
        "name": l.name,
        "kind": l.kind.name(),
        "visible": l.visible,
        "opacity": l.opacity,
        "blend": l.blend.name(),
        "locked": l.locked,
        "lock_alpha": l.lock_alpha,
        "parent": l.parent.map(|p| p.to_string()),
        "mask": l.mask.as_ref().map(|m| json!({
            "enabled": m.enabled,
            "linked": m.linked,
            "show_area": m.show_area,
            "tiles": m.tile_count(),
        })),
        "clip": l.clip,
        "expanded": l.expanded,
        "tiles": l.tiles.len(),
    })
}

/// Apply an undoable command to a document copy.
fn edit(d: &mut Document, cmd: &Command, pressure_curve: &Curve) -> Result<(), ExecError> {
    match cmd {
        Command::LayerNew(p) => {
            let kind = p.kind.unwrap_or(LayerKind::Raster);
            let name = p
                .name
                .clone()
                .unwrap_or_else(|| format!("Layer {}", p.layer.0));
            let mut layer = Layer::new(p.layer, name, kind);
            layer.clip = p.clip.unwrap_or(false);
            match (p.above, p.into) {
                (_, None) => d.add_layer(layer, p.above)?,
                (None, Some(f)) => d.add_layer_into(layer, f)?,
                (Some(_), Some(_)) => {
                    return Err(ExecError(
                        "layer.new takes above=… or into=…, not both".into(),
                    ));
                }
            }
        }
        Command::LayerGroup(p) => {
            let name = p
                .name
                .clone()
                .unwrap_or_else(|| format!("Folder {}", p.folder.0));
            d.group(Layer::new(p.folder, name, LayerKind::Folder), p.layer)?;
        }
        Command::LayerUngroup(p) => d.ungroup(p.folder)?,
        Command::MaskNew(p) => {
            let bounds = (d.width, d.height);
            let sel = d.selection.clone();
            let sel = sel.as_ref();
            let l = d.layer_mut(p.layer)?;
            if l.mask.is_some() {
                return Err(ExecError(format!("{} already has a mask", p.layer)));
            }
            let mut mask = brushye_core::Mask::new(match p.fill.unwrap_or(MaskFill::Reveal) {
                MaskFill::Reveal | MaskFill::Selection => 255,
                MaskFill::Hide | MaskFill::Outside => 0,
            });
            // From a selection (S-001): Mask Outside Selection gives M = S, Mask Selection
            // hides the selected area, M = 255 − S. Without a selection S is 0 everywhere.
            if let (Some(s), Some(f @ (MaskFill::Selection | MaskFill::Outside))) = (sel, p.fill) {
                let mut v = s.to_dense(bounds.0, bounds.1);
                if f == MaskFill::Selection {
                    sel::invert(&mut v);
                }
                mask.grid = Grid8::from_dense(bounds.0, bounds.1, &v);
            }
            l.mask = Some(mask);
        }
        Command::MaskDelete(p) => {
            let l = d.layer_mut(p.layer)?;
            if l.mask.take().is_none() {
                return Err(ExecError(format!("{} has no mask", p.layer)));
            }
        }
        Command::MaskApply(p) => {
            let l = d.layer_mut(p.layer)?;
            if l.is_folder() {
                // CSP greys out Apply Mask to Layer for folders (L-008 run 3).
                return Err(ExecError(format!(
                    "{}: a folder's mask can't be applied",
                    p.layer
                )));
            }
            let m = l
                .mask
                .take()
                .ok_or_else(|| ExecError(format!("{} has no mask", p.layer)))?;
            // Apply Mask to Layer writes exactly floor(a·M/255) (L-008).
            let coords: Vec<_> = l.tiles.iter().map(|(c, _)| *c).collect();
            for c in coords {
                let mt = m.tile(c).cloned();
                let t = l.tiles.get_mut(c);
                for (i, px) in t.px.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                    let v = mt.as_ref().map_or(m.background, |mt| mt.v[i]);
                    px[3] = brushye_core::Mask::apply(px[3], v);
                }
            }
            l.tiles.prune();
        }
        Command::MaskSet(p) => {
            if p.enabled.is_none() && p.linked.is_none() && p.show_area.is_none() {
                return Err(ExecError("mask.set needs at least one setting".into()));
            }
            let m = d
                .layer_mut(p.layer)?
                .mask
                .as_mut()
                .ok_or_else(|| ExecError(format!("{} has no mask", p.layer)))?;
            if let Some(v) = p.enabled {
                m.enabled = v;
            }
            if let Some(v) = p.linked {
                m.linked = v;
            }
            if let Some(v) = p.show_area {
                m.show_area = v;
            }
        }
        Command::LayerDelete(p) => {
            d.remove_layer(p.layer)?;
        }
        Command::LayerMove(p) => match (p.above, p.to, p.into, p.below) {
            (Some(a), None, None, None) => d.move_layer(p.layer, Some(a))?,
            (None, None, Some(f), None) => d.move_into(p.layer, f)?,
            (None, None, None, Some(b)) => d.move_below(p.layer, b)?,
            (None, Some(Place::Bottom), None, None) => d.move_layer(p.layer, None)?,
            (None, Some(Place::Top), None, None) => {
                // Above the topmost top-level layer (a folder's entry is its topmost layer).
                let top = d
                    .layers()
                    .iter()
                    .rev()
                    .find(|l| l.parent.is_none() && l.id != p.layer)
                    .map(|l| l.id);
                match top {
                    Some(t) => d.move_layer(p.layer, Some(t))?,
                    None => d.move_layer(p.layer, None)?,
                }
            }
            _ => {
                return Err(ExecError(
                    "layer.move needs exactly one of above=…, below=…, into=… or to=…".into(),
                ));
            }
        },
        Command::LayerSet(p) => {
            let only_unlock = p.locked == Some(false)
                && p.name.is_none()
                && p.opacity.is_none()
                && p.blend.is_none()
                && p.lock_alpha.is_none()
                && p.clip.is_none();
            if p.name.is_none()
                && p.opacity.is_none()
                && p.blend.is_none()
                && p.locked.is_none()
                && p.lock_alpha.is_none()
                && p.clip.is_none()
            {
                return Err(ExecError("layer.set needs at least one property".into()));
            }
            if let Some(o) = p.opacity
                && o > 100
            {
                return Err(DocError::OpacityOutOfRange(u32::from(o)).into());
            }
            let l = d.layer_mut(p.layer)?;
            if l.locked && !only_unlock {
                return Err(DocError::LayerLocked(p.layer).into());
            }
            if let Some(v) = &p.name {
                l.name.clone_from(v);
            }
            if let Some(v) = p.opacity {
                l.opacity = v;
            }
            if let Some(v) = p.blend {
                if v == BlendMode::Through && !l.is_folder() {
                    return Err(DocError::ThroughOnRaster(p.layer).into());
                }
                l.blend = v;
            }
            if let Some(v) = p.clip {
                l.clip = v;
            }
            if let Some(v) = p.locked {
                l.locked = v;
            }
            if let Some(v) = p.lock_alpha {
                l.lock_alpha = v;
            }
        }
        Command::LayerFill(p) => {
            let (w, h) = (d.width, d.height);
            let block: Vec<u8> = p
                .color
                .0
                .iter()
                .copied()
                .cycle()
                .take(w as usize * h as usize * 4)
                .collect();
            write_pixels(d, p.layer, 0, 0, w, h, &block)?;
        }
        Command::BrushStroke(p) if p.target == Some(Target::Selection) => {
            // Selection pen / Erase selection (S-001 run 2): the stroke's alpha, combined with
            // the selection by the mode. The layer is not touched.
            let settings = brush_settings(p, pressure_curve)?;
            let (w, h) = (d.width, d.height);
            d.layer(p.layer).ok_or(DocError::NoSuchLayer(p.layer))?;
            let mut tmp = Layer::new(LayerId(0), "selection", LayerKind::Raster);
            brushye_core::brush::stroke(&mut tmp, (w, h), &settings, &p.samples.0)
                .map_err(|e| ExecError(e.to_string()))?;
            let mut b = vec![0u8; w as usize * h as usize];
            for y in 0..h {
                for x in 0..w {
                    b[(y * w + x) as usize] = tmp.pixel(x, y)[3];
                }
            }
            let cur = d.selection.as_ref().map(|s| s.to_dense(w, h));
            let v = sel::combine(p.mode.unwrap_or(SelectMode::Add), cur.as_deref(), &b);
            set_selection(d, v);
        }
        Command::BrushStroke(p) => {
            if p.mode.is_some() {
                return Err(ExecError(
                    "brush.stroke: mode=… is for target=selection".into(),
                ));
            }
            let settings = brush_settings(p, pressure_curve)?;
            let bounds = (d.width, d.height);
            // Strokes are clipped to the selection (S-001 run 2).
            let clip = d.selection.clone();
            let l = d.layer_mut(p.layer)?;
            if l.locked {
                return Err(DocError::LayerLocked(p.layer).into());
            }
            if p.target == Some(Target::Mask) {
                // Masks are painted through a layer whose alpha is the mask value: opaque ink
                // raises it, the eraser lowers it (L-008).
                let m = l
                    .mask
                    .as_mut()
                    .ok_or_else(|| ExecError(format!("{} has no mask", p.layer)))?;
                let mut tmp = m.to_layer(bounds);
                brushye_core::brush::stroke_clipped(
                    &mut tmp,
                    bounds,
                    &settings,
                    &p.samples.0,
                    clip.as_ref(),
                )
                .map_err(|e| ExecError(e.to_string()))?;
                m.set_from_layer(&tmp, bounds);
            } else {
                if l.is_folder() {
                    return Err(ExecError(format!("{}: can't paint on a folder", p.layer)));
                }
                brushye_core::brush::stroke_clipped(
                    l,
                    bounds,
                    &settings,
                    &p.samples.0,
                    clip.as_ref(),
                )
                .map_err(|e| ExecError(e.to_string()))?;
            }
        }
        Command::SelectRect(p) => select_shape(
            d,
            p.mode,
            &Shape::Rect {
                x: p.x,
                y: p.y,
                w: p.w,
                h: p.h,
            },
        ),
        Command::SelectEllipse(p) => select_shape(
            d,
            p.mode,
            &Shape::Ellipse {
                x: p.x,
                y: p.y,
                w: p.w,
                h: p.h,
                aa: p.aa != Some(Aa::None),
            },
        ),
        Command::SelectPolygon(p) => {
            if p.points.0.len() < 3 {
                return Err(ExecError("select.polygon needs at least 3 points".into()));
            }
            select_shape(
                d,
                p.mode,
                &Shape::Polygon {
                    points: p.points.0.clone(),
                    aa: p.aa != Some(Aa::None),
                },
            );
        }
        Command::SelectAll(_) => d.selection = Some(Grid8::new(255)),
        Command::SelectNone(_) => {
            let s = d.selection.take().ok_or_else(nothing_selected)?;
            d.last_selection = Some(s);
        }
        Command::SelectReselect(_) => {
            d.selection = Some(
                d.last_selection
                    .clone()
                    .ok_or_else(|| ExecError("no selection to reselect".into()))?,
            );
        }
        Command::SelectInvert(_) => {
            let mut v = current_selection(d)?;
            sel::invert(&mut v);
            set_selection(d, v);
        }
        Command::SelectExpand(p) => {
            let v = current_selection(d)?;
            let round = p.shape == Some(MorphShape::Round);
            let v = sel::morph(&v, d.width, d.height, p.w, round, true, 0);
            set_selection(d, v);
        }
        Command::SelectShrink(p) => {
            let v = current_selection(d)?;
            let round = p.shape == Some(MorphShape::Round);
            let edge = if p.canvas_edge.unwrap_or(true) {
                0
            } else {
                255
            };
            let v = sel::morph(&v, d.width, d.height, p.w, round, false, edge);
            set_selection(d, v);
        }
        Command::SelectMove(p) => {
            let v = current_selection(d)?;
            let v = sel::shift(&v, d.width, d.height, p.dx, p.dy);
            set_selection(d, v);
        }
        Command::SelectBlur(p) => {
            if !(p.strength.is_finite() && (0.5..=1000.0).contains(&p.strength)) {
                return Err(ExecError(format!(
                    "select.blur: strength {} is not in 0.5..=1000",
                    p.strength
                )));
            }
            let v = current_selection(d)?;
            let v = sel::blur(&v, d.width, d.height, p.strength);
            set_selection(d, v);
        }
        Command::SelectFromLayer(p) => {
            let (w, h) = (d.width, d.height);
            let l = d.layer(p.layer).ok_or(DocError::NoSuchLayer(p.layer))?;
            if l.is_folder() {
                return Err(ExecError(format!("{}: a folder has no pixels", p.layer)));
            }
            let mut a = vec![0u8; w as usize * h as usize];
            for ((tx, ty), t) in l.tiles.iter() {
                let ts = TILE_SIZE as u32;
                for (i, px) in t.px.as_chunks::<4>().0.iter().enumerate() {
                    let x = *tx as u32 * ts + i as u32 % ts;
                    let y = *ty as u32 * ts + i as u32 / ts;
                    if x < w && y < h {
                        a[(y * w + x) as usize] = px[3];
                    }
                }
            }
            let op = p.op.unwrap_or(sel::LayerSelOp::Create);
            // CSP offers Add / Delete / Overlap only with a selection.
            let s = match op {
                sel::LayerSelOp::Create => vec![0u8; a.len()],
                _ => current_selection(d)?,
            };
            set_selection(d, sel::from_layer(op, &s, &a));
        }
        Command::EditFill(p) => {
            if p.color.0[3] != 255 {
                return Err(ExecError(
                    "edit.fill: color must be opaque (#rrggbb)".into(),
                ));
            }
            let (w, h) = (d.width, d.height);
            let s = d.selection.as_ref().map(|s| s.to_dense(w, h));
            let l = raster_target(d, p.layer)?;
            let rgb = [p.color.0[0], p.color.0[1], p.color.0[2]];
            // The selection value is the fill's alpha, laid on like ink (S-001: alpha = S on
            // an empty layer). Over existing pixels: open.
            for y in 0..h {
                for x in 0..w {
                    let v = s.as_ref().map_or(255, |s| s[(y * w + x) as usize]);
                    if v == 0 {
                        continue;
                    }
                    let dst = l.pixel(x, y);
                    let out = ink::apply(
                        InkMode::Normal,
                        dst,
                        rgb,
                        f64::from(v) / 255.0,
                        l.lock_alpha,
                    );
                    if out != dst {
                        put_pixel(l, x, y, out);
                    }
                }
            }
            l.tiles.prune();
        }
        Command::EditDelete(p) => {
            let (w, h) = (d.width, d.height);
            let s = d.selection.as_ref().map(|s| s.to_dense(w, h));
            let l = raster_target(d, p.layer)?;
            match s {
                None => l.tiles = Default::default(),
                Some(s) => {
                    // alpha → floor(a·(255 − S)/255) (S-001: 255 − S on an opaque layer).
                    let coords: Vec<_> = l.tiles.iter().map(|(c, _)| *c).collect();
                    let ts = TILE_SIZE as u32;
                    for (tx, ty) in coords {
                        let t = l.tiles.get_mut((tx, ty));
                        for (i, px) in t.px.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                            let x = tx as u32 * ts + (i as u32 % ts);
                            let y = ty as u32 * ts + (i as u32 / ts);
                            if x < w && y < h {
                                let v = s[(y * w + x) as usize];
                                px[3] = brushye_core::Mask::apply(px[3], 255 - v);
                                if px[3] == 0 {
                                    *px = [0; 4];
                                }
                            }
                        }
                    }
                    l.tiles.prune();
                }
            }
        }
        Command::LayerPixels(p) => {
            let need = u64::from(p.w) * u64::from(p.h) * 4;
            if p.data.0.len() as u64 != need {
                return Err(ExecError(format!(
                    "layer.pixels: data has {} bytes, w*h*4 = {need}",
                    p.data.0.len()
                )));
            }
            write_pixels(d, p.layer, p.x, p.y, p.w, p.h, &p.data.0)?;
        }
        _ => {
            return Err(ExecError(format!(
                "{} is not an undoable command",
                cmd.name()
            )));
        }
    }
    Ok(())
}

fn nothing_selected() -> ExecError {
    ExecError("nothing is selected".into())
}

/// The current selection as a dense canvas buffer.
fn current_selection(d: &Document) -> Result<Vec<u8>, ExecError> {
    d.selection
        .as_ref()
        .map(|s| s.to_dense(d.width, d.height))
        .ok_or_else(nothing_selected)
}

/// Store a selection result; all zero means nothing is selected.
fn set_selection(d: &mut Document, v: Vec<u8>) {
    d.selection = v
        .iter()
        .any(|&p| p != 0)
        .then(|| Grid8::from_dense(d.width, d.height, &v));
}

/// A shape combined with the current selection by the selection mode (S-001 run 2).
fn select_shape(d: &mut Document, mode: Option<SelectMode>, shape: &Shape) {
    let (w, h) = (d.width, d.height);
    let b = sel::rasterize(shape, w, h);
    let mode = mode.unwrap_or_default();
    let cur = match mode {
        SelectMode::New => None,
        _ => d.selection.as_ref().map(|s| s.to_dense(w, h)),
    };
    set_selection(d, sel::combine(mode, cur.as_deref(), &b));
}

/// A raster layer that edits may change.
fn raster_target(d: &mut Document, id: LayerId) -> Result<&mut Layer, ExecError> {
    let l = d.layer_mut(id)?;
    if l.locked {
        return Err(DocError::LayerLocked(id).into());
    }
    if l.is_folder() {
        return Err(ExecError(format!("{id} is a folder")));
    }
    Ok(l)
}

fn put_pixel(l: &mut Layer, x: u32, y: u32, px: [u8; 4]) {
    let ts = TILE_SIZE as u32;
    let t = l.tiles.get_mut(((x / ts) as i32, (y / ts) as i32));
    let i = (((y % ts) * ts + x % ts) * 4) as usize;
    t.px[i..i + 4].copy_from_slice(&px);
}

/// Brush settings of a `brush.stroke` command (validated by the engine).
fn brush_settings(p: &commands::BrushStroke, global: &Curve) -> Result<BrushSettings, ExecError> {
    if p.color.0[3] != 255 {
        return Err(ExecError(
            "brush.stroke: color must be opaque (#rrggbb); use opacity=… for transparency".into(),
        ));
    }
    crate::brush_preset::BrushPreset::from_stroke(p)
        .settings_with([p.color.0[0], p.color.0[1], p.color.0[2]], global)
        .map_err(|e| ExecError(format!("brush.stroke: {e}")))
}

/// Text of a single parameter value (for error messages and tools).
pub fn arg_text<T: Arg>(v: &T) -> String {
    value_text(&v.to_value())
}
