//! Layer masks (D-023, L-008): an 8-bit alpha-scale value per pixel, 255 = reveal, 0 = hide.
//!
//! Storage is sparse like layer pixels: 256×256 one-byte tiles, `Arc`-shared between document
//! versions (D-013). Pixels without a stored tile have the mask's `background` value, so a
//! reveal-all or hide-all mask (Create layer mask / Mask Outside Selection with no selection,
//! U-014) stores nothing.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::document::{Layer, LayerKind};
use crate::ids::LayerId;
use crate::tile::{TILE_SIZE, TileCoord};

const MASK_TILE: usize = TILE_SIZE * TILE_SIZE;

/// One 256×256 tile of mask values, row-major.
#[derive(Clone, PartialEq, Eq)]
pub struct MaskTile {
    pub v: Box<[u8]>,
}

impl std::fmt::Debug for MaskTile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MaskTile({} bytes)", self.v.len())
    }
}

impl MaskTile {
    fn filled(v: u8) -> Self {
        Self {
            v: vec![v; MASK_TILE].into_boxed_slice(),
        }
    }
}

/// A layer's or folder's mask.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mask {
    /// Value of every pixel without a stored tile.
    pub background: u8,
    tiles: BTreeMap<TileCoord, Arc<MaskTile>>,
    /// Enable Mask: a disabled mask is ignored when rendering (L-008).
    pub enabled: bool,
    /// Link Mask to Layer (moves with the layer; no transform tools yet).
    pub linked: bool,
    /// Show Mask Area: the canvas display tints hidden areas (display only, but a History
    /// step, H-002).
    pub show_area: bool,
}

impl Mask {
    /// A new mask, every pixel `background`; enabled and linked, area not shown (U-014).
    pub fn new(background: u8) -> Self {
        Self {
            background,
            tiles: BTreeMap::new(),
            enabled: true,
            linked: true,
            show_area: false,
        }
    }

    pub fn tile(&self, c: TileCoord) -> Option<&Arc<MaskTile>> {
        self.tiles.get(&c)
    }

    /// Stored tiles in sorted order (D-012).
    pub fn tiles(&self) -> impl Iterator<Item = (&TileCoord, &Arc<MaskTile>)> {
        self.tiles.iter()
    }

    /// Mutable tile, created filled with the background and cloned if shared.
    pub fn tile_mut(&mut self, c: TileCoord) -> &mut MaskTile {
        let bg = self.background;
        Arc::make_mut(
            self.tiles
                .entry(c)
                .or_insert_with(|| Arc::new(MaskTile::filled(bg))),
        )
    }

    /// Put a whole tile (loaders).
    pub fn put_tile(&mut self, c: TileCoord, v: Box<[u8]>) {
        if v.len() == MASK_TILE {
            self.tiles.insert(c, Arc::new(MaskTile { v }));
        }
    }

    /// Value at canvas (`x`, `y`).
    pub fn value(&self, x: u32, y: u32) -> u8 {
        let c = (
            (x as usize / TILE_SIZE) as i32,
            (y as usize / TILE_SIZE) as i32,
        );
        self.tiles.get(&c).map_or(self.background, |t| {
            t.v[(y as usize % TILE_SIZE) * TILE_SIZE + x as usize % TILE_SIZE]
        })
    }

    /// Drop tiles equal to the background everywhere.
    pub fn prune(&mut self) {
        let bg = self.background;
        self.tiles.retain(|_, t| t.v.iter().any(|&v| v != bg));
    }

    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }

    /// The mask as a raster layer whose alpha is the mask value (black colour), covering the
    /// canvas: brushes paint masks through it (any opaque ink raises the value, the eraser
    /// lowers it; L-008).
    pub fn to_layer(&self, bounds: (u32, u32)) -> Layer {
        let mut l = Layer::new(LayerId(0), "mask", LayerKind::Raster);
        let (tw, th) = (
            bounds.0.div_ceil(TILE_SIZE as u32) as i32,
            bounds.1.div_ceil(TILE_SIZE as u32) as i32,
        );
        for ty in 0..th {
            for tx in 0..tw {
                let t = l.tiles.get_mut((tx, ty));
                let src = self.tiles.get(&(tx, ty));
                for (i, p) in t.px.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                    p[3] = src.map_or(self.background, |s| s.v[i]);
                }
            }
        }
        l
    }

    /// Take the values back from a layer made by [`Mask::to_layer`].
    pub fn set_from_layer(&mut self, l: &Layer, bounds: (u32, u32)) {
        let (tw, th) = (
            bounds.0.div_ceil(TILE_SIZE as u32) as i32,
            bounds.1.div_ceil(TILE_SIZE as u32) as i32,
        );
        for ty in 0..th {
            for tx in 0..tw {
                let src = l.tiles.get((tx, ty));
                let t = self.tile_mut((tx, ty));
                for (i, v) in t.v.iter_mut().enumerate() {
                    *v = src.map_or(0, |s| s.px[i * 4 + 3]);
                }
            }
        }
        self.prune();
    }

    /// Pixel alpha with this mask applied: `floor(a · M / 255)` (L-008).
    pub fn apply(a: u8, m: u8) -> u8 {
        ((u32::from(a) * u32::from(m)) / 255) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_tiles_and_round_trip_through_a_layer() {
        let mut m = Mask::new(255);
        assert_eq!(m.value(1000, 5), 255);
        m.tile_mut((1, 0)).v[0] = 7;
        assert_eq!(m.value(256, 0), 7);
        assert_eq!(m.value(257, 0), 255);
        let l = m.to_layer((600, 300));
        let mut n = Mask::new(255);
        n.set_from_layer(&l, (600, 300));
        assert_eq!(n, m);
        m.tile_mut((1, 0)).v[0] = 255;
        m.prune();
        assert_eq!(m.tile_count(), 0);
        assert_eq!(Mask::apply(200, 128), 100);
    }
}
