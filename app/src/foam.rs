//! Hot-wire foam cutting: entry-point optimisation, travel minimisation and
//! Mach3 G-code for two-axis (X/Y) and four-axis (X/Y + A/B) foam tables.
//!
//! A foam cutter does not drill a pierce hole. The wire is switched on at the
//! edge of the block, follows the contour, and is switched off before the next
//! travel move. Closed contours are rotated so they start at the point nearest
//! the previous cut; open contours may be reversed. This directly reduces the
//! amount of hot wire dragged through waste foam.

use crate::codegen::{build_contours, optimize_job, CutJob, Seg};
use crate::model::{Doc, Pt};

#[derive(Clone, Debug)]
pub struct FoamOptions {
    /// Four-axis table: A/B are the top guide, X/Y the bottom guide.
    pub four_axis: bool,
    /// -1 = outside the contour, +1 = inside, 0 = no kerf compensation.
    pub side: i8,
    /// Preferred start /parking point in the foam block.
    pub start: Pt,
    /// Full kerf width in mm; the path is offset by half of it.
    pub kerf: f32,
    /// Cutting feed in mm/min.
    pub feed: f32,
    /// Rapid / off-wire travel speed in mm/min.
    pub rapid: f32,
    /// Lead-in length along the contour tangent.
    pub lead: f32,
    /// Top-guide offset in X/Y for a constant taper (four-axis mode).
    pub taper_x: f32,
    pub taper_y: f32,
    /// Keep the wire on between contours and cut the travel path.
    pub continuous: bool,
    pub optimize: bool,
    pub part: String,
}

