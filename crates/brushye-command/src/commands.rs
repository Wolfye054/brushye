//! The command and query registry (M2 set). Each entry declares its name, version, class and
//! typed parameters; parsing, printing, JSON and the schema all derive from it (D-011).

use brushye_core::brush::{Aa, Curve, Gap, InkMode, TipAxis};
use brushye_core::{BlendMode, LayerId, LayerKind, Rgba8};
use serde_json::{Value as Json, json};

use crate::params;
use crate::params::{Arg, Blob, JsonReader, Mode, ParamSpec, Points, Reader, Samples};
use crate::syntax::{RawLine, Value};
use crate::tools::{Chip, Tool};
use brushye_core::selection::{LayerSelOp, SelectMode};

/// How a command interacts with history (D-013, spec H-001).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// Starts a new document and a fresh history.
    Session,
    /// Changes the document; one undo step (or merged into the previous one).
    Undoable,
    /// Application/view state: journaled, never undoable.
    AppState,
    /// Tools, sub tools and colour chips (D-018): never undoable and not journaled, since they
    /// can't change the document (a stroke records its settings and colour).
    Tool,
    /// `history.undo` / `history.redo`.
    History,
}

/// Paper colour or no paper: `#rrggbb[aa]` or the word `none`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paper {
    Color(Rgba8),
    None,
}

impl Arg for Paper {
    const KIND: &'static str = "colour or none";
    fn from_value(v: &Value) -> Result<Self, String> {
        match v {
            Value::Word(w) if w == "none" => Ok(Paper::None),
            other => Rgba8::from_value(other)
                .map(Paper::Color)
                .map_err(|_| format!("expected a colour or none, found {}", other.kind())),
        }
    }
    fn to_value(&self) -> Value {
        match self {
            Paper::Color(c) => c.to_value(),
            Paper::None => Value::Word("none".into()),
        }
    }
    fn to_json(&self) -> Json {
        match self {
            Paper::Color(c) => c.to_json(),
            Paper::None => Json::String("none".into()),
        }
    }
    fn from_json(j: &Json) -> Result<Self, String> {
        match j.as_str() {
            Some("none") => Ok(Paper::None),
            _ => Rgba8::from_json(j).map(Paper::Color),
        }
    }
}

/// Word-valued enums for the command syntax (`name()` / `from_name()` + [`Arg`]).
macro_rules! word_enum {
    ($(#[$m:meta])* $t:ident, $kind:literal, { $($v:ident = $w:literal),* $(,)? }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $t { $($v),* }
        impl $t {
            pub fn name(self) -> &'static str { match self { $($t::$v => $w),* } }
            pub fn from_name(s: &str) -> Option<Self> { match s { $($w => Some($t::$v),)* _ => None } }
        }
        impl Arg for $t {
            const KIND: &'static str = $kind;
            fn from_value(v: &Value) -> Result<Self, String> {
                match v {
                    Value::Word(w) => $t::from_name(w).ok_or_else(|| format!("unknown {} '{w}'", $kind)),
                    other => Err(format!("expected {}, found {}", $kind, other.kind())),
                }
            }
            fn to_value(&self) -> Value { Value::Word(self.name().into()) }
            fn to_json(&self) -> Json { Json::String(self.name().into()) }
            fn from_json(j: &Json) -> Result<Self, String> {
                j.as_str().and_then($t::from_name).ok_or_else(|| format!("expected {} in JSON", $kind))
            }
        }
    };
}

word_enum!(
    /// What a stroke paints, or what is selected for painting: the layer's pixels or its mask.
    Target, "target (layer|mask|selection)", { Layer = "layer", Mask = "mask", Selection = "selection" }
);
word_enum!(
    /// A new mask's initial content (U-014: the palette button reveals all; Mask Outside
    /// Selection with no selection hides all).
    MaskFill, "mask fill (reveal|hide|selection|outside)", {
        Reveal = "reveal", Hide = "hide", Selection = "selection", Outside = "outside"
    }
);
word_enum!(
    /// What a fill or auto-select click looks at (F-001): the given layer only (Refer only to
    /// editing layer) or the visible image (Refer other layers / Refer to all layers).
    FillRef, "reference (layer|all)", { Layer = "layer", All = "all" }
);
word_enum!(
    /// Expand / Shrink selected area type (U-016).
    MorphShape, "shape (square|round)", { Square = "square", Round = "round" }
);

/// `top` or `bottom` of the layer stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Top,
    Bottom,
}

impl Place {
    pub fn name(self) -> &'static str {
        match self {
            Place::Top => "top",
            Place::Bottom => "bottom",
        }
    }
    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "top" => Some(Place::Top),
            "bottom" => Some(Place::Bottom),
            _ => None,
        }
    }
}

impl Arg for Place {
    const KIND: &'static str = "place (top|bottom)";
    fn from_value(v: &Value) -> Result<Self, String> {
        match v {
            Value::Word(w) => {
                Place::from_name(w).ok_or_else(|| format!("unknown place '{w}' (top|bottom)"))
            }
            other => Err(format!("expected top or bottom, found {}", other.kind())),
        }
    }
    fn to_value(&self) -> Value {
        Value::Word(self.name().into())
    }
    fn to_json(&self) -> Json {
        Json::String(self.name().into())
    }
    fn from_json(j: &Json) -> Result<Self, String> {
        j.as_str()
            .and_then(Place::from_name)
            .ok_or_else(|| "expected \"top\" or \"bottom\"".into())
    }
}

