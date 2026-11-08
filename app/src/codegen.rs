//! Wire-EDM cut path construction, ISO G-code and 3B code output.

use crate::model::{norm_angle, polyline_area, Doc, Kind, Pt};
use std::f32::consts::{PI, TAU};

#[derive(Clone, Debug)]
pub enum Seg {
    Line { a: Pt, b: Pt },
    /// Counter-clockwise when `sweep` is positive, clockwise when negative.
    Arc { c: Pt, r: f32, a0: f32, sweep: f32 },
}

impl Seg {
    pub fn start(&self) -> Pt {
        match self {
            Seg::Line { a, .. } => *a,
            Seg::Arc { c, r, a0, .. } => c.add(Pt::from_angle(*a0, *r)),
        }
    }
    pub fn end(&self) -> Pt {
        match self {
            Seg::Line { b, .. } => *b,
            Seg::Arc { c, r, a0, sweep } => c.add(Pt::from_angle(*a0 + *sweep, *r)),
        }
    }
    pub fn start_angle(&self) -> f32 {
        match self {
            Seg::Line { a, b } => b.sub(*a).angle(),
            Seg::Arc { a0, .. } => *a0,
        }
    }
    /// Unit travel direction at the start of the segment.
    pub fn start_tangent(&self) -> Pt {
        match self {
            Seg::Line { a, b } => b.sub(*a).norm(),
            Seg::Arc { a0, sweep, .. } => {
                let t = Pt::new(-a0.sin(), a0.cos());
                if *sweep >= 0.0 {
                    t
                } else {
                    t.mul(-1.0)
                }
            }
        }
    }
    pub fn length(&self) -> f32 {
        match self {
            Seg::Line { a, b } => a.dist(*b),
            Seg::Arc { r, sweep, .. } => r * sweep.abs(),
        }
    }
    pub fn reversed(&self) -> Seg {
        match self {
            Seg::Line { a, b } => Seg::Line { a: *b, b: *a },
            Seg::Arc { c, r, a0, sweep } => Seg::Arc {
                c: *c,
                r: *r,
                a0: *a0 + *sweep,
                sweep: -*sweep,
            },
        }
    }
    pub fn offset(&self, left: f32) -> Seg {
        match self {
            Seg::Line { a, b } => {
                let n = b.sub(*a).norm().left().mul(left);
                Seg::Line { a: a.add(n), b: b.add(n) }
            }
            Seg::Arc { c, r, a0, sweep } => {
                let delta = if *sweep >= 0.0 { -left } else { left };
                Seg::Arc {
                    c: *c,
                    r: (*r + delta).max(0.02),
                    a0: *a0,
                    sweep: *sweep,
                }
            }
        }
    }
    pub fn sampled(&self, max_step: f32) -> Vec<Pt> {
        match self {
            Seg::Line { a, b } => vec![*a, *b],
            Seg::Arc { c, r, a0, sweep } => {
                let steps = ((r * sweep.abs() / max_step.max(0.2)).ceil() as usize).clamp(4, 240);
                (0..=steps)
                    .map(|i| {
                        let a = *a0 + *sweep * (i as f32 / steps as f32);
                        c.add(Pt::from_angle(a, *r))
                    })
                    .collect()
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct CutPath {
    pub segs: Vec<Seg>,
    /// Straight move from the pierce point to the contour start.
    pub lead: Option<Seg>,
    pub closed: bool,
    pub pierce: Pt,
}

impl CutPath {
    pub fn length(&self) -> f32 {
        self.lead.as_ref().map(|s| s.length()).unwrap_or(0.0)
            + self.segs.iter().map(|s| s.length()).sum::<f32>()
    }
    pub fn contour_length(&self) -> f32 {
        self.segs.iter().map(|s| s.length()).sum()
    }
    pub fn start(&self) -> Option<Pt> {
        self.segs.first().map(|s| s.start())
    }
    pub fn end(&self) -> Option<Pt> {
        self.segs.last().map(|s| s.end())
    }
    pub fn sampled(&self) -> Vec<Pt> {
        let mut pts = Vec::new();
        if let Some(l) = &self.lead {
            pts.push(l.start());
            pts.push(l.end());
        }
        for (i, s) in self.segs.iter().enumerate() {
            let mut p = s.sampled(0.6);
            if i > 0 {
                p.remove(0);
            }
            pts.extend(p);
        }
        pts
    }
    pub fn area(&self) -> f32 {
        if self.closed {
            polyline_area(&self.sampled())
        } else {
            0.0
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct CutJob {
    pub paths: Vec<CutPath>,
}

impl CutJob {
    pub fn length(&self) -> f32 {
        self.paths.iter().map(|p| p.length()).sum()
    }
    pub fn area(&self) -> f32 {
        self.paths.iter().map(|p| p.area()).sum()
    }
}

fn entity_seg(e: &crate::model::Entity) -> Option<Seg> {
    match &e.kind {
        Kind::Line { a, b } => Some(Seg::Line { a: *a, b: *b }),
        Kind::Arc { c, r, a0, sweep } => Some(Seg::Arc {
            c: *c,
            r: *r,
            a0: *a0,
            sweep: *sweep,
        }),
        _ => None,
    }
}

/// Order the selected entities into connected chains and offset each chain for
/// the wire / kerf radius. No lead-in is added: this form is used by the foam
/// cutter, which chooses its own entry side and travel strategy.
pub fn build_contours(
    doc: &Doc,
    ids: &[u64],
    comp: f32,
    side: i8,
    hint: Pt,
) -> Result<CutJob, String> {
    let selected: Vec<crate::model::Entity> = ids
        .iter()
        .filter_map(|id| doc.get(*id).cloned())
        .collect();
    if selected.is_empty() {
        return Err("请先在图上选择要加工的轮廓（可框选或单击图元）".to_string());
    }
    let mut job = CutJob::default();
    let left = comp * side as f32;
    let mut circles: Vec<CutPath> = Vec::new();

    // Closed circles are kept separately so they can be nested / ordered later.
    for e in &selected {
        if let Kind::Circle { c, r } = e.kind {
            let a0 = if hint.dist(c) > 1e-3 {
                hint.sub(c).angle()
            } else {
                0.0
            };
            let r2 = (r - left).max(0.05);
            let seg = Seg::Arc {
                c,
                r: r2,
                a0,
                sweep: TAU,
            };
            let start = seg.start();
            circles.push(CutPath {
                segs: vec![seg],
                lead: None,
                closed: true,
                pierce: start,
            });
        }
    }

    // Chains of lines and arcs. Each new chain starts at the point nearest the
    // end of the previous chain, which keeps the travel between contours short.
    // Points and other non-cutting helpers are ignored instead of aborting the
    // whole job when the operator selects everything on the board.
    let mut remaining: Vec<crate::model::Entity> = selected
        .iter()
        .filter(|e| matches!(e.kind, Kind::Line { .. } | Kind::Arc { .. }))
        .cloned()
        .collect();
    let mut cursor = hint;
    while !remaining.is_empty() {
        let mut best = 0usize;
        let mut best_dist = f32::INFINITY;
        let mut best_rev = false;
        for (i, e) in remaining.iter().enumerate() {
            if let (Some(s), Some(en)) = (e.start(), e.end()) {
                let ds = s.dist(cursor);
                let de = en.dist(cursor);
                if ds < best_dist {
                    best_dist = ds;
                    best = i;
                    best_rev = false;
                }
                if de < best_dist {
                    best_dist = de;
                    best = i;
                    best_rev = true;
                }
            }
        }
        let mut seg = entity_seg(&remaining[best]).ok_or("无法处理的图元")?;
        if best_rev {
            seg = seg.reversed();
        }
        let chain_start = seg.start();
        let mut chain = vec![seg];
        remaining.remove(best);
        let mut current = chain[0].end();
        let mut guard = 0;
        while guard < 100_000 {
            guard += 1;
            let mut found = None;
            let mut found_dist = f32::INFINITY;
            for (i, e) in remaining.iter().enumerate() {
                if let Some(s) = entity_seg(e) {
                    let d1 = s.start().dist(current);
                    let d2 = s.end().dist(current);
                    if d1 < found_dist && d1 < 0.05 {
                        found = Some((i, false));
                        found_dist = d1;
                    }
                    if d2 < found_dist && d2 < 0.05 {
                        found = Some((i, true));
                        found_dist = d2;
                    }
                }
            }
            if let Some((i, rev)) = found {
                let mut s = entity_seg(&remaining[i]).unwrap();
                if rev {
                    s = s.reversed();
                }
                current = s.end();
                chain.push(s);
                remaining.remove(i);
            } else {
                break;
            }
        }
        if chain.is_empty() {
            break;
        }
        let closed = (current.dist(chain_start) < 0.05 && chain.len() > 1)
            || (chain.len() == 1
                && matches!(chain[0], Seg::Arc { sweep, .. } if sweep.abs() >= TAU - 1e-3));
        let mut segs: Vec<Seg> = chain.iter().map(|s| s.offset(left)).collect();
        // Stitch the offset segments together where the offset has opened a gap.
        let mut joined: Vec<Seg> = Vec::new();
        for i in 0..segs.len() {
            if i > 0 {
                let prev_end = segs[i - 1].end();
                let start = segs[i].start();
                if prev_end.dist(start) > 0.02 {
                    joined.push(Seg::Line { a: prev_end, b: start });
                }
            }
            joined.push(segs[i].clone());
        }
        if closed {
            if let (Some(last), Some(first)) = (segs.last(), segs.first()) {
                if last.end().dist(first.start()) > 0.02 {
                    joined.push(Seg::Line {
                        a: last.end(),
                        b: first.start(),
                    });
                }
            }
        }
        let path_start = joined.first().map(|s| s.start()).unwrap_or(chain_start);
        cursor = joined.last().map(|s| s.end()).unwrap_or(path_start);
        job.paths.push(CutPath {
            segs: joined,
            lead: None,
            closed,
            pierce: path_start,
        });
    }
    job.paths.extend(circles);

    if job.paths.is_empty() {
        return Err("所选图元无法组成切割路径".to_string());
    }
    Ok(job)
}

/// Order the selected entities into connected chains, offset each chain for the
/// wire radius and add a lead-in from the pierce point.
pub fn build_job(
    doc: &Doc,
    ids: &[u64],
    pierce: Pt,
    comp: f32,
    side: i8,
) -> Result<CutJob, String> {
    let mut job = build_contours(doc, ids, comp, side, pierce)?;
    for p in &mut job.paths {
        let start = p.segs.first().map(|s| s.start()).unwrap_or(pierce);
        p.pierce = pierce;
        p.lead = if pierce.dist(start) > 0.01 {
            Some(Seg::Line { a: pierce, b: start })
        } else {
            None
        };
    }
    Ok(job)
}

/// Closest point on one segment, the distance, and the segment parameter
/// (0..1 for a line, an unwrapped angle for an arc).
pub fn closest_on_seg(seg: &Seg, target: Pt) -> (f32, Pt, f32) {
    match seg {
        Seg::Line { a, b } => {
            let ab = b.sub(*a);
            let l2 = ab.dot(ab);
            let t = if l2 < 1e-12 {
                0.0
            } else {
                (target.sub(*a).dot(ab) / l2).clamp(0.0, 1.0)
            };
            let q = a.add(ab.mul(t));
            (target.dist(q), q, t)
        }
        Seg::Arc { c, r, a0, sweep } => {
            if let Some(qa) = arc_angle_on(*a0, *sweep, target.sub(*c).angle()) {
                let q = c.add(Pt::from_angle(qa, *r));
                (target.dist(q), q, qa)
            } else {
                let s = c.add(Pt::from_angle(*a0, *r));
                let e = c.add(Pt::from_angle(*a0 + *sweep, *r));
                if target.dist(s) <= target.dist(e) {
                    (target.dist(s), s, *a0)
                } else {
                    (target.dist(e), e, *a0 + *sweep)
                }
            }
        }
    }
}

fn arc_angle_on(a0: f32, sweep: f32, a: f32) -> Option<f32> {
    if sweep >= 0.0 {
        let d = norm_angle(a - a0);
        if d <= sweep + 1e-4 {
            Some(a0 + d)
        } else {
            None
        }
    } else {
        let d = norm_angle(a0 - a);
        if d <= -sweep + 1e-4 {
            Some(a0 - d)
        } else {
            None
        }
    }
}

/// Nearest point on a whole path.
pub fn nearest_on_path(path: &CutPath, target: Pt) -> Option<(f32, usize, f32)> {
    let mut best: Option<(f32, usize, f32)> = None;
    for (i, seg) in path.segs.iter().enumerate() {
        let (d, _, p) = closest_on_seg(seg, target);
        if best.map(|b| d < b.0).unwrap_or(true) {
            best = Some((d, i, p));
        }
    }
    best
}

fn split_seg_at(seg: &Seg, param: f32) -> (Option<Seg>, Option<Seg>) {
    match seg {
        Seg::Line { a, b } => {
            let t = param.clamp(0.0, 1.0);
            let q = a.lerp(*b, t);
            let first = if q.dist(*b) > 1e-4 {
                Some(Seg::Line { a: q, b: *b })
            } else {
                None
            };
            let second = if a.dist(q) > 1e-4 {
                Some(Seg::Line { a: *a, b: q })
            } else {
                None
            };
            (first, second)
        }
        Seg::Arc { c, r, a0, sweep } => {
            let end = *a0 + *sweep;
            let qa = param;
            let first = if (end - qa).abs() > 1e-4 {
                Some(Seg::Arc { c: *c, r: *r, a0: qa, sweep: end - qa })
            } else {
                None
            };
            let second = if (qa - *a0).abs() > 1e-4 {
                Some(Seg::Arc { c: *c, r: *r, a0: *a0, sweep: qa - *a0 })
            } else {
                None
            };
            (first, second)
        }
    }
}

/// Rotate a closed contour so it starts at the point nearest `target`.
/// Returns the entry distance (INFINITY when the path cannot be entered).
pub fn rotate_closed_path(path: &mut CutPath, target: Pt) -> f32 {
    if !path.closed || path.segs.is_empty() {
        return path.start().map(|s| s.dist(target)).unwrap_or(f32::INFINITY);
    }
    let Some((dist, idx, param)) = nearest_on_path(path, target) else {
        return f32::INFINITY;
    };
    if dist < 1e-4 {
        return dist;
    }
    let at_start = match &path.segs[idx] {
        Seg::Line { .. } => param <= 1e-3,
        Seg::Arc { a0, .. } => (param - *a0).abs() <= 1e-5,
    };
    if at_start {
        let mut new = Vec::with_capacity(path.segs.len());
        new.extend_from_slice(&path.segs[idx..]);
        new.extend_from_slice(&path.segs[..idx]);
        path.segs = new;
    } else {
        let at_end = match &path.segs[idx] {
            Seg::Line { .. } => param >= 0.999,
            Seg::Arc { a0, sweep, .. } => (param - (*a0 + *sweep)).abs() <= 1e-5,
        };
        if at_end {
            let ni = (idx + 1) % path.segs.len();
            let mut new = Vec::with_capacity(path.segs.len());
            new.extend_from_slice(&path.segs[ni..]);
            new.extend_from_slice(&path.segs[..ni]);
            path.segs = new;
        } else {
            let (first, second) = split_seg_at(&path.segs[idx], param);
            let mut new = Vec::new();
            if let Some(f) = first {
                new.push(f);
            }
            new.extend_from_slice(&path.segs[idx + 1..]);
            new.extend_from_slice(&path.segs[..idx]);
            if let Some(s) = second {
                new.push(s);
            }
            path.segs = new;
        }
    }
    path.pierce = path.start().unwrap_or(path.pierce);
    path.lead = None;
    dist
}

pub fn reverse_path(path: &mut CutPath) {
    path.segs = path.segs.iter().rev().map(|s| s.reversed()).collect();
    if let Some(s) = path.start() {
        path.pierce = s;
    }
    path.lead = None;
}

/// Greedy nearest-entry ordering. For closed contours the entry point is also
/// rotated to the nearest point, and open contours may be reversed. WEDM jobs
/// keep their first (outer) path fixed so holes are still cut after the outer.
pub fn optimize_job(job: &mut CutJob, start: Pt, outer_first: bool, rotate_closed: bool) {
    if job.paths.len() < 2 && !rotate_closed {
        return;
    }
    let mut remaining = std::mem::take(&mut job.paths);
    let mut ordered: Vec<CutPath> = Vec::with_capacity(remaining.len());
    let mut cursor = start;
    if outer_first && !remaining.is_empty() {
        let p = remaining.remove(0);
        cursor = p.end().unwrap_or(start);
        ordered.push(p);
    }
    while !remaining.is_empty() {
        let mut best = 0usize;
        let mut best_dist = f32::INFINITY;
        let mut best_rev = false;
        for (i, p) in remaining.iter().enumerate() {
            if p.segs.is_empty() {
                continue;
            }
            if p.closed {
                if let Some((d, _, _)) = nearest_on_path(p, cursor) {
                    if d < best_dist {
                        best_dist = d;
                        best = i;
                        best_rev = false;
                    }
                }
            } else {
                let ds = p.start().map(|s| s.dist(cursor)).unwrap_or(f32::INFINITY);
                let de = p.end().map(|e| e.dist(cursor)).unwrap_or(f32::INFINITY);
                if ds <= de && ds < best_dist {
                    best_dist = ds;
                    best = i;
                    best_rev = false;
                } else if de < best_dist {
                    best_dist = de;
                    best = i;
                    best_rev = true;
                }
            }
        }
        let mut p = remaining.remove(best);
        if !p.closed && best_rev {
            reverse_path(&mut p);
        }
        if p.closed && rotate_closed {
            rotate_closed_path(&mut p, cursor);
        }
        if rotate_closed {
            p.pierce = p.start().unwrap_or(cursor);
        }
        cursor = p.end().unwrap_or(cursor);
        ordered.push(p);
    }
    job.paths = ordered;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodeKind {
    GCode,
    ThreeB,
    Mach3,
}

impl CodeKind {
    pub fn label(self) -> &'static str {
        match self {
            CodeKind::GCode => "G 代码 (ISO)",
            CodeKind::ThreeB => "3B 代码",
            CodeKind::Mach3 => "Mach3 G 代码",
        }
    }
    pub fn extension(self) -> &'static str {
        match self {
            CodeKind::GCode => "iso",
            CodeKind::ThreeB => "3b",
            CodeKind::Mach3 => "tap",
        }
    }
    pub fn from_index(i: usize) -> Self {
        match i {
            0 => CodeKind::ThreeB,
            1 => CodeKind::GCode,
            _ => CodeKind::Mach3,
        }
    }
    pub fn index(self) -> usize {
        match self {
            CodeKind::ThreeB => 0,
            CodeKind::GCode => 1,
            CodeKind::Mach3 => 2,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CodeOptions {
    pub kind: CodeKind,
    pub pierce: Pt,
    pub comp: f32,
    pub side: i8,
    pub part: String,
    /// Reorder contours and (for foam) choose the nearest entry point.
    pub optimize: bool,
    /// Cut feed in mm/min.
    pub feed: f32,
    /// Rapid / travel feed in mm/min.
    pub rapid: f32,
}

#[derive(Clone, Debug)]
pub struct CodeOutput {
    pub text: String,
    pub job: CutJob,
    pub kind: CodeKind,
    pub line_count: usize,
    pub path_count: usize,
    /// Extra facts shown in the code dialog (travel, time, mode).
    pub extra: String,
}

pub fn generate(doc: &Doc, ids: &[u64], opt: &CodeOptions) -> Result<CodeOutput, String> {
    let mut job = build_job(doc, ids, opt.pierce, opt.comp, opt.side)?;
    if opt.optimize {
        optimize_job(&mut job, opt.pierce, true, false);
    }
    let text = match opt.kind {
        CodeKind::GCode => gcode(&job, opt),
        CodeKind::ThreeB => code_3b(&job, opt),
        CodeKind::Mach3 => mach3_wedm(&job, opt),
    };
    let line_count = text.lines().filter(|l| !l.trim().is_empty()).count();
    let path_count = job.paths.len();
    Ok(CodeOutput {
        text,
        job,
        kind: opt.kind,
        line_count,
        path_count,
        extra: String::new(),
    })
}

fn f3(v: f32) -> String {
    let s = format!("{:.3}", v);
    if s == "-0.000" {
        "0.000".to_string()
    } else {
        s
    }
}

fn clean_comment(s: &str) -> String {
    s.replace(['\r', '\n', '(', ')'], " ").trim().to_string()
}

fn gcode(job: &CutJob, opt: &CodeOptions) -> String {
    let mut s = String::new();
    s.push_str("%\n");
    s.push_str("(CHAOYUE CNC WEDM)\n");
    s.push_str("G90 G21 G17 G94 G40 G49 G80\n");
    s.push_str(&format!(
        "(COMP {:.3} MM SIDE {})\n",
        opt.comp,
        if opt.side > 0 {
            "LEFT"
        } else if opt.side < 0 {
            "RIGHT"
        } else {
            "OFF"
        }
    ));
    for p in &job.paths {
        s.push_str(&format!(
            "(PATH LEN {:.3} MM)\n",
            p.contour_length()
        ));
        s.push_str(&format!(
            "G00 X{} Y{}\n",
            f3(p.pierce.x),
            f3(p.pierce.y)
        ));
        if let Some(lead) = &p.lead {
            s.push_str(&format!(
                "G01 X{} Y{} F{}\n",
                f3(lead.end().x),
                f3(lead.end().y),
                f3(opt.feed)
            ));
        }
        let first_start = p.segs.first().map(|x| x.start());
        if let (Some(l), Some(fs)) = (&p.lead, first_start) {
            if l.end().dist(fs) > 0.001 {
                s.push_str(&format!("G01 X{} Y{}\n", f3(fs.x), f3(fs.y)));
            }
        }
        for seg in &p.segs {
            match seg {
                Seg::Line { b, .. } => {
                    s.push_str(&format!("G01 X{} Y{} F{}\n", f3(b.x), f3(b.y), f3(opt.feed)));
                }
                Seg::Arc { c, a0, sweep, .. } => {
                    let start = seg.start();
                    let end = seg.end();
                    let i = c.x - start.x;
                    let j = c.y - start.y;
                    let g = if *sweep >= 0.0 { "G03" } else { "G02" };
                    s.push_str(&format!(
                        "{} X{} Y{} I{} J{}\n",
                        g,
                        f3(end.x),
                        f3(end.y),
                        f3(i),
                        f3(j)
                    ));
                    let _ = a0;
                }
            }
        }
        if p.closed {
            s.push_str(&format!("G00 X{} Y{}\n", f3(p.pierce.x), f3(p.pierce.y)));
        }
        s.push_str("M00\n");
    }
    s.push_str("M02\n%\n");
    s
}

/// Mach3-flavoured WEDM program. Mach3 uses M3/M5 for the wire / relay and
/// expects a clean G90/G94 header; the geometry is the same compensated path.
fn mach3_wedm(job: &CutJob, opt: &CodeOptions) -> String {
    let mut s = String::new();
    s.push_str("%\n");
    s.push_str("(CHAOYUE CNC WEDM / MACH3)\n");
    s.push_str(&format!("(PART {})\n", clean_comment(&opt.part)));
    s.push_str(&format!(
        "(COMP {:.3} MM SIDE {})\n",
        opt.comp,
        if opt.side > 0 {
            "LEFT"
        } else if opt.side < 0 {
            "RIGHT"
        } else {
            "OFF"
        }
    ));
    s.push_str("G21 G90 G94 G17 G40 G49 G80\n");
    s.push_str("G91.1\n");
    if opt.rapid > 0.0 {
        s.push_str(&format!("(RAPID {:.0} MM/MIN)\n", opt.rapid));
    }
    s.push_str("M05\n");
    let first = job
        .paths
        .first()
        .map(|p| p.pierce)
        .unwrap_or_else(|| Pt::new(0.0, 0.0));
    s.push_str(&format!("G00 X{} Y{}\n", f3(first.x), f3(first.y)));
    for (idx, p) in job.paths.iter().enumerate() {
        if idx > 0 {
            s.push_str(&format!(
                "G00 X{} Y{}\n",
                f3(p.pierce.x),
                f3(p.pierce.y)
            ));
        }
        s.push_str("M03\nG04 P0.3\n");
        if let Some(lead) = &p.lead {
            s.push_str(&format!(
                "G01 X{} Y{} F{}\n",
                f3(lead.end().x),
                f3(lead.end().y),
                f3(opt.feed)
            ));
        }
        for seg in &p.segs {
            match seg {
                Seg::Line { b, .. } => {
                    s.push_str(&format!("G01 X{} Y{} F{}\n", f3(b.x), f3(b.y), f3(opt.feed)));
                }
                Seg::Arc { c, a0, sweep, .. } => {
                    let start = seg.start();
                    let end = seg.end();
                    let i = c.x - start.x;
                    let j = c.y - start.y;
                    let g = if *sweep >= 0.0 { "G03" } else { "G02" };
                    s.push_str(&format!(
                        "{} X{} Y{} I{} J{}\n",
                        g,
                        f3(end.x),
                        f3(end.y),
                        f3(i),
                        f3(j)
                    ));
                    let _ = a0;
                }
            }
        }
        s.push_str("M05\n");
    }
    s.push_str(&format!("G00 X{} Y{}\n", f3(first.x), f3(first.y)));
    s.push_str("M30\n%\n");
    s
}

fn code_3b(job: &CutJob, opt: &CodeOptions) -> String {
    let mut s = String::new();
    let mut current: Option<Pt> = None;
    for p in &job.paths {
        // 3B is always relative to the current position, so jump across to the
        // next pierce point with an explicit straight move when needed.
        if let Some(cur) = current {
            if cur.dist(p.pierce) > 0.01 {
                s.push_str(&b3_line(cur, p.pierce));
                s.push('\n');
            }
        }
        if let Some(lead) = &p.lead {
            s.push_str(&b3_line(lead.start(), lead.end()));
            s.push('\n');
        }
        let mut pos = p.lead.as_ref().map(|l| l.end()).unwrap_or(p.pierce);
        for seg in &p.segs {
            match seg {
                Seg::Line { a, b } => {
                    if a.dist(*b) > 1e-4 {
                        s.push_str(&b3_line(*a, *b));
                        s.push('\n');
                        pos = *b;
                    }
                }
                Seg::Arc { c, r, a0, sweep } => {
                    for (aa, sw) in split_arc(*a0, *sweep) {
                        s.push_str(&b3_arc(*c, *r, aa, sw));
                        s.push('\n');
                    }
                    pos = seg.end();
                }
            }
        }
        current = Some(pos);
    }
    s.push_str("D\n");
    let _ = opt;
    s
}

/// Split an arc at the quadrant boundaries so each 3B block covers at most 90°.
fn split_arc(a0: f32, sweep: f32) -> Vec<(f32, f32)> {
    let mut out = Vec::new();
    let mut start = a0;
    let mut left = sweep;
    let dir = if sweep >= 0.0 { 1.0 } else { -1.0 };
    while left.abs() > 1e-5 {
        // Distance to the next multiple of 90° in the travel direction.
        let step = if dir > 0.0 {
            let next = (start / (PI * 0.5)).floor() * (PI * 0.5) + PI * 0.5;
            (next - start).max(1e-4)
        } else {
            let prev = (start / (PI * 0.5)).ceil() * (PI * 0.5) - PI * 0.5;
            (start - prev).max(1e-4)
        };
        let take = if left.abs() < step || step <= 1e-3 {
            left
        } else {
            dir * step
        };
        out.push((start, take));
        start += take;
        left -= take;
        if out.len() > 64 {
            break;
        }
    }
    out
}

fn b3_line(a: Pt, b: Pt) -> String {
    let dx = (b.x - a.x) * 1000.0;
    let dy = (b.y - a.y) * 1000.0;
    let x = dx.abs().round() as i64;
    let y = dy.abs().round() as i64;
    let j = x.max(y);
    let g = if x >= y { "GX" } else { "GY" };
    let l = match (dx >= 0.0, dy >= 0.0) {
        (true, true) => "L1",
        (false, true) => "L2",
        (false, false) => "L3",
        (true, false) => "L4",
    };
    format!("B {} B {} B {} {} {}", x, y, j, g, l)
}

fn b3_arc(c: Pt, r: f32, a0: f32, sweep: f32) -> String {
    let start = c.add(Pt::from_angle(a0, r));
    let end = c.add(Pt::from_angle(a0 + sweep, r));
    let xs = (start.x - c.x) * 1000.0;
    let ys = (start.y - c.y) * 1000.0;
    let xe = (end.x - c.x) * 1000.0;
    let ye = (end.y - c.y) * 1000.0;
    let x = xs.abs().round() as i64;
    let y = ys.abs().round() as i64;
    let jx = (xe - xs).abs();
    let jy = (ye - ys).abs();
    let j = (jx + jy).round() as i64;
    let g = if jx >= jy { "GX" } else { "GY" };
    let quad = ((norm_angle(a0) / (PI * 0.5)).floor() as i32 % 4) + 1;
    let z = if sweep >= 0.0 {
        format!("NR{}", quad)
    } else {
        format!("SR{}", quad)
    };
    format!("B {} B {} B {} {} {}", x, y, j, g, z)
}

pub fn validate(text: &str, kind: CodeKind) -> Result<String, String> {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        return Err("代码为空，请先生成代码".to_string());
    }
    match kind {
        CodeKind::GCode => {
            if !text.contains("M02") {
                return Err("G 代码缺少程序结束指令 M02".to_string());
            }
            let mut motion = 0;
            for (i, l) in lines.iter().enumerate() {
                let t = l.trim();
                if t.starts_with("G00") || t.starts_with("G01") || t.starts_with("G02") || t.starts_with("G03") {
                    if !t.contains('X') || !t.contains('Y') {
                        return Err(format!("第 {} 行动作缺少 X 或 Y 坐标", i + 1));
                    }
                    motion += 1;
                }
            }
            Ok(format!("代码校验通过：{} 行，{} 条动作指令", lines.len(), motion))
        }
        CodeKind::Mach3 => {
            if !text.contains("M30") {
                return Err("Mach3 程序缺少结束指令 M30".to_string());
            }
            let mut motion = 0;
            let mut wire_on = false;
            let mut wire_starts = 0;
            for (i, l) in lines.iter().enumerate() {
                let t = l.trim();
                if t.starts_with('(') || t.is_empty() || t == "%" || t == "D" {
                    continue;
                }
                let op = t.split_whitespace().next().unwrap_or("");
                if op == "M03" || op == "M3" {
                    wire_on = true;
                    wire_starts += 1;
                } else if op == "M05" || op == "M5" {
                    wire_on = false;
                } else if t.starts_with("G00")
                    || t.starts_with("G01")
                    || t.starts_with("G02")
                    || t.starts_with("G03")
                {
                    if !t.contains('X') || !t.contains('Y') {
                        return Err(format!("第 {} 行动作缺少 X 或 Y 坐标", i + 1));
                    }
                    if (t.starts_with("G02") || t.starts_with("G03"))
                        && (!t.contains('I') || !t.contains('J'))
                    {
                        return Err(format!("第 {} 行圆弧缺少 I/J 圆心偏移", i + 1));
                    }
                    motion += 1;
                }
            }
            if wire_starts == 0 {
                return Err("Mach3 程序没有热丝开启指令 M03".to_string());
            }
            if wire_on {
                return Err("程序结束时热丝仍处于开启状态，缺少 M05".to_string());
            }
            Ok(format!(
                "Mach3 校验通过：{} 行，{} 条动作，热丝开 {} 次",
                lines.len(),
                motion,
                wire_starts
            ))
        }
        CodeKind::ThreeB => {
            let mut bad = 0;
            for (i, l) in lines.iter().enumerate() {
                let t = l.trim();
                if t.starts_with('(') {
                    continue;
                }
                if !t.starts_with('B') || !t.contains(" G") {
                    if t == "D" {
                        continue;
                    }
                    bad += 1;
                    if i < 6 {
                        return Err(format!("第 {} 行不是有效的 3B 程序段", i + 1));
                    }
                }
            }
            if bad > 0 {
                return Err(format!("有 {} 行不是有效的 3B 程序段", bad));
            }
            Ok(format!("3B 代码校验通过：{} 行", lines.len()))
        }
    }
}

pub fn bounds_of_ids(doc: &Doc, ids: &[u64]) -> Option<crate::model::BBox> {
    let mut bb: Option<crate::model::BBox> = None;
    for id in ids {
        if let Some(e) = doc.get(*id) {
            let b = e.bbox();
            if let Some(cur) = &mut bb {
                cur.union(b);
            } else {
                bb = Some(b);
            }
        }
    }
    bb
}
