//! Uniform-grid broad phase for the drawing board.
//!
//! The board used to scan every entity for every cursor move: hover, object
//! snap, nearest-point snap, box selection and the visible-set build all paid
//! O(n). That is fine for a small part but an imported DXF with tens of
//! thousands of segments turns each mouse message into milliseconds of work.
//!
//! This index follows the same idea as OpenCADStudio's per-view interaction
//! index (a compact grid/BVH broad phase, with long or straddling items kept in
//! an always-checked overflow set): entities are placed in every cell their
//! bounding box touches, queries only visit overlapping cells, and an entity
//! that covers too many cells (a long line across the plate) is stored once in
//! `big` rather than being copied hundreds of times. Results are deduplicated
//! with a reusable stamp array, so a cursor query allocates nothing.
//!
//! The index is only an accelerator. Exact distance tests, trimming, snapping
//! and intersections still run on the real geometry.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use crate::model::{BBox, Entity, Pt};

/// Aim for a handful of entities per cell: fewer cells means faster rebuilds,
/// more means less exact testing per query.
const TARGET_PER_CELL: usize = 8;
/// Upper bound per axis. 512 x 512 cells is 6 MB of empty vectors at worst and
/// still resolves a 100k-entity plate to a few entities per cell.
const MAX_CELLS_AXIS: u32 = 512;
/// An entity covering more cells than this is treated as oversized and always
/// tested. Without the cap a diagonal 10 m line would be inserted into
/// thousands of cells on every rebuild.
const MAX_ENTRY_CELLS: u64 = 128;

#[derive(Clone, Copy, Debug)]
struct Loc {
    c0: u32,
    r0: u32,
    c1: u32,
    r1: u32,
    big: bool,
    slot: u32,
}

#[derive(Debug)]
pub struct SpatialIndex {
    geom_rev: u64,
    bounds: BBox,
    cell_w: f32,
    cell_h: f32,
    cols: u32,
    rows: u32,
    cells: Vec<Vec<u32>>,
    big: Vec<u32>,
    loc: Vec<Option<Loc>>,
    marks: RefCell<Vec<u32>>,
    epoch: Cell<u32>,
}

impl SpatialIndex {
    pub fn build(entities: &[Entity], bounds: BBox, geom_rev: u64) -> Self {
        let n = entities.len();
        let w = bounds.width().max(1e-3);
        let h = bounds.height().max(1e-3);
        let target = (n / TARGET_PER_CELL).max(1) as f32;
        let aspect = (w / h).clamp(0.02, 50.0);
        let mut cols = (target * aspect).sqrt().round().max(1.0) as u32;
        let mut rows = (target / cols.max(1) as f32).round().max(1.0) as u32;
        cols = cols.min(MAX_CELLS_AXIS).max(1);
        rows = rows.min(MAX_CELLS_AXIS).max(1);
        let cell_w = w / cols as f32;
        let cell_h = h / rows as f32;
        let mut idx = Self {
            geom_rev,
            bounds,
            cell_w,
            cell_h,
            cols,
            rows,
            cells: vec![Vec::new(); (cols * rows) as usize],
            big: Vec::new(),
            loc: vec![None; n],
            marks: RefCell::new(Vec::new()),
            epoch: Cell::new(0),
        };
        for (i, e) in entities.iter().enumerate() {
            let loc = idx.insert(i as u32, e.bbox());
            idx.loc[i] = loc;
        }
        idx
    }

    pub fn geom_rev(&self) -> u64 {
        self.geom_rev
    }

    pub fn set_geom_rev(&mut self, rev: u64) {
        self.geom_rev = rev;
    }

    fn cell_range(&self, bb: BBox) -> Option<(u32, u32, u32, u32, u64)> {
        if bb.max.x < self.bounds.min.x
            || bb.max.y < self.bounds.min.y
            || bb.min.x > self.bounds.max.x
            || bb.min.y > self.bounds.max.y
        {
            return None;
        }
        let c0f = ((bb.min.x - self.bounds.min.x) / self.cell_w).floor();
        let r0f = ((bb.min.y - self.bounds.min.y) / self.cell_h).floor();
        let c1f = ((bb.max.x - self.bounds.min.x) / self.cell_w).floor();
        let r1f = ((bb.max.y - self.bounds.min.y) / self.cell_h).floor();
        let c0 = c0f.max(0.0).min((self.cols - 1) as f32) as u32;
        let r0 = r0f.max(0.0).min((self.rows - 1) as f32) as u32;
        let c1 = c1f.max(0.0).min((self.cols - 1) as f32) as u32;
        let r1 = r1f.max(0.0).min((self.rows - 1) as f32) as u32;
        let span = (c1 - c0 + 1) as u64 * (r1 - r0 + 1) as u64;
        Some((c0, r0, c1, r1, span))
    }