params! {
    /// New document; starts a fresh history.
    pub struct DocNew {
        width: u32 = pos "Canvas width in pixels",
        height: u32 = pos "Canvas height in pixels",
        dpi: Option<u32> = opt "Resolution (default 72)",
        paper: Option<Paper> = opt "Paper colour, or none (default #ffffff)",
    }
}
params! {
    /// New raster layer with a caller-chosen id.
    pub struct LayerNew {
        layer: LayerId = pos "Id for the new layer (must be unused)",
        kind: Option<LayerKind> = opt "Layer kind (default raster)",
        name: Option<String> = opt "Name (default \"Layer <n>\")",
        above: Option<LayerId> = opt "Insert directly above this layer, in its folder (default: on top)",
        into: Option<LayerId> = opt "Insert at the top of this folder (instead of above=)",
        clip: Option<bool> = opt "Clip to Layer Below (CSP clips a new layer inserted inside a clipping group)",
    }
}
params! {
    /// Delete a layer.
    pub struct LayerDelete {
        layer: LayerId = pos "Layer to delete",
    }
}
params! {
    /// Move a layer: directly above another layer, or to the top/bottom.
    pub struct LayerMove {
        layer: LayerId = pos "Layer to move",
        above: Option<LayerId> = opt "Place directly above this layer, in its folder",
        to: Option<Place> = opt "top or bottom (of the whole stack, top level)",
        into: Option<LayerId> = opt "Place at the top of this folder",
        below: Option<LayerId> = opt "Place directly below this layer, in its folder",
    }
}
params! {
    /// Set layer properties (undoable). Visibility is `layer.show`.
    pub struct LayerSet {
        layer: LayerId = pos "Target layer",
        name: Option<String> = opt "Name",
        opacity: Option<u8> = opt "Opacity in percent, 0..=100",
        blend: Option<BlendMode> = opt "Blending mode (through: folders only)",
        locked: Option<bool> = opt "Lock layer",
        lock_alpha: Option<bool> = opt "Lock transparent pixels",
        clip: Option<bool> = opt "Clip to Layer Below",
    }
}
params! {
    /// Wrap a layer (a folder with its contents) in a new folder that takes its place
    /// (Create folder and insert layer, Ctrl+G).
    pub struct LayerGroup {
        folder: LayerId = pos "Id for the new folder (must be unused)",
        layer: LayerId = key "Layer to put in the folder",
        name: Option<String> = opt "Folder name (default \"Folder <n>\")",
    }
}
params! {
    /// Remove a folder and leave its contents in its place (Ungroup Layer Folder).
    pub struct LayerUngroup {
        folder: LayerId = pos "Folder to ungroup",
    }
}
params! {
    /// Open or close a folder in the Layer palette (journaled, not undoable: H-002).
    pub struct LayerExpand {
        folder: LayerId = pos "Folder",
        open: bool = key "Open (true) or closed (false)",
    }
}
params! {
    /// Fill a layer's whole canvas area with one colour.
    pub struct LayerFill {
        layer: LayerId = pos "Target layer",
        color: Rgba8 = key "Fill colour #rrggbb or #rrggbbaa (straight alpha)",
    }
}
params! {
    /// Write a block of raw straight-RGBA8 pixels into a layer.
    pub struct LayerPixels {
        layer: LayerId = pos "Target layer",
        x: i64 = key "Left edge in canvas pixels",
        y: i64 = key "Top edge in canvas pixels",
        w: u32 = key "Block width",
        h: u32 = key "Block height",
        data: Blob = key "w*h*4 bytes, row-major RGBA",
    }
}
params! {
    /// Show or hide a layer (journaled, not undoable, as in CSP — spec H-001).
    pub struct LayerShow {
        layer: LayerId = pos "Target layer",
        visible: bool = key "Visible",
    }
}
params! {
    /// Make a layer the active (selected) layer (application state).
    pub struct LayerSelect {
        layer: LayerId = pos "Layer to select",
        target: Option<Target> = opt "Paint the layer (default) or its mask",
    }
}
params! {
    /// Create layer mask: a new mask that reveals or hides everything (L-008, U-014).
    pub struct MaskNew {
        layer: LayerId = pos "Layer or folder",
        fill: Option<MaskFill> = opt "reveal (default, the palette button), hide, selection (Mask Selection: hides the selected area, M = 255 − S) or outside (Mask Outside Selection: M = S)",
    }
}
params! {
    /// Delete Mask.
    pub struct MaskDelete {
        layer: LayerId = pos "Layer or folder",
    }
}
params! {
    /// Apply Mask to Layer: the layer's alpha becomes floor(a·M/255), and the mask is removed.
    pub struct MaskApply {
        layer: LayerId = pos "Raster layer",
    }
}
params! {
    /// Mask settings (each is a History step in CSP, H-002).
    pub struct MaskSet {
        layer: LayerId = pos "Layer or folder",
        enabled: Option<bool> = opt "Enable Mask",
        linked: Option<bool> = opt "Link Mask to Layer",
        show_area: Option<bool> = opt "Show Mask Area",
    }
}
params! {
    /// Paint one brush stroke (round tip) from recorded input samples. Settings left out take
    /// their defaults; `*_pressure` turns on pen pressure for that setting with the given
    /// minimum value, shaped by the optional `*_curve`. Pressure in `samples` is taken after
    /// the global pen pressure adjustment.
    pub struct BrushStroke {
        layer: LayerId = pos "Target layer",
        color: Rgba8 = key "Ink colour #rrggbb",
        size: f64 = key "Brush size (diameter) in px",
        aa: Option<Aa> = opt "Anti-aliasing (default middle)",
        hardness: Option<u8> = opt "Tip hardness 1 (soft) ..= 5 (hard, default)",
        opacity: Option<u8> = opt "Ink opacity in percent (default 100)",
        density: Option<u8> = opt "Brush density in percent (default 100)",
        ink: Option<InkMode> = opt "Ink blending mode (default normal): the layer modes plus erase, background, replace_alpha, compare_density, erase_compare, black_burn, white_burn",
        gap: Option<Gap> = opt "Dab gap: narrow, normal (default), wide, or percent of size",
        density_by_gap: Option<bool> = opt "Adjust brush density by gap",
        thickness: Option<u8> = opt "Tip thickness in percent, 1..=100 (default 100 = round)",
        direction: Option<TipAxis> = opt "Axis that thickness shortens (default vertical)",
        angle: Option<f64> = opt "Tip angle in degrees, clockwise (default 0)",
        angle_line: Option<bool> = opt "Tip angle follows the direction of the line",
        stabilization: Option<u8> = opt "Stabilization 0..=100 (default 0)",
        size_pressure: Option<u8> = opt "Pen pressure controls size; minimum in percent",
        size_curve: Option<Curve> = opt "Size pressure curve points [x,y;…]",
        opacity_pressure: Option<u8> = opt "Pen pressure controls opacity; minimum in percent",
        opacity_curve: Option<Curve> = opt "Opacity pressure curve points [x,y;…]",
        density_pressure: Option<u8> = opt "Pen pressure controls density; minimum in percent",
        density_curve: Option<Curve> = opt "Density pressure curve points [x,y;…]",
        samples: Samples = key "Input samples (b64:… or [x,y,p,t;…])",
        target: Option<Target> = opt "Paint the layer's pixels (default), its mask, or the selection (Selection pen; the layer is untouched)",
        mode: Option<SelectMode> = opt "With target=selection: how the stroke combines with the selection (default add; Erase selection = subtract)",
    }
}
params! {
    /// Select a rectangle: the integer box [x, x+w) × [y, y+h) (S-001).
    pub struct SelectRect {
        x: i64 = key "Left edge in canvas pixels",
        y: i64 = key "Top edge in canvas pixels",
        w: u32 = key "Width",
        h: u32 = key "Height",
        mode: Option<SelectMode> = opt "new (default), add, subtract or intersect",
    }
}
params! {
    /// Select the ellipse that fills a box.
    pub struct SelectEllipse {
        x: f64 = key "Box left edge in canvas pixels",
        y: f64 = key "Box top edge in canvas pixels",
        w: f64 = key "Box width",
        h: f64 = key "Box height",
        aa: Option<Aa> = opt "Anti-aliasing (default middle; weak, middle and strong are identical, S-001)",
        mode: Option<SelectMode> = opt "new (default), add, subtract or intersect",
    }
}
params! {
    /// Select a closed polygon (Lasso, Polyline).
    pub struct SelectPolygon {
        points: Points = key "Corners [x,y;…] in canvas pixels; the last joins the first",
        aa: Option<Aa> = opt "Anti-aliasing (default middle)",
        mode: Option<SelectMode> = opt "new (default), add, subtract or intersect",
    }
}
params! {
    /// Select All.
    pub struct SelectAll {}
}
params! {
    /// Deselect (keeps the selection for Reselect).
    pub struct SelectNone {}
}
params! {
    /// Reselect: bring back the selection from before the last Deselect.
    pub struct SelectReselect {}
}
params! {
    /// Invert selected area: S → 255 − S.
    pub struct SelectInvert {}
}
params! {
    /// Expand selected area (S-001).
    pub struct SelectExpand {
        w: u32 = key "Expansion width in px",
        shape: Option<MorphShape> = opt "square (default) or round",
    }
}
params! {
    /// Shrink selected area (S-001).
    pub struct SelectShrink {
        w: u32 = key "Shrinking width in px",
        shape: Option<MorphShape> = opt "square (default) or round",
        canvas_edge: Option<bool> = opt "Shrink at edge of canvas (default true)",
    }
}
params! {
    /// Fill tool: fill the area around a canvas point with a colour (F-001). Settings left out
    /// take CSP's defaults for the sub tool: tolerance 10, connected, anti-aliased, opacity 100.
    pub struct FillArea {
        layer: LayerId = pos "Layer to paint (and to read with ref=layer)",
        x: u32 = key "Click x in canvas pixels",
        y: u32 = key "Click y in canvas pixels",
        color: Rgba8 = key "Colour #rrggbb",
        tolerance: Option<f64> = opt "Tolerance in percent (default 10)",
        connected: Option<bool> = opt "Apply to connected pixels only (default true)",
        close_gap: Option<u8> = opt "Close gap level 0 (off, default) ..= 5",
        aa: Option<bool> = opt "Anti-aliasing (default true)",
        scale: Option<i64> = opt "Area scaling in px (default 0 = off; negative shrinks)",
        scale_mode: Option<brushye_core::fill::ScaleMode> = opt "Scaling mode: rectangle, round or darkest (To darkest pixel, the default)",
        opacity: Option<u8> = opt "Opacity in percent (default 100)",
        refer: Option<FillRef> = opt "layer (default: the layer itself) or all (the visible image)",
    }
}
params! {
    /// Enclose and fill: fill every region lying entirely inside a lasso (F-002).
    pub struct FillEnclose {
        layer: LayerId = pos "Layer to paint (and to read with refer=layer)",
        points: Points = key "Lasso input events [x,y;…] in canvas pixels, press point first; the lasso is their B-spline, closed (F-002 run 3)",
        color: Rgba8 = key "Colour #rrggbb",
        stabilization: Option<u8> = opt "Stabilization 0..=100 applied to the events (default 0; CSP applies it to pen input only)",
        target: Option<brushye_core::fill::EncloseTarget> = opt "Target color (default semi_as_transparent, CSP's default)",
        tolerance: Option<f64> = opt "Tolerance in percent (default 30)",
        aa: Option<bool> = opt "Anti-aliasing (default false)",
        scale: Option<i64> = opt "Area scaling in px (default 10; 0 = off; negative shrinks)",
        scale_mode: Option<brushye_core::fill::ScaleMode> = opt "Scaling mode (default darkest)",
        opacity: Option<u8> = opt "Opacity in percent (default 100)",
        refer: Option<FillRef> = opt "all (default: the visible image) or layer",
    }
}
params! {
    /// Paint unfilled area: fill every region lying entirely inside a brush stroke (F-003).
    pub struct FillUnfilled {
        layer: LayerId = pos "Layer to paint (and to read with refer=layer)",
        color: Rgba8 = key "Colour #rrggbb",
        size: f64 = key "Brush size (diameter) in px (the sub tool's default 20)",
        samples: Samples = key "Pointer samples (b64:… or [x,y,p,t;…]) in canvas pixels",
        gap: Option<Gap> = opt "Dab gap: narrow, normal (default), wide, or percent of size",
        target: Option<brushye_core::fill::EncloseTarget> = opt "Target color (default semi_as_transparent, CSP's default)",
        tolerance: Option<f64> = opt "Tolerance in percent (default 80)",
        aa: Option<bool> = opt "Anti-aliasing (default false)",
        scale: Option<i64> = opt "Area scaling in px (default 0 = off; negative shrinks)",
        scale_mode: Option<brushye_core::fill::ScaleMode> = opt "Scaling mode (default darkest)",
        opacity: Option<u8> = opt "Opacity in percent (default 100)",
        refer: Option<FillRef> = opt "all (default: the visible image) or layer",
    }
}
params! {
    /// Auto select: select the area around a canvas point (F-001; same area as the Fill tool).
    pub struct SelectAuto {
        layer: LayerId = pos "Layer to read with ref=layer",
        x: u32 = key "Click x in canvas pixels",
        y: u32 = key "Click y in canvas pixels",
        tolerance: Option<f64> = opt "Tolerance in percent (default 10)",
        connected: Option<bool> = opt "Apply to connected pixels only (default true)",
        close_gap: Option<u8> = opt "Close gap level 0 (off, default) ..= 5",
        aa: Option<bool> = opt "Anti-aliasing (default true)",
        scale: Option<i64> = opt "Area scaling in px (default 0 = off; negative shrinks)",
        scale_mode: Option<brushye_core::fill::ScaleMode> = opt "Scaling mode: rectangle, round or darkest (To darkest pixel, the default)",
        refer: Option<FillRef> = opt "layer (default) or all (the visible image)",
        mode: Option<SelectMode> = opt "new (default), add, subtract or intersect",
    }
}
params! {
    /// Select > Blur border: soften the selection's edge (S-001 run 6).
    pub struct SelectBlur {
        strength: f64 = key "Strength in px (0.5 and up)",
    }
}
params! {
    /// Layer > Selection from Layer: the layer's pixel alpha as a selection, or combined with
    /// the current one (S-001 run 6).
    pub struct SelectFromLayer {
        layer: LayerId = pos "Raster layer",
        op: Option<LayerSelOp> = opt "create (default), add, delete or overlap (Select Overlapping Area)",
    }
}
params! {
    /// Move the selection (dragging inside it, S-001).
    pub struct SelectMove {
        dx: i64 = key "Horizontal offset in px",
        dy: i64 = key "Vertical offset in px",
    }
}
params! {
    /// Edit > Fill: fill the selection (or the whole layer) with a colour; alpha = S on an
    /// empty layer (S-001).
    pub struct EditFill {
        layer: LayerId = pos "Raster layer",
        color: Rgba8 = key "Colour #rrggbb",
    }
}
params! {
    /// Edit > Delete: clear the selection (or the whole layer); alpha becomes
    /// floor(a·(255 − S)/255) (S-001).
    pub struct EditDelete {
        layer: LayerId = pos "Raster layer",
    }
}
params! {
    /// The selection: whether there is one, its bounding box and Reselect availability.
    pub struct QSelection {}
}
params! {
    /// The selection value at canvas coordinates (0 = not selected, 255 = selected).
    pub struct QSelectionPixel {
        x: u32 = key "x",
        y: u32 = key "y",
    }
}
params! {
    /// Set the global pen pressure adjustment curve (application setting, like CSP's
    /// File > Pen Pressure Settings): applied to raw pen pressure before every brush setting's
    /// own curve (spec B-006). Journaled, not undoable, kept across `doc.new`.
    pub struct InputPressureCurve {
        points: Curve = key "Interior control points [x,y;…]; [] = linear",
    }
}
params! {
    /// The global pen pressure adjustment curve.
    pub struct QInputPressureCurve {}
}
params! {
    /// Select a tool in the tool column.
    pub struct ToolSelect {
        tool: Tool = pos "Tool",
    }
}
params! {
    /// Select a sub tool (and its tool and group). A locked sub tool that is left reverts to its
    /// locked settings (U-010).
    pub struct SubtoolSelect {
        tool: Option<Tool> = opt "Tool (default: the current tool)",
        group: Option<u32> = opt "Group tab, from 0 (default: the current group)",
        item: Option<u32> = opt "Sub tool in the group, from 0 (default: the selected one)",
    }
}
params! {
    /// Change a sub tool's name or settings. Settings left out keep their value; `clear` turns
    /// settings off (back to their default), e.g. `clear=size_pressure,hardness`.
    pub struct SubtoolSet {
        tool: Option<Tool> = opt "Tool (default: the current tool)",
        group: Option<u32> = opt "Group tab, from 0 (default: the current group)",
        item: Option<u32> = opt "Sub tool in the group, from 0 (default: the selected one)",
        name: Option<String> = opt "New name",
        size: Option<f64> = opt "Brush size (diameter) in px",
        aa: Option<Aa> = opt "Anti-aliasing",
        hardness: Option<u8> = opt "Tip hardness 1 (soft) ..= 5 (hard)",
        opacity: Option<u8> = opt "Ink opacity in percent",
        density: Option<u8> = opt "Brush density in percent",
        ink: Option<InkMode> = opt "Ink blending mode",
        gap: Option<Gap> = opt "Dab gap: narrow, normal, wide, or percent of size",
        density_by_gap: Option<bool> = opt "Adjust brush density by gap",
        thickness: Option<u8> = opt "Tip thickness in percent, 1..=100",
        direction: Option<TipAxis> = opt "Axis that thickness shortens",
        angle: Option<f64> = opt "Tip angle in degrees, clockwise",
        angle_line: Option<bool> = opt "Tip angle follows the direction of the line",
        stabilization: Option<u8> = opt "Stabilization 0..=100",
        size_pressure: Option<u8> = opt "Pen pressure controls size; minimum in percent",
        size_curve: Option<Curve> = opt "Size pressure curve points [x,y;…]",
        opacity_pressure: Option<u8> = opt "Pen pressure controls opacity; minimum in percent",
        opacity_curve: Option<Curve> = opt "Opacity pressure curve points [x,y;…]",
        density_pressure: Option<u8> = opt "Pen pressure controls density; minimum in percent",
        density_curve: Option<Curve> = opt "Density pressure curve points [x,y;…]",
        mode: Option<SelectMode> = opt "Selection mode (selection sub tools)",
        tolerance: Option<f64> = opt "Fill / Auto select tolerance in percent",
        close_gap: Option<u8> = opt "Fill / Auto select Close gap level 0..=5",
        connected: Option<bool> = opt "Fill / Auto select: apply to connected pixels only",
        scale: Option<i64> = opt "Fill / Auto select area scaling in px (0 = off)",
        scale_mode: Option<brushye_core::fill::ScaleMode> = opt "Fill / Auto select scaling mode",
        target: Option<brushye_core::fill::EncloseTarget> = opt "Enclose and fill target color",
        clear: Option<String> = opt "Comma-separated settings to turn off (not size)",
    }
}
params! {
    /// Add a sub tool at the end of a group and select it.
    pub struct SubtoolAdd {
        tool: Option<Tool> = opt "Tool (default: the current tool)",
        group: Option<u32> = opt "Group tab, from 0 (default: the current group)",
        name: String = key "Name",
        settings: Option<String> = opt "Settings as `key=value …` (default: the tool's first default sub tool)",
    }
}
params! {
    /// Copy a sub tool; the copy goes right after it and is selected.
    pub struct SubtoolCopy {
        tool: Option<Tool> = opt "Tool (default: the current tool)",
        group: Option<u32> = opt "Group tab, from 0 (default: the current group)",
        item: Option<u32> = opt "Sub tool in the group, from 0 (default: the selected one)",
        name: Option<String> = opt "Name of the copy (default: \"<name> 2\")",
    }
}
params! {
    /// Delete a sub tool (a group keeps at least one).
    pub struct SubtoolDelete {
        tool: Option<Tool> = opt "Tool (default: the current tool)",
        group: Option<u32> = opt "Group tab, from 0 (default: the current group)",
        item: Option<u32> = opt "Sub tool in the group, from 0 (default: the selected one)",
    }
}
params! {
    /// Move a sub tool to another position in its group.
    pub struct SubtoolMove {
        tool: Option<Tool> = opt "Tool (default: the current tool)",
        group: Option<u32> = opt "Group tab, from 0 (default: the current group)",
        item: Option<u32> = opt "Sub tool in the group, from 0 (default: the selected one)",
        to: u32 = key "New position, from 0",
    }
}
params! {
    /// Lock or unlock a sub tool: edits to a locked sub tool are undone when it is left.
    pub struct SubtoolLock {
        tool: Option<Tool> = opt "Tool (default: the current tool)",
        group: Option<u32> = opt "Group tab, from 0 (default: the current group)",
        item: Option<u32> = opt "Sub tool in the group, from 0 (default: the selected one)",
        locked: bool = key "Locked",
    }
}
params! {
    /// Reset a sub tool to its registered default, else its built-in default.
    pub struct SubtoolReset {
        tool: Option<Tool> = opt "Tool (default: the current tool)",
        group: Option<u32> = opt "Group tab, from 0 (default: the current group)",
        item: Option<u32> = opt "Sub tool in the group, from 0 (default: the selected one)",
    }
}
params! {
    /// Register a sub tool's current settings as its default (what reset restores).
    pub struct SubtoolSaveDefault {
        tool: Option<Tool> = opt "Tool (default: the current tool)",
        group: Option<u32> = opt "Group tab, from 0 (default: the current group)",
        item: Option<u32> = opt "Sub tool in the group, from 0 (default: the selected one)",
    }
}
params! {
    /// Show or hide a setting's row in the Tool property palette.
    pub struct SubtoolShowRow {
        tool: Option<Tool> = opt "Tool (default: the current tool)",
        group: Option<u32> = opt "Group tab, from 0 (default: the current group)",
        item: Option<u32> = opt "Sub tool in the group, from 0 (default: the selected one)",
        row: String = key "Row key (as in Sub Tool Detail, e.g. size, hardness, aa)",
        shown: bool = key "Shown in Tool property",
    }
}
params! {
    /// Add back the built-in sub tools missing from a tool.
    pub struct SubtoolAddDefaults {
        tool: Option<Tool> = opt "Tool (default: the current tool)",
    }
}
params! {
    /// Rename a sub tool group.
    pub struct SubtoolGroupSet {
        group: u32 = pos "Group tab, from 0",
        tool: Option<Tool> = opt "Tool (default: the current tool)",
        name: String = key "Name",
    }
}
params! {
    /// Set the colour of the main or sub chip.
    pub struct ColorSet {
        chip: Chip = pos "main or sub",
        color: Rgba8 = key "Colour #rrggbb",
    }
}
params! {
    /// Make a colour chip the drawing colour (transparent erases).
    pub struct ColorActive {
        chip: Chip = pos "main, sub or transparent",
    }
}
params! {
    /// Tools, the current tool and every tool's sub tool names.
    pub struct QTools {}
}
params! {
    /// One sub tool in full.
    pub struct QSubtool {
        tool: Option<Tool> = opt "Tool (default: the current tool)",
        group: Option<u32> = opt "Group tab, from 0 (default: the current group)",
        item: Option<u32> = opt "Sub tool in the group, from 0 (default: the selected one)",
    }
}
params! {
    /// The colour chips.
    pub struct QColor {}
}