#[derive(Clone, Debug)]
pub struct FoamOutput {
    pub text: String,
    pub job: CutJob,
    pub line_count: usize,
    pub path_count: usize,
    pub cut_len: f32,
    pub travel_len: f32,
    pub minutes: f32,
    pub four_axis: bool,
    pub continuous: bool,
    /// Short facts for the code dialog.
    pub extra: String,
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

fn coord(p: Pt, opt: &FoamOptions) -> String {
    if opt.four_axis {
        format!(
            "X{} Y{} A{} B{}",
            f3(p.x),
            f3(p.y),
            f3(p.x + opt.taper_x),
            f3(p.y + opt.taper_y)
        )
    } else {
        format!("X{} Y{}", f3(p.x), f3(p.y))
    }
}

fn add_leads(job: &mut CutJob, lead: f32) -> Result<(), String> {
    for p in &mut job.paths {
        let entry = p.start().ok_or("切割路径为空")?;
        let dir = p
            .segs
            .first()
            .map(|s| s.start_tangent())
            .unwrap_or(Pt::new(1.0, 0.0));
        let approach = entry.sub(dir.mul(lead.max(0.0)));
        p.pierce = approach;
        p.lead = if lead > 0.001 {
            Some(Seg::Line { a: approach, b: entry })
        } else {
            None
        };
    }
    Ok(())
}

fn measure(job: &CutJob, opt: &FoamOptions) -> (f32, f32, f32) {
    let mut cut_len = 0.0f32;
    let mut travel_len = 0.0f32;
    let mut cur = opt.start;
    for p in &job.paths {
        let approach = p
            .lead
            .as_ref()
            .map(|l| l.start())
            .or_else(|| p.start())
            .unwrap_or(cur);
        travel_len += cur.dist(approach);
        cut_len += p.contour_length() + p.lead.as_ref().map(|l| l.length()).unwrap_or(0.0);
        cur = if p.closed {
            p.start().unwrap_or(approach)
        } else {
            p.end().unwrap_or(approach)
        };
    }
    let travel_minutes = travel_len / if opt.continuous { opt.feed } else { opt.rapid };
    let minutes = cut_len / opt.feed + travel_minutes;
    (cut_len, travel_len, minutes)
}

fn prepare(mut job: CutJob, opt: &FoamOptions) -> Result<(CutJob, f32, f32, f32), String> {
    add_leads(&mut job, opt.lead)?;
    let (cut_len, travel_len, minutes) = measure(&job, opt);
    Ok((job, cut_len, travel_len, minutes))
}

pub fn generate(doc: &Doc, ids: &[u64], opt: &FoamOptions) -> Result<FoamOutput, String> {
    if !opt.feed.is_finite() || opt.feed <= 0.1 {
        return Err("切割速度必须大于 0 mm/min".to_string());
    }
    if !opt.rapid.is_finite() || opt.rapid <= 1.0 {
        return Err("空走速度必须大于 1 mm/min".to_string());
    }
    if !opt.kerf.is_finite() || opt.kerf < 0.0 || opt.kerf > 50.0 {
        return Err("割缝宽度应在 0 到 50 mm 之间".to_string());
    }
    if !opt.lead.is_finite() || opt.lead < 0.0 || opt.lead > 500.0 {
        return Err("引入线长度应在 0 到 500 mm 之间".to_string());
    }
    if !opt.taper_x.is_finite() || !opt.taper_y.is_finite() {
        return Err("四轴上端偏移必须是有效数字".to_string());
    }

    let comp = (opt.kerf * 0.5).max(0.0);
    let base = build_contours(doc, ids, comp, opt.side, opt.start)?;
    if base.paths.is_empty() {
        return Err("所选图元无法组成泡沫切割路径".to_string());
    }

    // Try the raw order, an order-only optimisation and an order + rotated
    // start optimisation. Pick the candidate with the smallest rapid travel;
    // this guarantees the "optimised" program is never worse than the raw one.
    let mut candidates = Vec::new();
    if opt.optimize {
        let mut rotated = base.clone();
        optimize_job(&mut rotated, opt.start, false, true);
        candidates.push(prepare(rotated, opt)?);
        let mut ordered = base.clone();
        optimize_job(&mut ordered, opt.start, false, false);
        candidates.push(prepare(ordered, opt)?);
    }
    candidates.push(prepare(base, opt)?);
    let (job, cut_len, travel_len, minutes) = candidates
        .into_iter()
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .expect("foam candidate");

    let mut s = String::new();
    s.push_str("%\n");
    s.push_str("(CHAOYUE CNC FOAM CUT / MACH3)\n");
    s.push_str(&format!("(PART {})\n", clean_comment(&opt.part)));
    s.push_str(&format!(
        "(MODE {})\n",
        if opt.four_axis {
            "4-AXIS X Y A B"
        } else {
            "2-AXIS X Y"
        }
    ));
    s.push_str(&format!(
        "(KERF {:.3} MM  COMP {:.3} MM  FEED {:.0} MM/MIN  LEAD {:.1} MM)\n",
        opt.kerf, comp, opt.feed, opt.lead
    ));
    s.push_str(&format!(
        "(CUT {:.1} MM  TRAVEL {:.1} MM  EST {:.1} MIN)\n",
        cut_len, travel_len, minutes
    ));
    s.push_str("(M03 = HOT WIRE ON, M05 = HOT WIRE OFF)\n");
    if opt.four_axis {
        s.push_str("(X/Y = BOTTOM GUIDE, A/B = TOP GUIDE)\n");
    }
    s.push_str("G21 G90 G94 G17 G40 G49 G80\n");
    s.push_str("G91.1\n");
    s.push_str("M05\n");

    let first_approach = job
        .paths
        .first()
        .and_then(|p| p.lead.as_ref().map(|l| l.start()).or_else(|| p.start()))
        .unwrap_or(opt.start);
    let mut cur = first_approach;
    s.push_str(&format!("G00 {}\n", coord(cur, opt)));
    s.push_str("M03\nG04 P0.3\n");

    for (idx, p) in job.paths.iter().enumerate() {
        let approach = p
            .lead
            .as_ref()
            .map(|l| l.start())
            .or_else(|| p.start())
            .unwrap_or(cur);
        if idx > 0 {
            if opt.continuous {
                // Wire stays hot: the move between contours is a real cut.
                if cur.dist(approach) > 0.001 {
                    s.push_str(&format!(
                        "G01 {} F{}\n",
                        coord(approach, opt),
                        f3(opt.feed)
                    ));
                    cur = approach;
                }
            } else {
                s.push_str("M05\n");
                s.push_str(&format!("G00 {}\n", coord(approach, opt)));
                cur = approach;
                s.push_str("M03\nG04 P0.3\n");
            }
        }

        let entry = p.start().unwrap_or(approach);
        if cur.dist(entry) > 0.001 {
            s.push_str(&format!("G01 {} F{}\n", coord(entry, opt), f3(opt.feed)));
            cur = entry;
        }

        for seg in &p.segs {
            let start = seg.start();
            if cur.dist(start) > 0.002 {
                s.push_str(&format!("G01 {} F{}\n", coord(start, opt), f3(opt.feed)));
                cur = start;
            }
            match seg {
                Seg::Line { b, .. } => {
                    if cur.dist(*b) > 1e-4 {
                        s.push_str(&format!("G01 {} F{}\n", coord(*b, opt), f3(opt.feed)));
                        cur = *b;
                    }
                }
                Seg::Arc { c, sweep, .. } => {
                    let end = seg.end();
                    let i = c.x - start.x;
                    let j = c.y - start.y;
                    let g = if *sweep >= 0.0 { "G03" } else { "G02" };
                    s.push_str(&format!(
                        "{} {} I{} J{} F{}\n",
                        g,
                        coord(end, opt),
                        f3(i),
                        f3(j),
                        f3(opt.feed)
                    ));
                    cur = end;
                }
            }
        }

        if p.closed {
            let close = p.start().unwrap_or(entry);
            if cur.dist(close) > 0.002 {
                s.push_str(&format!("G01 {} F{}\n", coord(close, opt), f3(opt.feed)));
                cur = close;
            }
        }
    }

    s.push_str("M05\n");
    s.push_str(&format!("G00 {}\n", coord(opt.start, opt)));
    s.push_str("M30\n%\n");

    let line_count = s.lines().filter(|l| !l.trim().is_empty()).count();
    let extra = format!(
        "{} · {} 条路径 · 切割 {:.1} mm · 空走 {:.1} mm · 约 {:.1} 分钟",
        if opt.four_axis { "四轴 XYAB" } else { "两轴 XY" },
        job.paths.len(),
        cut_len,
        travel_len,
        minutes
    );
    Ok(FoamOutput {
        text: s,
        path_count: job.paths.len(),
        job,
        line_count,
        cut_len,
        travel_len,
        minutes,
        four_axis: opt.four_axis,
        continuous: opt.continuous,
        extra,
    })
}