    fn insert(&mut self, e: u32, bb: BBox) -> Option<Loc> {
        let Some((c0, r0, c1, r1, span)) = self.cell_range(bb) else {
            self.big.push(e);
            return Some(Loc {
                c0: 0,
                r0: 0,
                c1: 0,
                r1: 0,
                big: true,
                slot: (self.big.len() - 1) as u32,
            });
        };
        if span > MAX_ENTRY_CELLS {
            self.big.push(e);
            return Some(Loc {
                c0: 0,
                r0: 0,
                c1: 0,
                r1: 0,
                big: true,
                slot: (self.big.len() - 1) as u32,
            });
        }
        for r in r0..=r1 {
            let row = (r * self.cols) as usize;
            for c in c0..=c1 {
                self.cells[row + c as usize].push(e);
            }
        }
        Some(Loc {
            c0,
            r0,
            c1,
            r1,
            big: false,
            slot: 0,
        })
    }

    fn remove(&mut self, e: u32, loc: Loc) {
        if loc.big {
            let slot = loc.slot as usize;
            if slot < self.big.len() && self.big[slot] == e {
                self.big.swap_remove(slot);
                if let Some(moved) = self.big.get(slot).copied() {
                    if let Some(Some(l)) = self.loc.get_mut(moved as usize) {
                        l.slot = slot as u32;
                    }
                }
            }
            return;
        }
        for r in loc.r0..=loc.r1 {
            let row = (r * self.cols) as usize;
            for c in loc.c0..=loc.c1 {
                let cell = &mut self.cells[row + c as usize];
                if let Some(pos) = cell.iter().position(|&x| x == e) {
                    cell.swap_remove(pos);
                }
            }
        }
    }

    /// Move one entity after a transform. Only the moved entity is re-inserted,
    /// so dragging a selection on a big drawing does not rebuild the whole
    /// grid every mouse message.
    pub fn update(&mut self, e: u32, bb: BBox) {
        let Some(loc) = self.loc.get(e as usize).copied().flatten() else {
            let loc = self.insert(e, bb);
            if let Some(slot) = self.loc.get_mut(e as usize) {
                *slot = loc;
            }
            return;
        };
        self.remove(e, loc);
        let loc = self.insert(e, bb);
        self.loc[e as usize] = loc;
    }

    /// Insert a newly appended entity without rebuilding the grid.
    pub fn add_entity(&mut self, e: u32, bb: BBox) {
        if self.loc.len() <= e as usize {
            self.loc.resize(e as usize + 1, None);
        }
        let loc = self.insert(e, bb);
        self.loc[e as usize] = loc;
    }

    /// Append the indices of entities whose cached bounds intersect `rect`.
    /// Duplicates from entities spanning several cells are removed with a
    /// generation stamp, so repeated queries never allocate.
    pub fn query_indices(&self, rect: BBox, entities: &[Entity], out: &mut Vec<u32>) {
        let n = self.loc.len();
        if n == 0 {
            return;
        }
        let mut marks = self.marks.borrow_mut();
        if marks.len() != n {
            marks.clear();
            marks.resize(n, 0);
        }
        let mut epoch = self.epoch.get().wrapping_add(1);
        if epoch == 0 {
            marks.iter_mut().for_each(|m| *m = 0);
            epoch = 1;
        }
        self.epoch.set(epoch);

        let push = |i: u32, marks: &mut [u32], out: &mut Vec<u32>| {
            let iu = i as usize;
            if iu < marks.len() && marks[iu] != epoch {
                marks[iu] = epoch;
                if entities.get(iu).is_some() {
                    out.push(i);
                }
            }
        };

        if let Some((c0, r0, c1, r1, _)) = self.cell_range(rect) {
            for r in r0..=r1 {
                let row = (r * self.cols) as usize;
                for c in c0..=c1 {
                    for &i in &self.cells[row + c as usize] {
                        if entities[i as usize].bbox().intersects(&rect) {
                            push(i, &mut marks, out);
                        }
                    }
                }
            }
        }
        for &i in &self.big {
            if entities[i as usize].bbox().intersects(&rect) {
                push(i, &mut marks, out);
            }
        }
    }