impl SubtoolSet {
    /// The preset settings this command gives, in the preset's JSON form.
    pub fn settings_json(&self) -> serde_json::Map<String, Json> {
        let names: Vec<&str> = crate::brush_preset::BrushPreset::PARAMS
            .iter()
            .map(|p| p.name)
            .collect();
        self.write_json()
            .into_iter()
            .filter(|(k, _)| names.contains(&k.as_str()))
            .collect()
    }
}

params! {
    /// Undo one step.
    pub struct HistoryUndo {}
}
params! {
    /// Redo one step.
    pub struct HistoryRedo {}
}

params! {
    /// Document properties.
    pub struct QDocInfo {}
}
params! {
    /// All layers, bottom to top.
    pub struct QLayerList {}
}
params! {
    /// One layer's properties.
    pub struct QLayerGet {
        layer: LayerId = pos "Layer",
    }
}
params! {
    /// Flattened (rendered) pixel at canvas coordinates.
    pub struct QPixel {
        x: u32 = key "x",
        y: u32 = key "y",
    }
}
params! {
    /// A layer's own pixel at canvas coordinates.
    pub struct QLayerPixel {
        layer: LayerId = pos "Layer",
        x: u32 = key "x",
        y: u32 = key "y",
    }
}
params! {
    /// A layer's mask value at canvas coordinates (0 = hidden, 255 = revealed).
    pub struct QMaskPixel {
        layer: LayerId = pos "Layer or folder",
        x: u32 = key "x",
        y: u32 = key "y",
    }
}
params! {
    /// Deterministic content hash of the current document (hex).
    pub struct QDocHash {}
}
params! {
    /// Undo history: step labels and the current position.
    pub struct QHistory {}
}
params! {
    /// The command and query registry with parameter schemas.
    pub struct QCommands {}
}

