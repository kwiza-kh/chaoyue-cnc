//! Geometry model for the wire-EDM drawing board.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::f32::consts::{PI, TAU};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Pt {
    pub x: f32,
    pub y: f32,
}

impl Pt {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    pub fn add(self, o: Pt) -> Pt {
        Pt::new(self.x + o.x, self.y + o.y)
    }
    pub fn sub(self, o: Pt) -> Pt {
        Pt::new(self.x - o.x, self.y - o.y)
    }
    pub fn mul(self, f: f32) -> Pt {
        Pt::new(self.x * f, self.y * f)
    }
    pub fn dot(self, o: Pt) -> f32 {
        self.x * o.x + self.y * o.y
    }
    pub fn len(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
    pub fn dist(self, o: Pt) -> f32 {
        self.sub(o).len()
    }
    pub fn norm(self) -> Pt {
        let l = self.len();
        if l < 1e-9 {
            Pt::new(0.0, 0.0)
        } else {
            self.mul(1.0 / l)
        }
    }
    pub fn left(self) -> Pt {
        Pt::new(-self.y, self.x)
    }
    pub fn angle(self) -> f32 {
        self.y.atan2(self.x)
    }
    pub fn from_angle(a: f32, r: f32) -> Pt {
        Pt::new(a.cos() * r, a.sin() * r)
    }
    pub fn rotate(self, center: Pt, angle: f32) -> Pt {
        let d = self.sub(center);
        let (s, c) = angle.sin_cos();
        center.add(Pt::new(d.x * c - d.y * s, d.x * s + d.y * c))
    }
    pub fn lerp(self, o: Pt, t: f32) -> Pt {
        Pt::new(self.x + (o.x - self.x) * t, self.y + (o.y - self.y) * t)
    }
}

pub fn norm_angle(a: f32) -> f32 {
    a.rem_euclid(TAU)
}

pub fn angle_on_arc(a0: f32, sweep: f32, angle: f32) -> bool {
    if sweep.abs() >= TAU - 1e-4 {
        return true;
    }
    if sweep >= 0.0 {
        norm_angle(angle - a0) <= sweep + 1e-4
    } else {
        norm_angle(a0 - angle) <= -sweep + 1e-4
    }
}

pub fn dist_point_segment(p: Pt, a: Pt, b: Pt) -> f32 {
    let ab = b.sub(a);
    let l2 = ab.dot(ab);
    if l2 < 1e-12 {
        return p.dist(a);
    }
    let t = (p.sub(a).dot(ab) / l2).clamp(0.0, 1.0);
    p.dist(a.add(ab.mul(t)))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BBox {
    pub min: Pt,
    pub max: Pt,
}

impl BBox {
    pub fn new(p: Pt) -> Self {
        Self { min: p, max: p }
    }
    pub fn union_point(&mut self, p: Pt) {
        self.min.x = self.min.x.min(p.x);
        self.min.y = self.min.y.min(p.y);
        self.max.x = self.max.x.max(p.x);
        self.max.y = self.max.y.max(p.y);
    }
    pub fn union(&mut self, o: BBox) {
        self.union_point(o.min);
        self.union_point(o.max);
    }
    pub fn center(&self) -> Pt {
        Pt::new((self.min.x + self.max.x) * 0.5, (self.min.y + self.max.y) * 0.5)
    }
    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }
    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }
    pub fn contains(&self, p: Pt) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }
    pub fn intersects(&self, o: &BBox) -> bool {
        self.min.x <= o.max.x && self.max.x >= o.min.x && self.min.y <= o.max.y && self.max.y >= o.min.y
    }
    pub fn expand(&self, d: f32) -> BBox {
        BBox {
            min: Pt::new(self.min.x - d, self.min.y - d),
            max: Pt::new(self.max.x + d, self.max.y + d),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Kind {
    Point { p: Pt },
    Line { a: Pt, b: Pt },
    Circle { c: Pt, r: f32 },
    /// An arc always travels counter-clockwise from `a0` by `sweep` radians.
    Arc { c: Pt, r: f32, a0: f32, sweep: f32 },
}

#[derive(Clone, Debug)]
pub struct Entity {
    pub id: u64,
    pub kind: Kind,
    /// Cached world bounds. Every transform refreshes it, so drawing and
    /// picking no longer re-evaluate arc trigonometry for each frame or each
    /// mouse move.
    bbox: BBox,
}

impl Entity {
    pub fn new(id: u64, kind: Kind) -> Self {
        let bbox = bbox_of_kind(&kind);
        Self { id, kind, bbox }
    }
    pub fn start(&self) -> Option<Pt> {
        match &self.kind {
            Kind::Point { p } => Some(*p),
            Kind::Line { a, .. } => Some(*a),
            Kind::Arc { c, r, a0, .. } => Some(c.add(Pt::from_angle(*a0, *r))),
            Kind::Circle { .. } => None,
        }
    }
    pub fn end(&self) -> Option<Pt> {
        match &self.kind {
            Kind::Point { p } => Some(*p),
            Kind::Line { b, .. } => Some(*b),
            Kind::Arc { c, r, a0, sweep } => Some(c.add(Pt::from_angle(*a0 + *sweep, *r))),
            Kind::Circle { .. } => None,
        }
    }
    pub fn bbox(&self) -> BBox {
        self.bbox
    }
    fn refresh_bbox(&mut self) {
        self.bbox = bbox_of_kind(&self.kind);
    }
    pub fn length(&self) -> f32 {
        match &self.kind {
            Kind::Point { .. } => 0.0,
            Kind::Line { a, b } => a.dist(*b),
            Kind::Circle { r, .. } => TAU * r,
            Kind::Arc { r, sweep, .. } => r * sweep.abs(),
        }
    }
    pub fn hit(&self, p: Pt) -> f32 {
        match &self.kind {
            Kind::Point { p: q } => p.dist(*q),
            Kind::Line { a, b } => dist_point_segment(p, *a, *b),
            Kind::Circle { c, r } => (p.dist(*c) - r).abs(),
            Kind::Arc { c, r, a0, sweep } => {
                let d = p.dist(*c);
                let a = p.sub(*c).angle();
                if angle_on_arc(*a0, *sweep, a) {
                    (d - r).abs()
                } else {
                    let s = c.add(Pt::from_angle(*a0, *r));
                    let e = c.add(Pt::from_angle(*a0 + *sweep, *r));
                    p.dist(s).min(p.dist(e))
                }
            }
        }
    }
    /// Exact closest point on the entity, used by the "nearest point" snap.
    pub fn nearest_point(&self, p: Pt) -> (Pt, f32) {
        match &self.kind {
            Kind::Point { p: q } => (*q, p.dist(*q)),
            Kind::Line { a, b } => {
                let ab = b.sub(*a);
                let l2 = ab.dot(ab);
                let t = if l2 < 1e-12 {
                    0.0
                } else {
                    (p.sub(*a).dot(ab) / l2).clamp(0.0, 1.0)
                };
                let q = a.add(ab.mul(t));
                (q, p.dist(q))
            }
            Kind::Circle { c, r } => {
                let d = p.sub(*c);
                let q = if d.len() < 1e-6 {
                    c.add(Pt::new(*r, 0.0))
                } else {
                    c.add(d.norm().mul(*r))
                };
                (q, p.dist(q))
            }
            Kind::Arc { c, r, a0, sweep } => {
                let a = p.sub(*c).angle();
                if angle_on_arc(*a0, *sweep, a) {
                    let q = c.add(Pt::from_angle(a, *r));
                    (q, p.dist(q))
                } else {
                    let s = c.add(Pt::from_angle(*a0, *r));
                    let e = c.add(Pt::from_angle(*a0 + *sweep, *r));
                    if p.dist(s) <= p.dist(e) {
                        (s, p.dist(s))
                    } else {
                        (e, p.dist(e))
                    }
                }
            }
        }
    }
    pub fn snap_points(&self) -> Vec<Pt> {
        match &self.kind {
            Kind::Point { p } => vec![*p],
            Kind::Line { a, b } => vec![*a, *b, a.lerp(*b, 0.5)],
            Kind::Circle { c, r } => vec![
                *c,
                c.add(Pt::new(*r, 0.0)),
                c.add(Pt::new(-*r, 0.0)),
                c.add(Pt::new(0.0, *r)),
                c.add(Pt::new(0.0, -*r)),
            ],
            Kind::Arc { c, r, a0, sweep } => {
                let s = c.add(Pt::from_angle(*a0, *r));
                let e = c.add(Pt::from_angle(*a0 + *sweep, *r));
                let m = c.add(Pt::from_angle(*a0 + *sweep * 0.5, *r));
                vec![*c, s, e, m]
            }
        }
    }
    pub fn translate(&mut self, d: Pt) {
        match &mut self.kind {
            Kind::Point { p } => *p = p.add(d),
            Kind::Line { a, b } => {
                *a = a.add(d);
                *b = b.add(d);
            }
            Kind::Circle { c, .. } | Kind::Arc { c, .. } => *c = c.add(d),
        }
        self.refresh_bbox();
    }
    pub fn rotate(&mut self, center: Pt, angle: f32) {
        match &mut self.kind {
            Kind::Point { p } => *p = p.rotate(center, angle),
            Kind::Line { a, b } => {
                *a = a.rotate(center, angle);
                *b = b.rotate(center, angle);
            }
            Kind::Circle { c, .. } => *c = c.rotate(center, angle),
            Kind::Arc { c, a0, .. } => {
                *c = c.rotate(center, angle);
                *a0 = norm_angle(*a0 + angle);
            }
        }
        self.refresh_bbox();
    }
    pub fn scale(&mut self, center: Pt, f: f32) {
        match &mut self.kind {
            Kind::Point { p } => {
                *p = center.add(p.sub(center).mul(f));
            }
            Kind::Line { a, b } => {
                *a = center.add(a.sub(center).mul(f));
                *b = center.add(b.sub(center).mul(f));
            }
            Kind::Circle { c, r } => {
                *c = center.add(c.sub(center).mul(f));
                *r *= f.abs();
            }
            Kind::Arc { c, r, .. } => {
                *c = center.add(c.sub(center).mul(f));
                *r *= f.abs();
            }
        }
        self.refresh_bbox();
    }
    pub fn mirror(&mut self, p1: Pt, p2: Pt) {
        let reflect = |p: Pt| -> Pt {
            let d = p2.sub(p1);
            let l2 = d.dot(d);
            if l2 < 1e-12 {
                return p;
            }
            let t = p.sub(p1).dot(d) / l2;
            let proj = p1.add(d.mul(t));
            proj.mul(2.0).sub(p)
        };
        match &mut self.kind {
            Kind::Point { p } => *p = reflect(*p),
            Kind::Line { a, b } => {
                *a = reflect(*a);
                *b = reflect(*b);
            }
            Kind::Circle { c, .. } => *c = reflect(*c),
            Kind::Arc { c, r, a0, sweep } => {
                let old_start = c.add(Pt::from_angle(*a0, *r));
                let old_end = c.add(Pt::from_angle(*a0 + *sweep, *r));
                *c = reflect(*c);
                let new_start = reflect(old_start);
                let new_end = reflect(old_end);
                *a0 = norm_angle(new_start.sub(*c).angle());
                *sweep = -*sweep;
                let _ = new_end;
            }
        }
        self.refresh_bbox();
    }
    pub fn offset(&self, d: f32) -> Option<Kind> {
        match &self.kind {
            Kind::Line { a, b } => {
                let n = b.sub(*a).norm().left().mul(d);
                Some(Kind::Line { a: a.add(n), b: b.add(n) })
            }
            Kind::Circle { c, r } => Some(Kind::Circle { c: *c, r: (*r + d).max(0.01) }),
            Kind::Arc { c, r, a0, sweep } => Some(Kind::Arc {
                c: *c,
                r: (*r + d).max(0.01),
                a0: *a0,
                sweep: *sweep,
            }),
            Kind::Point { p } => Some(Kind::Point { p: *p }),
        }
    }
}

/// World bounds for one geometric primitive. Arc bounds are exact: only the
/// cardinal directions actually covered by the sweep are joined to the two
/// endpoints, so a small arc no longer reports the bounds of its full circle.
pub fn bbox_of_kind(kind: &Kind) -> BBox {
    match kind {
        Kind::Point { p } => BBox::new(*p),
        Kind::Line { a, b } => {
            let mut bb = BBox::new(*a);
            bb.union_point(*b);
            bb
        }
        Kind::Circle { c, r } => BBox {
            min: Pt::new(c.x - r, c.y - r),
            max: Pt::new(c.x + r, c.y + r),
        },
        Kind::Arc { c, r, a0, sweep } => {
            let mut bb = BBox::new(c.add(Pt::from_angle(*a0, *r)));
            bb.union_point(c.add(Pt::from_angle(*a0 + *sweep, *r)));
            for q in 0..4 {
                let a = q as f32 * PI * 0.5;
                if angle_on_arc(*a0, *sweep, a) {
                    bb.union_point(c.add(Pt::from_angle(a, *r)));
                }
            }
            bb
        }
    }
}

#[derive(Debug)]
pub struct Doc {
    pub entities: Vec<Entity>,
    pub next_id: u64,
    pub title: String,
    pub part: String,
    pub unit: String,
    pub programmer: String,
    pub note: String,
    /// Bumped by every change. The UI uses this O(1) key instead of hashing
    /// all geometry on every frame.
    rev: u64,
    geom_rev: u64,
    uid: u64,
    index: RefCell<Option<crate::spatial::SpatialIndex>>,
    ids: RefCell<Option<HashMap<u64, usize>>>,
    ids_rev: Cell<u64>,
    bounds_cache: RefCell<Option<BBox>>,
    bounds_rev: Cell<u64>,
}

static NEXT_DOC_UID: AtomicU64 = AtomicU64::new(1);

fn next_doc_uid() -> u64 {
    NEXT_DOC_UID.fetch_add(1, Ordering::Relaxed)
}

impl Clone for Doc {
    fn clone(&self) -> Self {
        // Undo history clones the document, not its acceleration structures:
        // an index is rebuilt lazily from the cloned entities on first use.
        Self {
            entities: self.entities.clone(),
            next_id: self.next_id,
            title: self.title.clone(),
            part: self.part.clone(),
            unit: self.unit.clone(),
            programmer: self.programmer.clone(),
            note: self.note.clone(),
            rev: self.rev,
            geom_rev: self.geom_rev,
            uid: self.uid,
            index: RefCell::new(None),
            ids: RefCell::new(None),
            ids_rev: Cell::new(0),
            bounds_cache: RefCell::new(None),
            bounds_rev: Cell::new(u64::MAX),
        }
    }
}

impl Doc {
    pub fn new() -> Self {
        Self {
            entities: Vec::new(),
            next_id: 1,
            title: "未命名".to_string(),
            part: "未命名".to_string(),
            unit: "mm".to_string(),
            programmer: String::new(),
            note: String::new(),
            rev: 1,
            geom_rev: 1,
            uid: next_doc_uid(),
            index: RefCell::new(None),
            ids: RefCell::new(None),
            ids_rev: Cell::new(0),
            bounds_cache: RefCell::new(None),
            bounds_rev: Cell::new(u64::MAX),
        }
    }

    /// Identity of this document and its current edit revision. Together they
    /// are a cheap cache key for rendered layers.
    pub fn revision(&self) -> u64 {
        self.rev
    }

    pub fn uid(&self) -> u64 {
        self.uid
    }

    /// Metadata changed (title, part name, note...). Geometry caches stay
    /// valid; only the rendered layer needs to be rebuilt.
    pub fn touch(&mut self) {
        self.rev = self.rev.wrapping_add(1);
    }

    /// Geometry changed in place (rotate/scale/trim, or a whole document was
    /// edited outside the add/remove helpers).
    pub fn touch_geometry(&mut self) {
        self.rev = self.rev.wrapping_add(1);
        self.geom_rev = self.geom_rev.wrapping_add(1);
        *self.index.get_mut() = None;
        *self.ids.get_mut() = None;
        self.bounds_rev.set(u64::MAX);
    }

    pub fn demo() -> (Self, Vec<u64>) {
        let mut doc = Doc::new();
        doc.title = "示例零件.cnc".to_string();
        doc.part = "示例零件".to_string();
        doc.programmer = "编程员".to_string();
        doc.note = "带三个穿丝孔的样板".to_string();
        let (x0, x1, y0, y1, r) = (-110.0, 110.0, -70.0, 70.0, 20.0);
        let mut outer = Vec::new();
        let push = |doc: &mut Doc, outer: &mut Vec<u64>, k: Kind| {
            outer.push(doc.add(k));
        };
        push(&mut doc, &mut outer, Kind::Line {
            a: Pt::new(x0 + r, y0),
            b: Pt::new(x1 - r, y0),
        });
        push(&mut doc, &mut outer, Kind::Arc {
            c: Pt::new(x1 - r, y0 + r),
            r,
            a0: -PI * 0.5,
            sweep: PI * 0.5,
        });
        push(&mut doc, &mut outer, Kind::Line {
            a: Pt::new(x1, y0 + r),
            b: Pt::new(x1, y1 - r),
        });
        push(&mut doc, &mut outer, Kind::Arc {
            c: Pt::new(x1 - r, y1 - r),
            r,
            a0: 0.0,
            sweep: PI * 0.5,
        });
        push(&mut doc, &mut outer, Kind::Line {
            a: Pt::new(x1 - r, y1),
            b: Pt::new(x0 + r, y1),
        });
        push(&mut doc, &mut outer, Kind::Arc {
            c: Pt::new(x0 + r, y1 - r),
            r,
            a0: PI * 0.5,
            sweep: PI * 0.5,
        });
        push(&mut doc, &mut outer, Kind::Line {
            a: Pt::new(x0, y1 - r),
            b: Pt::new(x0, y0 + r),
        });
        push(&mut doc, &mut outer, Kind::Arc {
            c: Pt::new(x0 + r, y0 + r),
            r,
            a0: PI,
            sweep: PI * 0.5,
        });
        doc.add(Kind::Circle { c: Pt::new(-65.0, 0.0), r: 12.0 });
        doc.add(Kind::Circle { c: Pt::new(0.0, 0.0), r: 22.0 });
        doc.add(Kind::Circle { c: Pt::new(65.0, 0.0), r: 12.0 });
        (doc, outer)
    }

    pub fn add(&mut self, kind: Kind) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let idx = self.entities.len() as u32;
        self.entities.push(Entity::new(id, kind));
        let old_geom = self.geom_rev;
        let index_live = self
            .index
            .get_mut()
            .as_ref()
            .map(|ix| ix.geom_rev() == old_geom)
            .unwrap_or(false);
        self.rev = self.rev.wrapping_add(1);
        self.geom_rev = self.geom_rev.wrapping_add(1);
        if index_live {
            let bb = self.entities[idx as usize].bbox();
            if let Some(ix) = self.index.get_mut().as_mut() {
                ix.add_entity(idx, bb);
                ix.set_geom_rev(self.geom_rev);
            }
        }
        if let Some(map) = self.ids.get_mut() {
            if self.ids_rev.get() == old_geom {
                map.insert(id, idx as usize);
                self.ids_rev.set(self.geom_rev);
            } else {
                *self.ids.get_mut() = None;
            }
        }
        self.bounds_rev.set(u64::MAX);
        id
    }

    pub fn remove_ids(&mut self, ids: &[u64]) {
        if ids.is_empty() || self.entities.is_empty() {
            return;
        }
        let remove: HashSet<u64> = ids.iter().copied().collect();
        let before = self.entities.len();
        self.entities.retain(|e| !remove.contains(&e.id));
        if self.entities.len() != before {
            self.touch_geometry();
        }
    }

    fn ensure_ids(&self) {
        let valid = self.ids.borrow().is_some() && self.ids_rev.get() == self.geom_rev;
        if !valid {
            let map = crate::spatial::build_id_map(&self.entities);
            *self.ids.borrow_mut() = Some(map);
            self.ids_rev.set(self.geom_rev);
        }
    }

    /// O(1) handle lookup. Previously every call scanned the whole drawing,
    /// which made queries over a large selection quadratic.
    pub fn get(&self, id: u64) -> Option<&Entity> {
        self.ensure_ids();
        let ids = self.ids.borrow();
        let idx = ids.as_ref()?.get(&id).copied()?;
        self.entities.get(idx)
    }

    pub fn bounds(&self) -> Option<BBox> {
        if self.bounds_rev.get() == self.geom_rev {
            return *self.bounds_cache.borrow();
        }
        let mut it = self.entities.iter();
        let bb = it.next().map(|e| {
            let mut bb = e.bbox();
            for e in it {
                bb.union(e.bbox());
            }
            bb
        });
        *self.bounds_cache.borrow_mut() = bb;
        self.bounds_rev.set(self.geom_rev);
        bb
    }

    fn ensure_index(&self) {
        if self.entities.is_empty() {
            *self.index.borrow_mut() = None;
            return;
        }
        let valid = self
            .index
            .borrow()
            .as_ref()
            .map(|ix| ix.geom_rev() == self.geom_rev)
            .unwrap_or(false);
        if valid {
            return;
        }
        let bounds = self.bounds().unwrap_or(BBox {
            min: Pt::new(-1.0, -1.0),
            max: Pt::new(1.0, 1.0),
        });
        let index = crate::spatial::SpatialIndex::build(&self.entities, bounds, self.geom_rev);
        *self.index.borrow_mut() = Some(index);
    }

    /// Append ids whose bounds intersect `rect`. Near the cursor this visits
    /// only a handful of grid cells.
    pub fn query_rect(&self, rect: BBox, out: &mut Vec<u64>) {
        self.ensure_index();
        let index = self.index.borrow();
        let Some(ix) = index.as_ref() else { return };
        let mut found = Vec::new();
        ix.query_indices(rect, &self.entities, &mut found);
        out.reserve(found.len());
        for i in found {
            out.push(self.entities[i as usize].id);
        }
    }

    /// Best exact hit within `tol` world units of `p`.
    pub fn pick(&self, p: Pt, tol: f32) -> Option<u64> {
        let rect = BBox {
            min: Pt::new(p.x - tol, p.y - tol),
            max: Pt::new(p.x + tol, p.y + tol),
        };
        self.ensure_index();
        let index = self.index.borrow();
        let Some(ix) = index.as_ref() else { return None };
        let mut found = Vec::new();
        ix.query_indices(rect, &self.entities, &mut found);
        let mut best: Option<(f32, u64)> = None;
        for i in found {
            let e = &self.entities[i as usize];
            let d = e.hit(p);
            if d <= tol && best.as_ref().map(|b| d < b.0).unwrap_or(true) {
                best = Some((d, e.id));
            }
        }
        best.map(|b| b.1)
    }

    /// Move a set of entities and keep the acceleration structures current.
    /// Dragging a small selection updates only those entries; dragging most of
    /// a very large drawing falls back to a cheap lazy rebuild.
    pub fn translate_ids(&mut self, ids: &[u64], d: Pt) {
        if ids.is_empty() || self.entities.is_empty() {
            return;
        }
        self.ensure_ids();
        let mut indices: Vec<u32> = Vec::with_capacity(ids.len());
        {
            let map = self.ids.borrow();
            let Some(map) = map.as_ref() else { return };
            for id in ids {
                if let Some(&i) = map.get(id) {
                    indices.push(i as u32);
                }
            }
        }
        if indices.is_empty() {
            return;
        }
        let old_geom = self.geom_rev;
        let incremental = indices.len().saturating_mul(4) <= self.entities.len();
        if incremental {
            self.ensure_index();
        }
        let index_live = incremental
            && self
                .index
                .get_mut()
                .as_ref()
                .map(|ix| ix.geom_rev() == old_geom)
                .unwrap_or(false);
        for &i in &indices {
            self.entities[i as usize].translate(d);
        }
        self.rev = self.rev.wrapping_add(1);
        self.geom_rev = self.geom_rev.wrapping_add(1);
        if index_live {
            let bboxes: Vec<(u32, BBox)> = indices
                .iter()
                .map(|&i| (i, self.entities[i as usize].bbox()))
                .collect();
            if let Some(ix) = self.index.get_mut().as_mut() {
                for (i, bb) in bboxes {
                    ix.update(i, bb);
                }
                ix.set_geom_rev(self.geom_rev);
            }
        } else {
            *self.index.get_mut() = None;
        }
        // Entities were not reordered, so an existing handle map stays valid.
        self.ids_rev.set(self.geom_rev);
        self.bounds_rev.set(u64::MAX);
    }

    pub fn total_length(&self) -> f32 {
        self.entities.iter().map(|e| e.length()).sum()
    }

    /// Intersections of line entities with their point inside `clip`.
    /// The old implementation compared every pair; this one only pairs lines
    /// sharing a grid cell and emits each point in exactly one cell.
    pub fn intersections_in(&self, clip: BBox) -> Vec<Pt> {
        let mut out = Vec::new();
        if self.entities.is_empty() {
            return out;
        }
        self.ensure_index();
        let index = self.index.borrow();
        let Some(ix) = index.as_ref() else { return out };
        let is_line = |i: u32| matches!(self.entities[i as usize].kind, Kind::Line { .. });
        let line_of = |i: u32| -> Option<(Pt, Pt)> {
            match &self.entities[i as usize].kind {
                Kind::Line { a, b } => Some((*a, *b)),
                _ => None,
            }
        };
        if let Some((c0, r0, c1, r1)) = ix.cells_in(clip) {
            for r in r0..=r1 {
                for c in c0..=c1 {
                    let cb = ix.cell_bounds(c, r);
                    let lines: Vec<u32> = ix
                        .cell(c, r)
                        .iter()
                        .copied()
                        .filter(|&i| {
                            is_line(i) && self.entities[i as usize].bbox().intersects(&clip)
                        })
                        .collect();
                    for (ia, &a) in lines.iter().enumerate() {
                        for &b in &lines[ia + 1..] {
                            let (Some((a1, b1)), Some((a2, b2))) = (line_of(a), line_of(b)) else {
                                continue;
                            };
                            if let Some(p) = segment_intersection(a1, b1, a2, b2) {
                                if in_cell(&cb, p) && clip.contains(p) {
                                    out.push(p);
                                }
                            }
                        }
                    }
                }
            }
        }
        // Entities stored in the overflow set are not present in any cell.
        let big: Vec<u32> = ix
            .big()
            .iter()
            .copied()
            .filter(|&i| is_line(i) && self.entities[i as usize].bbox().intersects(&clip))
            .collect();
        if !big.is_empty() {
            let mut candidates = Vec::new();
            ix.query_indices(clip, &self.entities, &mut candidates);
            for (ia, &a) in big.iter().enumerate() {
                for &b in &candidates {
                    // Other overflow lines are paired once in the big/big loop.
                    if a == b || big.contains(&b) {
                        continue;
                    }
                    let (Some((a1, b1)), Some((a2, b2))) = (line_of(a), line_of(b)) else {
                        continue;
                    };
                    if let Some(p) = segment_intersection(a1, b1, a2, b2) {
                        if clip.contains(p) {
                            out.push(p);
                        }
                    }
                }
                for &b in &big[ia + 1..] {
                    let (Some((a1, b1)), Some((a2, b2))) = (line_of(a), line_of(b)) else {
                        continue;
                    };
                    if let Some(p) = segment_intersection(a1, b1, a2, b2) {
                        if clip.contains(p) {
                            out.push(p);
                        }
                    }
                }
            }
        }
        out
    }

    pub fn all_ids(&self) -> Vec<u64> {
        self.entities.iter().map(|e| e.id).collect()
    }

    pub fn save_text(&self) -> String {
        fn clean(s: &str) -> String {
            s.replace(['\r', '\n'], " ")
        }
        let mut out = String::new();
        out.push_str("CNC-WEDM 1\n");
        out.push_str(&format!("TITLE {}\n", clean(&self.title)));
        out.push_str(&format!("PART {}\n", clean(&self.part)));
        out.push_str(&format!("UNIT {}\n", clean(&self.unit)));
        out.push_str(&format!("PROGRAMMER {}\n", clean(&self.programmer)));
        out.push_str(&format!("NOTE {}\n", clean(&self.note)));
        for e in &self.entities {
            match &e.kind {
                Kind::Point { p } => out.push_str(&format!("E POINT {:.4} {:.4}\n", p.x, p.y)),
                Kind::Line { a, b } => out.push_str(&format!(
                    "E LINE {:.4} {:.4} {:.4} {:.4}\n",
                    a.x, a.y, b.x, b.y
                )),
                Kind::Circle { c, r } => {
                    out.push_str(&format!("E CIRCLE {:.4} {:.4} {:.4}\n", c.x, c.y, r))
                }
                Kind::Arc { c, r, a0, sweep } => out.push_str(&format!(
                    "E ARC {:.4} {:.4} {:.4} {:.5} {:.5}\n",
                    c.x, c.y, r, a0, sweep
                )),
            }
        }
        out
    }

    pub fn from_text(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        let first = lines.next().unwrap_or("");
        if !first.starts_with("CNC-WEDM") {
            return Err("不是本软件的图形文件（缺少文件头）".to_string());
        }
        let mut doc = Doc::new();
        for (idx, raw) in lines.enumerate() {
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }
            let (key, rest) = match line.split_once(' ') {
                Some(v) => v,
                None => (line, ""),
            };
            let mut nums = || -> Vec<f32> { rest.split_whitespace().filter_map(|s| s.parse::<f32>().ok()).collect() };
            match key {
                "TITLE" => doc.title = rest.to_string(),
                "PART" => doc.part = rest.to_string(),
                "UNIT" => doc.unit = rest.to_string(),
                "PROGRAMMER" => doc.programmer = rest.to_string(),
                "NOTE" => doc.note = rest.to_string(),
                "E" => {
                    let mut parts = rest.splitn(2, ' ');
                    let kind = parts.next().unwrap_or("");
                    let tail = parts.next().unwrap_or("");
                    let v: Vec<f32> = tail.split_whitespace().filter_map(|s| s.parse::<f32>().ok()).collect();
                    let n = v.len();
                    let kind = match kind {
                        "POINT" if n >= 2 => Kind::Point { p: Pt::new(v[0], v[1]) },
                        "LINE" if n >= 4 => Kind::Line {
                            a: Pt::new(v[0], v[1]),
                            b: Pt::new(v[2], v[3]),
                        },
                        "CIRCLE" if n >= 3 => Kind::Circle {
                            c: Pt::new(v[0], v[1]),
                            r: v[2].abs(),
                        },
                        "ARC" if n >= 5 => Kind::Arc {
                            c: Pt::new(v[0], v[1]),
                            r: v[2].abs(),
                            a0: v[3],
                            sweep: v[4],
                        },
                        _ => return Err(format!("第 {} 行图元数据无法识别", idx + 2)),
                    };
                    doc.add(kind);
                }
                _ => {}
            }
        }
        Ok(doc)
    }
}

