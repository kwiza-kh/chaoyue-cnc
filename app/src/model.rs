//! Geometry model for the wire-EDM drawing board.

use std::f32::consts::{PI, TAU};

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
}

impl Entity {
    pub fn new(id: u64, kind: Kind) -> Self {
        Self { id, kind }
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
        match &self.kind {
            Kind::Point { p } => BBox::new(*p),
            Kind::Line { a, b } => {
                let mut bb = BBox::new(*a);
                bb.union_point(*b);
                bb
            }
            Kind::Circle { c, r } => {
                let mut bb = BBox::new(Pt::new(c.x - r, c.y - r));
                bb.union_point(Pt::new(c.x + r, c.y + r));
                bb
            }
            Kind::Arc { c, r, a0, sweep } => {
                let mut bb = BBox::new(Pt::new(c.x - r, c.y - r));
                bb.union_point(Pt::new(c.x + r, c.y + r));
                // Restrict to the actual arc by testing cardinal points.
                let mut pts = Vec::new();
                for q in 0..4 {
                    let a = q as f32 * PI * 0.5;
                    if angle_on_arc(*a0, *sweep, a) {
                        pts.push(c.add(Pt::from_angle(a, *r)));
                    }
                }
                pts.push(c.add(Pt::from_angle(*a0, *r)));
                pts.push(c.add(Pt::from_angle(*a0 + *sweep, *r)));
                let mut bb = BBox::new(pts[0]);
                for p in pts {
                    bb.union_point(p);
                }
                bb
            }
        }
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

#[derive(Clone, Debug)]
pub struct Doc {
    pub entities: Vec<Entity>,
    pub next_id: u64,
    pub title: String,
    pub part: String,
    pub unit: String,
    pub programmer: String,
    pub note: String,
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
        }
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
        self.entities.push(Entity::new(id, kind));
        id
    }

    pub fn remove_ids(&mut self, ids: &[u64]) {
        self.entities.retain(|e| !ids.contains(&e.id));
    }

    pub fn get(&self, id: u64) -> Option<&Entity> {
        self.entities.iter().find(|e| e.id == id)
    }

    pub fn bounds(&self) -> Option<BBox> {
        let mut it = self.entities.iter();
        let first = it.next()?.bbox();
        let mut bb = first;
        for e in it {
            bb.union(e.bbox());
        }
        Some(bb)
    }

    pub fn total_length(&self) -> f32 {
        self.entities.iter().map(|e| e.length()).sum()
    }

    pub fn intersections(&self) -> Vec<Pt> {
        let mut pts = Vec::new();
        for i in 0..self.entities.len() {
            if let Kind::Line { a: a1, b: b1 } = self.entities[i].kind {
                for j in (i + 1)..self.entities.len() {
                    if let Kind::Line { a: a2, b: b2 } = self.entities[j].kind {
                        if let Some(p) = segment_intersection(a1, b1, a2, b2) {
                            pts.push(p);
                        }
                    }
                }
            }
        }
        pts
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

pub fn segment_intersection(a1: Pt, b1: Pt, a2: Pt, b2: Pt) -> Option<Pt> {
    let r = b1.sub(a1);
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