/// Declares the command enum, the query enum and their registry tables.
macro_rules! registry {
    (
        commands { $( $C:ident ( $P:ident ) = $name:literal v $ver:literal $class:ident $doc:literal ),* $(,)? }
        queries { $( $Q:ident ( $QP:ident ) = $qname:literal $qdoc:literal ),* $(,)? }
    ) => {
        #[derive(Clone, Debug, PartialEq)]
        pub enum Command { $( $C($P) ),* }

        #[derive(Clone, Debug, PartialEq)]
        pub enum Query { $( $Q($QP) ),* }

        /// Registry entry (schema) of a command or query.
        #[derive(Clone, Copy, Debug)]
        pub struct Spec {
            pub name: &'static str,
            pub version: u32,
            /// `None` for queries.
            pub class: Option<Class>,
            pub doc: &'static str,
            pub params: &'static [ParamSpec],
        }

        pub const COMMANDS: &[Spec] = &[
            $( Spec { name: $name, version: $ver, class: Some(Class::$class), doc: $doc, params: $P::PARAMS } ),*
        ];
        pub const QUERIES: &[Spec] = &[
            $( Spec { name: $qname, version: 1, class: None, doc: $qdoc, params: $QP::PARAMS } ),*
        ];

        impl Command {
            pub fn name(&self) -> &'static str {
                match self { $( Command::$C(_) => $name ),* }
            }
            pub fn version(&self) -> u32 {
                match self { $( Command::$C(_) => $ver ),* }
            }
            pub fn class(&self) -> Class {
                match self { $( Command::$C(_) => Class::$class ),* }
            }
            /// Typed command from a parsed line at the current version (see `migrate`).
            pub fn from_raw(raw: &RawLine) -> Result<Self, String> {
                let mut r = Reader::new(&raw.positional, &raw.named);
                let cmd = match raw.name.as_str() {
                    $( $name => Command::$C($P::read(&mut r)?), )*
                    other => return Err(format!("unknown command '{other}'")),
                };
                r.finish()?;
                Ok(cmd)
            }
            /// Parsed-line form (no sequence number).
            pub fn to_raw(&self) -> RawLine {
                let mut raw = RawLine { name: self.name().into(), version: self.version(), ..Default::default() };
                match self { $( Command::$C(p) => p.write(&mut raw.positional, &mut raw.named) ),* }
                raw
            }
            pub fn to_json(&self) -> Json {
                let args = match self { $( Command::$C(p) => p.write_json() ),* };
                json!({ "cmd": self.name(), "v": self.version(), "args": args })
            }
            pub fn from_json(j: &Json) -> Result<Self, String> {
                let name = j.get("cmd").and_then(Json::as_str).ok_or("missing \"cmd\"")?;
                let v = j.get("v").and_then(Json::as_u64).unwrap_or(1);
                let empty = serde_json::Map::new();
                let args = match j.get("args") { Some(Json::Object(m)) => m, None => &empty, Some(_) => return Err("\"args\" must be an object".into()) };
                let mut r = JsonReader::new(args);
                let cmd = match name {
                    $( $name => {
                        if v != $ver { return Err(format!("{name}: JSON form is version {v}, this Brushye has version {}", $ver)); }
                        Command::$C($P::read_json(&mut r)?)
                    } )*
                    other => return Err(format!("unknown command '{other}'")),
                };
                r.finish()?;
                Ok(cmd)
            }
        }

        impl Query {
            pub fn name(&self) -> &'static str {
                match self { $( Query::$Q(_) => $qname ),* }
            }
            pub fn from_raw(raw: &RawLine) -> Result<Self, String> {
                if raw.version != 1 {
                    return Err(format!("query {} has no version {}", raw.name, raw.version));
                }
                let mut r = Reader::new(&raw.positional, &raw.named);
                let q = match raw.name.as_str() {
                    $( $qname => Query::$Q($QP::read(&mut r)?), )*
                    other => return Err(format!("unknown query '{other}'")),
                };
                r.finish()?;
                Ok(q)
            }
            pub fn to_raw(&self) -> RawLine {
                let mut raw = RawLine { name: self.name().into(), version: 1, query: true, ..Default::default() };
                match self { $( Query::$Q(p) => p.write(&mut raw.positional, &mut raw.named) ),* }
                raw
            }
        }
    };
}