    /// Id projection used by tests.
    #[cfg(test)]
    pub fn query(&self, rect: BBox, entities: &[Entity], out: &mut Vec<u64>) {
        let mut found = Vec::new();
        self.query_indices(rect, entities, &mut found);
        out.extend(found.into_iter().map(|i| entities[i as usize].id));
    }

    /// The grid cells the given world rectangle covers, clipped to the index.
    pub fn cells_in(&self, rect: BBox) -> Option<(u32, u32, u32, u32)> {
        self.cell_range(rect).map(|(c0, r0, c1, r1, _)| (c0, r0, c1, r1))
    }

    pub fn cell(&self, c: u32, r: u32) -> &[u32] {
        &self.cells[(r * self.cols + c) as usize]
    }

    pub fn cell_bounds(&self, c: u32, r: u32) -> BBox {
        BBox {
            min: Pt::new(
                self.bounds.min.x + c as f32 * self.cell_w,
                self.bounds.min.y + r as f32 * self.cell_h,
            ),
            max: Pt::new(
                self.bounds.min.x + (c + 1) as f32 * self.cell_w,
                self.bounds.min.y + (r + 1) as f32 * self.cell_h,
            ),
        }
    }

    pub fn big(&self) -> &[u32] {
        &self.big
    }
}

/// Build a scratch id map for O(1) handle lookup.
pub fn build_id_map(entities: &[Entity]) -> HashMap<u64, usize> {
    let mut map = HashMap::with_capacity(entities.len());
    for (i, e) in entities.iter().enumerate() {
        map.insert(e.id, i);
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Kind;

    fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> BBox {
        BBox {
            min: Pt::new(x0, y0),
            max: Pt::new(x1, y1),
        }
    }

    #[test]
    fn query_matches_brute_force_after_updates() {
        let mut entities = Vec::new();
        for i in 0..500 {
            let x = (i % 25) as f32 * 3.0;
            let y = (i / 25) as f32 * 3.0;
            entities.push(Entity::new(
                i as u64 + 1,
                Kind::Line {
                    a: Pt::new(x, y),
                    b: Pt::new(x + 2.0, y + 1.0),
                },
            ));
        }
        let bounds = BBox {
            min: Pt::new(-2.0, -2.0),
            max: Pt::new(80.0, 62.0),
        };
        let mut idx = SpatialIndex::build(&entities, bounds, 1);
        let q = rect(10.0, 8.0, 25.0, 20.0);
        let mut got = Vec::new();
        idx.query(q, &entities, &mut got);
        let mut expected: Vec<u64> = entities
            .iter()
            .filter(|e| e.bbox().intersects(&q))
            .map(|e| e.id)
            .collect();
        got.sort_unstable();
        expected.sort_unstable();
        assert_eq!(got, expected);

        entities[0].translate(Pt::new(40.0, 10.0));
        idx.update(0, entities[0].bbox());
        let q = rect(35.0, 15.0, 60.0, 35.0);
        let mut got = Vec::new();
        idx.query(q, &entities, &mut got);
        let mut expected: Vec<u64> = entities
            .iter()
            .filter(|e| e.bbox().intersects(&q))
            .map(|e| e.id)
            .collect();
        got.sort_unstable();
        expected.sort_unstable();
        assert_eq!(got, expected);
    }

    #[test]
    fn oversized_entity_is_always_found() {
        let entities = vec![Entity::new(
            7,
            Kind::Line {
                a: Pt::new(-500.0, -500.0),
                b: Pt::new(500.0, 500.0),
            },
        )];
        let idx = SpatialIndex::build(&entities, rect(-10.0, -10.0, 10.0, 10.0), 1);
        let mut got = Vec::new();
        idx.query(rect(0.0, 0.0, 1.0, 1.0), &entities, &mut got);
        assert_eq!(got, vec![7]);
    }
}