/// A grid cell owns its left/bottom edge and, at the drawing's outer edge,
/// its right/top edge too. Intersections exactly on an internal cell border can
/// therefore be reported by both neighbouring cells; that is harmless for a
/// snap or a node marker and avoids missing a boundary point.
fn in_cell(cell: &BBox, p: Pt) -> bool {
    let eps = 1e-4;
    p.x >= cell.min.x - eps
        && p.x <= cell.max.x + eps
        && p.y >= cell.min.y - eps
        && p.y <= cell.max.y + eps
}

pub fn segment_intersection(a1: Pt, b1: Pt, a2: Pt, b2: Pt) -> Option<Pt> {    let r = b1.sub(a1);
    let s = b2.sub(a2);
    let denom = r.x * s.y - r.y * s.x;
    if denom.abs() < 1e-9 {
        return None;
    }
    let qp = a2.sub(a1);
    let t = (qp.x * s.y - qp.y * s.x) / denom;
    let u = (qp.x * r.y - qp.y * r.x) / denom;
    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
        Some(a1.add(r.mul(t)))
    } else {
        None
    }
}

pub fn polyline_area(pts: &[Pt]) -> f32 {
    if pts.len() < 3 {
        return 0.0;
    }
    let mut a = 0.0;
    for i in 0..pts.len() {
        let p = pts[i];
        let q = pts[(i + 1) % pts.len()];
        a += p.x * q.y - q.x * p.y;
    }
    a.abs() * 0.5
}

pub fn polyline_length(pts: &[Pt]) -> f32 {
    pts.windows(2).map(|w| w[0].dist(w[1])).sum()
}