registry! {
    commands {
        DocNew(DocNew) = "doc.new" v 1 Session "New document; starts a fresh history",
        LayerNew(LayerNew) = "layer.new" v 1 Undoable "New layer with a caller-chosen id",
        LayerDelete(LayerDelete) = "layer.delete" v 1 Undoable "Delete a layer",
        LayerMove(LayerMove) = "layer.move" v 1 Undoable "Move a layer above another layer, or to the top or bottom",
        LayerSet(LayerSet) = "layer.set" v 1 Undoable "Set layer properties (name, opacity, blend, locks)",
        LayerFill(LayerFill) = "layer.fill" v 1 Undoable "Fill a layer with one colour",
        LayerPixels(LayerPixels) = "layer.pixels" v 1 Undoable "Write raw RGBA pixels into a layer",
        BrushStroke(BrushStroke) = "brush.stroke" v 1 Undoable "Paint one brush stroke from input samples",
        LayerGroup(LayerGroup) = "layer.group" v 1 Undoable "Put a layer in a new folder (Ctrl+G)",
        LayerUngroup(LayerUngroup) = "layer.ungroup" v 1 Undoable "Remove a folder, keeping its contents",
        MaskNew(MaskNew) = "mask.new" v 1 Undoable "Create a layer mask (Create layer mask)",
        MaskDelete(MaskDelete) = "mask.delete" v 1 Undoable "Delete a layer's mask",
        MaskApply(MaskApply) = "mask.apply" v 1 Undoable "Apply a mask to its layer's pixels",
        MaskSet(MaskSet) = "mask.set" v 1 Undoable "Enable, link or show the area of a mask",
        SelectRect(SelectRect) = "select.rect" v 1 Undoable "Select a rectangle",
        SelectEllipse(SelectEllipse) = "select.ellipse" v 1 Undoable "Select an ellipse",
        SelectPolygon(SelectPolygon) = "select.polygon" v 1 Undoable "Select a polygon (lasso, polyline)",
        SelectAll(SelectAll) = "select.all" v 1 Undoable "Select All",
        SelectNone(SelectNone) = "select.none" v 1 Undoable "Deselect",
        SelectReselect(SelectReselect) = "select.reselect" v 1 Undoable "Reselect the last deselected selection",
        SelectInvert(SelectInvert) = "select.invert" v 1 Undoable "Invert selected area",
        SelectExpand(SelectExpand) = "select.expand" v 1 Undoable "Expand selected area",
        SelectShrink(SelectShrink) = "select.shrink" v 1 Undoable "Shrink selected area",
        SelectMove(SelectMove) = "select.move" v 1 Undoable "Move the selection",
        SelectBlur(SelectBlur) = "select.blur" v 1 Undoable "Blur the selection's border",
        SelectAuto(SelectAuto) = "select.auto" v 1 Undoable "Auto select the area around a point",
        FillArea(FillArea) = "fill.area" v 1 Undoable "Fill the area around a point (Fill tool)",
        FillEnclose(FillEnclose) = "fill.enclose" v 1 Undoable "Fill the regions inside a lasso (Enclose and fill)",
        FillUnfilled(FillUnfilled) = "fill.unfilled" v 1 Undoable "Fill the regions inside a brush stroke (Paint unfilled area)",
        SelectFromLayer(SelectFromLayer) = "select.from_layer" v 1 Undoable "Selection from a layer's transparency",
        EditFill(EditFill) = "edit.fill" v 1 Undoable "Fill the selection (or layer) with a colour",
        EditDelete(EditDelete) = "edit.delete" v 1 Undoable "Clear the selection (or layer)",
        LayerShow(LayerShow) = "layer.show" v 1 AppState "Show or hide a layer (not undoable, as in CSP)",
        LayerExpand(LayerExpand) = "layer.expand" v 1 AppState "Open or close a folder in the Layer palette",
        LayerSelect(LayerSelect) = "layer.select" v 1 AppState "Make a layer the active layer",
        InputPressureCurve(InputPressureCurve) = "input.pressure_curve" v 1 AppState "Set the global pen pressure adjustment curve",
        ToolSelect(ToolSelect) = "tool.select" v 1 Tool "Select a tool",
        SubtoolSelect(SubtoolSelect) = "subtool.select" v 1 Tool "Select a sub tool",
        SubtoolSet(SubtoolSet) = "subtool.set" v 1 Tool "Change a sub tool's name or settings",
        SubtoolAdd(SubtoolAdd) = "subtool.add" v 1 Tool "Add a sub tool to a group",
        SubtoolCopy(SubtoolCopy) = "subtool.copy" v 1 Tool "Copy a sub tool",
        SubtoolDelete(SubtoolDelete) = "subtool.delete" v 1 Tool "Delete a sub tool",
        SubtoolMove(SubtoolMove) = "subtool.move" v 1 Tool "Move a sub tool within its group",
        SubtoolLock(SubtoolLock) = "subtool.lock" v 1 Tool "Lock or unlock a sub tool",
        SubtoolReset(SubtoolReset) = "subtool.reset" v 1 Tool "Reset a sub tool to its default",
        SubtoolSaveDefault(SubtoolSaveDefault) = "subtool.save_default" v 1 Tool "Register a sub tool's settings as its default",
        SubtoolShowRow(SubtoolShowRow) = "subtool.show_row" v 1 Tool "Show or hide a Tool property row",
        SubtoolAddDefaults(SubtoolAddDefaults) = "subtool.add_defaults" v 1 Tool "Add back missing built-in sub tools",
        SubtoolGroupSet(SubtoolGroupSet) = "subtool.group_set" v 1 Tool "Rename a sub tool group",
        ColorSet(ColorSet) = "color.set" v 1 Tool "Set the main or sub colour",
        ColorActive(ColorActive) = "color.active" v 1 Tool "Choose the active colour chip",
        HistoryUndo(HistoryUndo) = "history.undo" v 1 History "Undo one step",
        HistoryRedo(HistoryRedo) = "history.redo" v 1 History "Redo one step",
    }
    queries {
        DocInfo(QDocInfo) = "doc.info" "Document size, dpi, paper and active layer",
        LayerList(QLayerList) = "layer.list" "All layers, bottom to top",
        LayerGet(QLayerGet) = "layer.get" "One layer's properties",
        DocHash(QDocHash) = "doc.hash" "Deterministic content hash of the document (hex)",
        Pixel(QPixel) = "pixel" "Rendered pixel at x, y (RGBA)",
        LayerPixel(QLayerPixel) = "layer.pixel" "A layer's own pixel at x, y (RGBA)",
        MaskPixel(QMaskPixel) = "layer.mask_pixel" "A layer's mask value at x, y",
        Selection(QSelection) = "selection" "Whether something is selected, its bounds, and Reselect",
        SelectionPixel(QSelectionPixel) = "selection.pixel" "The selection value at x, y",
        History(QHistory) = "history" "Undo history: step labels and current position",
        InputPressureCurve(QInputPressureCurve) = "input.pressure_curve" "The global pen pressure curve's control points",
        Tools(QTools) = "tools" "The current tool and every tool's sub tools",
        Subtool(QSubtool) = "subtool" "One sub tool's name, settings, rows and lock",
        Color(QColor) = "color" "The main and sub colours and the active chip",
        Commands(QCommands) = "commands" "Command and query registry with parameter schemas",
    }
}

