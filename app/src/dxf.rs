//! Minimal DXF reader for contours made of LINE, CIRCLE, ARC and LWPOLYLINE.
//! DXF coordinates are already in millimetres with Y pointing up, matching the
//! drawing model, so no axis conversion is needed.

use crate::model::{Kind, Pt};
use std::f32::consts::{FRAC_PI_4, TAU};

pub fn import(text: &str) -> Result<Vec<Kind>, String> {
    let mut lines = text.lines();
    let mut pairs: Vec<(i32, String)> = Vec::new();
    while let (Some(c), Some(v)) = (lines.next(), lines.next()) {
        if let Ok(code) = c.trim().parse::<i32>() {
            pairs.push((code, v.trim().to_string()));
        }
    }
    let mut kinds = Vec::new();
    let mut in_entities = false;
    let mut i = 0usize;
    while i < pairs.len() {
        let (code, val) = (&pairs[i].0, &pairs[i].1);
        if *code == 0 && val == "SECTION" {
            if i + 1 < pairs.len() && pairs[i + 1].0 == 2 && pairs[i + 1].1 == "ENTITIES" {
                in_entities = true;
                i += 2;
                continue;
            }
        }
        if *code == 0 && val == "ENDSEC" {
            in_entities = false;
        }
        if in_entities && *code == 0 {
            let name = val.as_str();
            let start = i + 1;
            let mut j = start;
            while j < pairs.len() && pairs[j].0 != 0 {
                j += 1;
            }
            let ent = &pairs[start..j];
            match name {
                "LINE" => {
                    if let (Some(x1), Some(y1), Some(x2), Some(y2)) = (
                        get(ent, 10, 0),
                        get(ent, 20, 0),
                        get(ent, 11, 0),
                        get(ent, 21, 0),
                    ) {
                        kinds.push(Kind::Line {
                            a: Pt::new(x1, y1),
                            b: Pt::new(x2, y2),
                        });
                    }
                }
                "CIRCLE" => {
                    if let (Some(x), Some(y), Some(r)) = (get(ent, 10, 0), get(ent, 20, 0), get(ent, 40, 0)) {
                        if r.abs() > 1e-6 {
                            kinds.push(Kind::Circle {
                                c: Pt::new(x, y),
                                r: r.abs(),
                            });
                        }
                    }
                }
                "ARC" => {
                    if let (Some(x), Some(y), Some(r), Some(a), Some(b)) = (
                        get(ent, 10, 0),
                        get(ent, 20, 0),
                        get(ent, 40, 0),
                        get(ent, 50, 0),
                        get(ent, 51, 0),
                    ) {
                        if r.abs() > 1e-6 {
                            let a0 = a.to_radians();
                            let a1 = b.to_radians();
                            let mut sweep = (a1 - a0).rem_euclid(TAU);
                            if sweep < 1e-5 {
                                sweep = TAU;
                            }
                            kinds.push(Kind::Arc {
                                c: Pt::new(x, y),
                                r: r.abs(),
                                a0,
                                sweep,
                            });
                        }
                    }
                }
                "LWPOLYLINE" => {
                    let closed = get_i(ent, 70).unwrap_or(0) & 1 != 0;
                    let mut verts: Vec<(Pt, f32)> = Vec::new();
                    let mut cx: Option<f32> = None;
                    let mut cy: Option<f32> = None;
                    for (c, v) in ent {
                        match *c {
                            10 => {
                                cx = v.parse::<f32>().ok();
                                cy = None;
                            }
                            20 => {
                                cy = v.parse::<f32>().ok();
                                if let (Some(x), Some(y)) = (cx, cy) {
                                    verts.push((Pt::new(x, y), 0.0));
                                }
                            }
                            42 => {
                                if let Some(last) = verts.last_mut() {
                                    last.1 = v.parse::<f32>().unwrap_or(0.0);
                                }
                            }
                            _ => {}
                        }
                    }
                    let n = verts.len();
                    if n >= 2 {
                        let segments = if closed { n } else { n - 1 };
                        for k in 0..segments {
                            let (p1, bulge) = verts[k];
                            let p2 = verts[(k + 1) % n].0;
                            if let Some(kind) = bulge_kind(p1, p2, bulge) {
                                kinds.push(kind);
                            }
                        }
                    }
                }
                _ => {}
            }
            i = j;
            continue;
        }
        i += 1;
    }
    if kinds.is_empty() {
        Err("DXF 中没有找到 LINE / CIRCLE / ARC / LWPOLYLINE 图元".to_string())
    } else {
        Ok(kinds)
    }
}

fn get(ent: &[(i32, String)], code: i32, index: usize) -> Option<f32> {
    ent.iter()
        .filter(|(c, _)| *c == code)
        .nth(index)
        .and_then(|(_, v)| v.parse::<f32>().ok())
}

fn get_i(ent: &[(i32, String)], code: i32) -> Option<i32> {
    ent.iter()
        .find(|(c, _)| *c == code)
        .and_then(|(_, v)| v.parse::<i32>().ok())
}

/// Convert a polyline segment plus its DXF bulge into a line or an arc.
fn bulge_kind(p1: Pt, p2: Pt, bulge: f32) -> Option<Kind> {
    let chord = p1.dist(p2);
    if chord < 1e-7 {
        return None;
    }
    if bulge.abs() < 1e-6 {
        return Some(Kind::Line { a: p1, b: p2 });
    }
    let theta = 4.0 * bulge.atan();
    if theta.abs() < 1e-5 {
        return Some(Kind::Line { a: p1, b: p2 });
    }
    let h = chord / (2.0 * (theta * 0.5).tan());
    let mid = p1.lerp(p2, 0.5);
    let left = p2.sub(p1).norm().left();
    let c = mid.add(left.mul(h));
    let r = c.dist(p1);
    if r < 1e-6 {
        return Some(Kind::Line { a: p1, b: p2 });
    }
    if theta > 0.0 {
        let a0 = p1.sub(c).angle();
        Some(Kind::Arc { c, r, a0, sweep: theta })
    } else {
        // Model arcs are stored counter-clockwise, so reverse a clockwise bulge.
        let a0 = p2.sub(c).angle();
        Some(Kind::Arc { c, r, a0, sweep: -theta })
    }
}

/// Approximate a polyline from a list of points (used by tests and importers).
pub fn polyline_from_points(pts: &[Pt], closed: bool) -> Vec<Kind> {
    let mut out = Vec::new();
    if pts.len() < 2 {
        return out;
    }
    for i in 0..pts.len() - 1 {
        out.push(Kind::Line { a: pts[i], b: pts[i + 1] });
    }
    if closed && pts[0].dist(*pts.last().unwrap()) > 1e-6 {
        out.push(Kind::Line {
            a: *pts.last().unwrap(),
            b: pts[0],
        });
    }
    let _ = FRAC_PI_4;
    out
}