/// Registry as JSON (for `?commands` and the MCP server).
pub fn registry_json() -> Json {
    let spec = |s: &Spec| {
        json!({
            "name": s.name,
            "version": s.version,
            "class": s.class.map(|c| format!("{c:?}").to_lowercase()),
            "params": s.params.iter().map(|p| json!({
                "name": p.name, "kind": p.kind, "doc": p.doc,
                "mode": match p.mode { Mode::Pos => "positional", Mode::Key => "required", Mode::Opt => "optional" },
            })).collect::<Vec<_>>(),
        })
    };
    json!({
        "commands": COMMANDS.iter().map(spec).collect::<Vec<_>>(),
        "queries": QUERIES.iter().map(spec).collect::<Vec<_>>(),
    })
}

/// The command reference as Markdown (`docs/commands.md`, `brushye commands --markdown`).
pub fn reference_markdown() -> String {
    let mut out = String::from(
        "# Brushye command reference\n\nGenerated from the command registry by `brushye commands --markdown`; a test keeps this\nfile current. Syntax: `docs/command-format.md`. Classes: **session** starts a new document\nand history, **undoable** is one undo step, **appstate** is journaled but not undoable,\n**history** is undo/redo, **tool** changes tools, sub tools and colours (not undoable, not\njournaled).\n",
    );
    let table = |out: &mut String, s: &Spec| {
        if s.params.is_empty() {
            out.push_str("No parameters.\n");
            return;
        }
        out.push_str("| Parameter | Kind | Form | Description |\n|---|---|---|---|\n");
        for p in s.params {
            let form = match p.mode {
                Mode::Pos => "positional",
                Mode::Key => "required `key=`",
                Mode::Opt => "optional `key=`",
            };
            out.push_str(&format!(
                "| `{}` | {} | {} | {} |\n",
                p.name,
                p.kind.replace('|', "\\|"),
                form,
                p.doc.replace('|', "\\|")
            ));
        }
    };
    out.push_str("\n## Commands\n");
    for s in COMMANDS {
        let class = s
            .class
            .map(|c| format!("{c:?}").to_lowercase())
            .unwrap_or_default();
        out.push_str(&format!(
            "\n### `{}` (v{}, {class})\n{}.\n\n",
            s.name, s.version, s.doc
        ));
        table(&mut out, s);
    }
    out.push_str(
        "\n## Queries\nQueries start with `?` and return JSON; they never change state.\n",
    );
    for s in QUERIES {
        out.push_str(&format!("\n### `?{}`\n{}.\n\n", s.name, s.doc));
        table(&mut out, s);
    }
    out
}

/// A migration step: rewrites a line from `from` to `from + 1` for one command.
pub type Migration = (&'static str, u32, fn(RawLine) -> Result<RawLine, String>);

/// All migrations (none yet: every command is at version 1).
pub const MIGRATIONS: &[Migration] = &[];

/// Current version of a registered command.
pub fn current_version(name: &str) -> Option<u32> {
    COMMANDS.iter().find(|s| s.name == name).map(|s| s.version)
}

/// Bring a parsed line to its command's current version (D-011).
pub fn migrate(raw: RawLine) -> Result<RawLine, String> {
    migrate_with(MIGRATIONS, current_version, raw)
}

/// Migration engine, parameterised for tests.
pub fn migrate_with(
    table: &[Migration],
    current: fn(&str) -> Option<u32>,
    mut raw: RawLine,
) -> Result<RawLine, String> {
    let now = current(&raw.name).ok_or_else(|| format!("unknown command '{}'", raw.name))?;
    if raw.version > now {
        return Err(format!(
            "{} version {} is newer than this Brushye supports (version {now})",
            raw.name, raw.version
        ));
    }
    while raw.version < now {
        let (name, from) = (raw.name.clone(), raw.version);
        let step = table
            .iter()
            .find(|m| m.0 == name && m.1 == from)
            .ok_or_else(|| format!("no migration for {name} from version {from}"))?;
        raw = (step.2)(raw)?;
        raw.version = from + 1;
    }
    Ok(raw)
}
