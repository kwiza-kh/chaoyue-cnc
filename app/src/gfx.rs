//! Small software-drawn user interface toolkit used by the CNC program.
//! Everything is painted into an RGBA pixmap with tiny-skia and then blitted
//! to the Win32 window by the platform layer.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use tiny_skia::{
    Color as SkColor, FillRule, LineCap, LineJoin, Paint, Path, PathBuilder, Pixmap,
    Rect as SkRect, Shader, Stroke, StrokeDash, Transform,
};
use ttf_parser::{Face, OutlineBuilder};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
    pub const fn from_hex(hex: u32) -> Self {
        Self {
            r: ((hex >> 16) & 0xff) as u8,
            g: ((hex >> 8) & 0xff) as u8,
            b: (hex & 0xff) as u8,
            a: 255,
        }
    }
    pub fn with_alpha(self, a: u8) -> Self {
        Self { a, ..self }
    }
    pub fn sk(self) -> SkColor {
        SkColor::from_rgba8(self.r, self.g, self.b, self.a)
    }
    pub fn mix(self, other: Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        Color {
            r: (self.r as f32 + (other.r as f32 - self.r as f32) * t) as u8,
            g: (self.g as f32 + (other.g as f32 - self.g as f32) * t) as u8,
            b: (self.b as f32 + (other.b as f32 - self.b as f32) * t) as u8,
            a: (self.a as f32 + (other.a as f32 - self.a as f32) * t) as u8,
        }
    }
}

pub const C_BG: Color = Color::from_hex(0xF0F3F7);
pub const C_MENU: Color = Color::from_hex(0xF8FAFC);
pub const C_TOOLBAR_TOP: Color = Color::from_hex(0xFDFEFF);
pub const C_TOOLBAR_BOTTOM: Color = Color::from_hex(0xE5EBF2);
pub const C_BORDER: Color = Color::from_hex(0xC7D1DD);
pub const C_BORDER_SOFT: Color = Color::from_hex(0xDCE3EB);
pub const C_TEXT: Color = Color::from_hex(0x1E2935);
pub const C_TEXT_DIM: Color = Color::from_hex(0x64717F);
pub const C_TEXT_LIGHT: Color = Color::from_hex(0x8A96A3);
pub const C_ACCENT: Color = Color::from_hex(0x2B6CB0);
pub const C_ACCENT_DARK: Color = Color::from_hex(0x1E4F87);
pub const C_ACCENT_SOFT: Color = Color::from_hex(0xE1ECF8);
pub const C_WHITE: Color = Color::from_hex(0xFFFFFF);
pub const C_CANVAS: Color = Color::from_hex(0x0B1118);
pub const C_CANVAS_2: Color = Color::from_hex(0x0E1822);
pub const C_GRID: Color = Color::from_hex(0x16202B);
pub const C_GRID_MAJOR: Color = Color::from_hex(0x213042);
pub const C_AXIS_X: Color = Color::from_hex(0x6B3B3B);
pub const C_AXIS_Y: Color = Color::from_hex(0x2C5540);
pub const C_ENTITY: Color = Color::from_hex(0xD9E4F0);
pub const C_ENTITY_DIM: Color = Color::from_hex(0x536477);
pub const C_SELECT: Color = Color::from_hex(0x22D3EE);
pub const C_CUT: Color = Color::from_hex(0xFF5E62);
pub const C_PIERCE: Color = Color::from_hex(0xFFC857);
pub const C_GREEN: Color = Color::from_hex(0x25A06B);

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x <= self.x + self.w && y <= self.y + self.h
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.w * 0.5, self.y + self.h * 0.5)
    }
    pub fn inset(&self, d: f32) -> Rect {
        Rect::new(self.x + d, self.y + d, (self.w - d * 2.0).max(0.0), (self.h - d * 2.0).max(0.0))
    }
    pub fn inset_xy(&self, dx: f32, dy: f32) -> Rect {
        Rect::new(self.x + dx, self.y + dy, (self.w - dx * 2.0).max(0.0), (self.h - dy * 2.0).max(0.0))
    }
    pub fn offset(&self, dx: f32, dy: f32) -> Rect {
        Rect::new(self.x + dx, self.y + dy, self.w, self.h)
    }
    pub fn intersect(&self, o: Rect) -> Option<Rect> {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = self.right().min(o.right());
        let y1 = self.bottom().min(o.bottom());
        if x1 > x0 && y1 > y0 {
            Some(Rect::new(x0, y0, x1 - x0, y1 - y0))
        } else {
            None
        }
    }
    pub fn sk(&self) -> SkRect {
        SkRect::from_xywh(self.x, self.y, self.w, self.h).unwrap_or_else(|| SkRect::from_xywh(0.0, 0.0, 0.0, 0.0).unwrap())
    }
}

struct GlyphBuilder {
    pb: PathBuilder,
    ox: f32,
    baseline: f32,
    scale: f32,
}

impl GlyphBuilder {
    fn point(&self, x: f32, y: f32) -> (f32, f32) {
        (self.ox + x * self.scale, self.baseline - y * self.scale)
    }
}

impl OutlineBuilder for GlyphBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.point(x, y);
        self.pb.move_to(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.point(x, y);
        self.pb.line_to(x, y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (x1, y1) = self.point(x1, y1);
        let (x, y) = self.point(x, y);
        self.pb.quad_to(x1, y1, x, y);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (x1, y1) = self.point(x1, y1);
        let (x2, y2) = self.point(x2, y2);
        let (x, y) = self.point(x, y);
        self.pb.cubic_to(x1, y1, x2, y2, x, y);
    }
    fn close(&mut self) {
        self.pb.close();
    }
}

pub struct FontFace {
    face: Face<'static>,
    id: u8,
}

struct TextBitmap {
    pixmap: Pixmap,
    ascent: f32,
    pad: i32,
}

fn blend_pixmap_at(dst: &mut Pixmap, x: i32, y: i32, src: &Pixmap) {
    let sw = src.width() as i32;
    let sh = src.height() as i32;
    let dw = dst.width() as i32;
    let dh = dst.height() as i32;
    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x + sw).min(dw);
    let y1 = (y + sh).min(dh);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let swidth = src.width() as usize;
    let dwidth = dst.width() as usize;
    let src_data = src.data();
    let dst_data = dst.data_mut();
    for row in y0..y1 {
        let srow = (row - y) as usize * swidth;
        let drow = row as usize * dwidth;
        for col in x0..x1 {
            let si = (srow + (col - x) as usize) * 4;
            let di = (drow + col as usize) * 4;
            let sa = src_data[si + 3] as u32;
            if sa == 0 {
                continue;
            }
            if sa == 255 {
                dst_data[di..di + 4].copy_from_slice(&src_data[si..si + 4]);
            } else {
                let inv = 255 - sa;
                dst_data[di] =
                    (src_data[si] as u32 + dst_data[di] as u32 * inv / 255).min(255) as u8;
                dst_data[di + 1] =
                    (src_data[si + 1] as u32 + dst_data[di + 1] as u32 * inv / 255).min(255) as u8;
                dst_data[di + 2] =
                    (src_data[si + 2] as u32 + dst_data[di + 2] as u32 * inv / 255).min(255) as u8;
                dst_data[di + 3] = 255;
            }
        }
    }
}

impl FontFace {
    fn load(path: &str, index: u32, id: u8) -> Option<Self> {
        let bytes = std::fs::read(path).ok()?;
        let data: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        let face = Face::parse(data, index).ok()?;
        Some(Self { face, id })
    }
    fn scale(&self, size: f32) -> f32 {
        size / self.face.units_per_em() as f32
    }
    pub fn ascent(&self, size: f32) -> f32 {
        self.face.ascender() as f32 * self.scale(size)
    }
    pub fn descent(&self, size: f32) -> f32 {
        (-self.face.descender() as f32) * self.scale(size)
    }
    pub fn advance(&self, ch: char, size: f32) -> f32 {
        match self.face.glyph_index(ch) {
            Some(gid) => self
                .face
                .glyph_hor_advance(gid)
                .unwrap_or(self.face.units_per_em() / 2) as f32
                * self.scale(size),
            None => size * 0.55,
        }
    }
    pub fn width(&self, text: &str, size: f32) -> f32 {
        text.chars().map(|c| self.advance(c, size)).sum()
    }
    pub fn draw(&self, pixmap: &mut Pixmap, x: f32, baseline: f32, text: &str, size: f32, color: Color) {
        if size <= 0.5 || text.is_empty() {
            return;
        }
        static TEXT_CACHE: OnceLock<Mutex<HashMap<(u8, String, u32, u32), TextBitmap>>> =
            OnceLock::new();
        let cache = TEXT_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        let color_key = (color.r as u32) << 24
            | (color.g as u32) << 16
            | (color.b as u32) << 8
            | color.a as u32;
        let key = (self.id, text.to_string(), size.to_bits(), color_key);
        let Ok(mut guard) = cache.lock() else { return };
        let bmp = guard.entry(key).or_insert_with(|| {
            let width = self.width(text, size).ceil().max(1.0) as u32;
            let ascent = self.ascent(size);
            let descent = self.descent(size);
            let pad = 4i32;
            let w = width + pad as u32 * 2;
            let h = (ascent + descent).ceil().max(1.0) as u32 + pad as u32 * 2;
            let mut tmp = Pixmap::new(w.max(1), h.max(1)).expect("text bitmap");
            if let Some(path) = self.text_path(text, size) {
                if let Some(path) =
                    path.transform(Transform::from_translate(pad as f32, pad as f32 + ascent))
                {
                    let mut paint = Paint::default();
                    paint.set_color(color.sk());
                    paint.anti_alias = true;
                    tmp.fill_path(
                        &path,
                        &paint,
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }
            }
            TextBitmap {
                pixmap: tmp,
                ascent,
                pad,
            }
        });
        let px = x.round() as i32 - bmp.pad;
        let py = (baseline - bmp.ascent).round() as i32 - bmp.pad;
        blend_pixmap_at(pixmap, px, py, &bmp.pixmap);
        if guard.len() > 2048 {
            guard.clear();
        }
    }

    /// Build (and cache) the whole text run as one path at the origin. Drawing a
    /// label then costs one transform and one fill instead of one outline lookup
    /// and fill per character, which is what makes the board feel smooth while
    /// the crosshair moves.
    fn text_path(&self, text: &str, size: f32) -> Option<Path> {
        static RUNS: OnceLock<Mutex<HashMap<(u8, String, u32), Path>>> = OnceLock::new();
        let cache = RUNS.get_or_init(|| Mutex::new(HashMap::new()));
        let key = (self.id, text.to_string(), size.to_bits());
        if let Ok(mut guard) = cache.lock() {
            if let Some(p) = guard.get(&key) {
                return Some(p.clone());
            }
        }
        let scale = self.scale(size);
        let mut pen = 0.0f32;
        let mut pb = PathBuilder::new();
        let mut any = false;
        for ch in text.chars() {
            if !matches!(ch, '\n' | '\r') {
                if let Some(gid) = self.face.glyph_index(ch) {
                    let mut b = GlyphBuilder {
                        pb,
                        ox: pen,
                        baseline: 0.0,
                        scale,
                    };
                    if self.face.outline_glyph(gid, &mut b).is_some() {
                        any = true;
                        pb = b.pb;
                    } else {
                        pb = b.pb;
                    }
                }
            }
            pen += self.advance(ch, size);
        }
        if !any {
            return None;
        }
        let path = pb.finish()?;
        if let Ok(mut guard) = cache.lock() {
            if guard.len() > 4096 {
                guard.clear();
            }
            guard.insert(key, path.clone());
        }
        Some(path)
    }
}

pub struct Fonts {
    pub ui: FontFace,
    pub bold: Option<FontFace>,
    pub mono: Option<FontFace>,
}

impl Fonts {
    pub fn bold(&self) -> &FontFace {
        self.bold.as_ref().unwrap_or(&self.ui)
    }
    pub fn mono(&self) -> &FontFace {
        self.mono.as_ref().unwrap_or(&self.ui)
    }
}

static FONTS: OnceLock<Fonts> = OnceLock::new();

fn load_first(paths: &[(&str, u32)], id: u8) -> Option<FontFace> {
    for (p, i) in paths {
        if let Some(f) = FontFace::load(p, *i, id) {
            return Some(f);
        }
    }
    None
}

pub fn fonts() -> &'static Fonts {
    FONTS.get_or_init(|| {
        let ui = load_first(
            &[
                ("C:/Windows/Fonts/simsun.ttc", 0),
                ("C:/Windows/Fonts/simhei.ttf", 0),
                ("C:/Windows/Fonts/Deng.ttf", 0),
                ("C:/Windows/Fonts/msyh.ttc", 0),
            ],
            0,
        )
        .expect("no usable CJK font found");
        let bold = load_first(
            &[
                ("C:/Windows/Fonts/simhei.ttf", 0),
                ("C:/Windows/Fonts/Dengb.ttf", 0),
                ("C:/Windows/Fonts/msyhbd.ttc", 0),
            ],
            1,
        );
        let mono = load_first(
            &[
                ("C:/Windows/Fonts/consola.ttf", 0),
                ("C:/Windows/Fonts/cour.ttf", 0),
            ],
            2,
        );
        Fonts { ui, bold, mono }
    })
}

pub struct Painter {
    pub pixmap: Pixmap,
}

impl Painter {
    pub fn new(w: u32, h: u32) -> Self {
        let pixmap = Pixmap::new(w.max(1), h.max(1)).expect("pixmap");
        Self { pixmap }
    }
    pub fn resize(&mut self, w: u32, h: u32) {
        if self.pixmap.width() != w.max(1) || self.pixmap.height() != h.max(1) {
            self.pixmap = Pixmap::new(w.max(1), h.max(1)).expect("pixmap");
        }
    }
    pub fn width(&self) -> f32 {
        self.pixmap.width() as f32
    }
    pub fn height(&self) -> f32 {
        self.pixmap.height() as f32
    }
    pub fn clear(&mut self, c: Color) {
        self.pixmap.fill(c.sk());
    }
    pub fn fill_rect(&mut self, r: Rect, c: Color) {
        if r.w <= 0.0 || r.h <= 0.0 {
            return;
        }
        let paint = Paint {
            shader: Shader::SolidColor(c.sk()),
            anti_alias: false,
            ..Default::default()
        };
        self.pixmap.fill_rect(r.sk(), &paint, Transform::identity(), None);
    }
    pub fn vgradient(&mut self, r: Rect, top: Color, bottom: Color) {
        if r.w <= 0.0 || r.h <= 0.0 {
            return;
        }
        // Row-wise software gradient. tiny-skia's shader path costs several
        // milliseconds for a toolbar; this is a simple memory fill and keeps
        // the whole window under one frame.
        let pw = self.pixmap.width() as f32;
        let ph = self.pixmap.height() as f32;
        let x0 = r.x.floor().clamp(0.0, pw) as u32;
        let x1 = r.right().ceil().clamp(0.0, pw) as u32;
        let y0 = r.y.floor().clamp(0.0, ph) as u32;
        let y1 = r.bottom().ceil().clamp(0.0, ph) as u32;
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let width = self.pixmap.width() as usize;
        let data = self.pixmap.data_mut();
        for y in y0..y1 {
            let t = ((y as f32 + 0.5 - r.y) / r.h).clamp(0.0, 1.0);
            let c = top.mix(bottom, t);
            let a = c.a as u32;
            let (cr, cg, cb) = if a == 255 {
                (c.r, c.g, c.b)
            } else {
                (
                    (c.r as u32 * a / 255) as u8,
                    (c.g as u32 * a / 255) as u8,
                    (c.b as u32 * a / 255) as u8,
                )
            };
            let px = [cr, cg, cb, a as u8];
            let row = y as usize * width;
            for x in x0..x1 {
                let i = (row + x as usize) * 4;
                data[i] = px[0];
                data[i + 1] = px[1];
                data[i + 2] = px[2];
                data[i + 3] = px[3];
            }
        }
    }
    pub fn fill_round_rect(&mut self, r: Rect, radius: f32, c: Color) {
        let mut pb = PathBuilder::new();
        push_round_rect(&mut pb, r, radius);
        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color(c.sk());
            paint.anti_alias = true;
            self.pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
        }
    }
    pub fn stroke_rect(&mut self, r: Rect, c: Color, width: f32) {
        let mut pb = PathBuilder::new();
        pb.move_to(r.x, r.y);
        pb.line_to(r.right(), r.y);
        pb.line_to(r.right(), r.bottom());
        pb.line_to(r.x, r.bottom());
        pb.close();
        self.stroke_path(pb, c, width, false);
    }
    pub fn line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, c: Color, width: f32) {
        let mut pb = PathBuilder::new();
        pb.move_to(x1, y1);
        pb.line_to(x2, y2);
        self.stroke_path(pb, c, width, false);
    }
    pub fn dashed_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, c: Color, width: f32, dash: f32) {
        let mut pb = PathBuilder::new();
        pb.move_to(x1, y1);
        pb.line_to(x2, y2);
        let mut stroke = Stroke::default();
        stroke.width = width;
        stroke.line_cap = LineCap::Butt;
        stroke.dash = StrokeDash::new(vec![dash, dash * 0.8], 0.0);
        let mut paint = Paint::default();
        paint.set_color(c.sk());
        paint.anti_alias = true;
        if let Some(path) = pb.finish() {
            self.pixmap
                .stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    }
    pub fn stroke_circle(&mut self, cx: f32, cy: f32, radius: f32, c: Color, width: f32) {
        if radius <= 0.0 {
            return;
        }
        let mut pb = PathBuilder::new();
        pb.push_circle(cx, cy, radius);
        self.stroke_path(pb, c, width, false);
    }
    pub fn fill_circle(&mut self, cx: f32, cy: f32, radius: f32, c: Color) {
        if radius <= 0.0 {
            return;
        }
        let mut pb = PathBuilder::new();
        pb.push_circle(cx, cy, radius);
        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color(c.sk());
            paint.anti_alias = true;
            self.pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
        }
    }
    pub fn stroke_polyline(&mut self, pts: &[(f32, f32)], c: Color, width: f32, closed: bool) {
        if pts.len() < 2 {
            return;
        }
        let mut pb = PathBuilder::new();
        pb.move_to(pts[0].0, pts[0].1);
        for p in &pts[1..] {
            pb.line_to(p.0, p.1);
        }
        if closed {
            pb.close();
        }
        self.stroke_path(pb, c, width, closed);
    }
    pub fn stroke_polyline_dashed(
        &mut self,
        pts: &[(f32, f32)],
        c: Color,
        width: f32,
        dash: f32,
    ) {
        if pts.len() < 2 {
            return;
        }
        let mut pb = PathBuilder::new();
        pb.move_to(pts[0].0, pts[0].1);
        for p in &pts[1..] {
            pb.line_to(p.0, p.1);
        }
        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color(c.sk());
            paint.anti_alias = true;
            let mut stroke = Stroke::default();
            stroke.width = width.max(0.5);
            stroke.line_cap = LineCap::Butt;
            stroke.line_join = LineJoin::Round;
            stroke.dash = StrokeDash::new(vec![dash, dash * 0.7], 0.0);
            self.pixmap
                .stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    }
    pub fn fill_poly(&mut self, pts: &[(f32, f32)], c: Color) {
        if pts.len() < 3 {
            return;
        }
        let mut pb = PathBuilder::new();
        pb.move_to(pts[0].0, pts[0].1);
        for p in &pts[1..] {
            pb.line_to(p.0, p.1);
        }
        pb.close();
        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color(c.sk());
            paint.anti_alias = true;
            self.pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
        }
    }
    pub fn stroke_arc(
        &mut self,
        cx: f32,
        cy: f32,
        radius: f32,
        a0: f32,
        sweep: f32,
        c: Color,
        width: f32,
    ) {
        if radius <= 0.0 || sweep.abs() < 1e-5 {
            return;
        }
        let steps = ((sweep.abs() * radius / 4.0).ceil() as usize).clamp(6, 180);
        let mut pts = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let a = a0 + sweep * (i as f32 / steps as f32);
            pts.push((cx + radius * a.cos(), cy + radius * a.sin()));
        }
        self.stroke_polyline(&pts, c, width, false);
    }
    fn stroke_path(&mut self, pb: PathBuilder, c: Color, width: f32, _closed: bool) {
        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color(c.sk());
            paint.anti_alias = true;
            let mut stroke = Stroke::default();
            stroke.width = width.max(0.5);
            stroke.line_cap = LineCap::Round;
            stroke.line_join = LineJoin::Round;
            self.pixmap
                .stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    }
    pub fn text(&mut self, x: f32, baseline: f32, text: &str, size: f32, c: Color) -> f32 {
        let f = fonts();
        f.ui.draw(&mut self.pixmap, x, baseline, text, size, c);
        f.ui.width(text, size)
    }
    pub fn text_bold(&mut self, x: f32, baseline: f32, text: &str, size: f32, c: Color) -> f32 {
        let f = fonts();
        let face = f.bold();
        face.draw(&mut self.pixmap, x, baseline, text, size, c);
        face.width(text, size)
    }
    pub fn text_mono(&mut self, x: f32, baseline: f32, text: &str, size: f32, c: Color) -> f32 {
        let f = fonts();
        let face = f.mono();
        face.draw(&mut self.pixmap, x, baseline, text, size, c);
        face.width(text, size)
    }
    pub fn text_width(&self, text: &str, size: f32) -> f32 {
        fonts().ui.width(text, size)
    }
    pub fn text_width_bold(&self, text: &str, size: f32) -> f32 {
        fonts().bold().width(text, size)
    }
    pub fn text_width_mono(&self, text: &str, size: f32) -> f32 {
        fonts().mono().width(text, size)
    }
    pub fn text_top(&mut self, x: f32, y: f32, text: &str, size: f32, c: Color) -> f32 {
        let ascent = fonts().ui.ascent(size);
        self.text(x, y + ascent, text, size, c)
    }
    pub fn text_top_bold(&mut self, x: f32, y: f32, text: &str, size: f32, c: Color) -> f32 {
        let ascent = fonts().bold().ascent(size);
        self.text_bold(x, y + ascent, text, size, c)
    }
    pub fn text_centered(&mut self, r: Rect, text: &str, size: f32, c: Color) {
        let w = fonts().ui.width(text, size);
        let ascent = fonts().ui.ascent(size);
        let x = r.x + (r.w - w) * 0.5;
        let y = r.y + (r.h - size) * 0.5 + ascent;
        self.text(x, y, text, size, c);
    }
    pub fn text_centered_bold(&mut self, r: Rect, text: &str, size: f32, c: Color) {
        let face = fonts().bold();
        let w = face.width(text, size);
        let ascent = face.ascent(size);
        let x = r.x + (r.w - w) * 0.5;
        let y = r.y + (r.h - size) * 0.5 + ascent;
        self.text_bold(x, y, text, size, c);
    }
    pub fn text_top_centered(&mut self, cx: f32, y: f32, text: &str, size: f32, c: Color) -> f32 {
        let w = fonts().ui.width(text, size);
        self.text_top(cx - w * 0.5, y, text, size, c);
        w
    }
    pub fn text_right(&mut self, right: f32, baseline: f32, text: &str, size: f32, c: Color) {
        let w = fonts().ui.width(text, size);
        self.text(right - w, baseline, text, size, c);
    }
    pub fn text_top_right(&mut self, right: f32, y: f32, text: &str, size: f32, c: Color) {
        let ascent = fonts().ui.ascent(size);
        self.text_right(right, y + ascent, text, size, c);
    }
    pub fn text_wrapped(&mut self, x: f32, y: f32, max_w: f32, text: &str, size: f32, c: Color, line_h: f32) -> f32 {
        let mut py = y;
        for para in text.split('\n') {
            let mut line = String::new();
            for ch in para.chars() {
                let mut test = line.clone();
                test.push(ch);
                if self.text_width(&test, size) > max_w && !line.is_empty() {
                    self.text_top(x, py, &line, size, c);
                    py += line_h;
                    line.clear();
                }
                line.push(ch);
            }
            if !line.is_empty() {
                self.text_top(x, py, &line, size, c);
                py += line_h;
            } else if para.is_empty() {
                py += line_h;
            }
        }
        py
    }
    pub fn blend_pixmap_at(&mut self, x: i32, y: i32, src: &Pixmap) {
        blend_pixmap_at(&mut self.pixmap, x, y, src);
    }
    /// Fast copy for opaque layers. tiny-skia's general pixmap painter is
    /// surprisingly slow for a full-canvas background, while a row copy is a
    /// few hundred microseconds.
    pub fn copy_pixmap_at(&mut self, x: i32, y: i32, src: &Pixmap) {
        let sw = src.width() as i32;
        let sh = src.height() as i32;
        let dw = self.pixmap.width() as i32;
        let dh = self.pixmap.height() as i32;
        let x0 = x.max(0);
        let y0 = y.max(0);
        let x1 = (x + sw).min(dw);
        let y1 = (y + sh).min(dh);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let row_bytes = ((x1 - x0) as usize) * 4;
        let src_stride = src.width() as usize;
        let dst_stride = self.pixmap.width() as usize;
        let src_data = src.data();
        let dst_data = self.pixmap.data_mut();
        for row in y0..y1 {
            let src_off = ((row - y) as usize * src_stride + (x0 - x) as usize) * 4;
            let dst_off = (row as usize * dst_stride + x0 as usize) * 4;
            dst_data[dst_off..dst_off + row_bytes]
                .copy_from_slice(&src_data[src_off..src_off + row_bytes]);
        }
    }
    pub fn png(&self) -> Result<Vec<u8>, String> {
        self.pixmap.encode_png().map_err(|e| e.to_string())
    }
}

fn push_round_rect(pb: &mut PathBuilder, r: Rect, radius: f32) {
    let radius = radius.min(r.w * 0.5).min(r.h * 0.5).max(0.0);
    if radius <= 0.1 {
        pb.push_rect(r.sk());
        return;
    }
    let (x0, y0, x1, y1) = (r.x, r.y, r.right(), r.bottom());
    pb.move_to(x0 + radius, y0);
    pb.line_to(x1 - radius, y0);
    pb.quad_to(x1, y0, x1, y0 + radius);
    pb.line_to(x1, y1 - radius);
    pb.quad_to(x1, y1, x1 - radius, y1);
    pb.line_to(x0 + radius, y1);
    pb.quad_to(x0, y1, x0, y1 - radius);
    pb.line_to(x0, y0 + radius);
    pb.quad_to(x0, y0, x0 + radius, y0);
    pb.close();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    New,
    Open,
    Save,
    Print,
    Preview,
    Info,
    Calc,
    Undo,
    Redo,
    Redraw,
    Pan,
    Zoom,
    ZoomWindow,
    Fit,
    Point,
    Line,
    Polyline,
    Rect,
    Circle,
    Arc,
    Delete,
    Copy,
    Mirror,
    Offset,
    Array,
    Move,
    Rotate,
    Scale,
    Trim,
    Snap,
    SnapEnd,
    SnapMid,
    SnapCenter,
    SnapNode,
    SnapNear,
    Gear,
    Code,
    Transfer,
    Check,
    Help,
    Close,
    Arrow,
    Folder,
    File,
    Select,
}

pub fn draw_icon(p: &mut Painter, icon: Icon, r: Rect, color: Color) {
    let s = r.w.min(r.h);
    if s < 1.0 {
        return;
    }
    static CACHE: OnceLock<Mutex<HashMap<(Icon, u32, u32), Pixmap>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let px = s.ceil() as u32;
    let color_key = (color.r as u32) << 24
        | (color.g as u32) << 16
        | (color.b as u32) << 8
        | color.a as u32;
    let key = (icon, px, color_key);
    let Ok(mut guard) = cache.lock() else { return };
    let bmp = guard.entry(key).or_insert_with(|| {
        let pad = 2u32;
        let mut tmp = Pixmap::new(px + pad * 2, px + pad * 2).expect("icon bitmap");
        let mut sp = Painter { pixmap: tmp };
        draw_icon_paths(
            &mut sp,
            icon,
            Rect::new(pad as f32, pad as f32, s, s),
            color,
        );
        tmp = sp.pixmap;
        tmp
    });
    let x = r.x + (r.w - s) * 0.5 - 2.0;
    let y = r.y + (r.h - s) * 0.5 - 2.0;
    blend_pixmap_at(&mut p.pixmap, x.round() as i32, y.round() as i32, bmp);
    if guard.len() > 512 {
        guard.clear();
    }
}

fn draw_icon_paths(p: &mut Painter, icon: Icon, r: Rect, color: Color) {
    let s = r.w.min(r.h);
    let ox = r.x + (r.w - s) * 0.5;
    let oy = r.y + (r.h - s) * 0.5;
    let x = |v: f32| ox + v * s;
    let y = |v: f32| oy + v * s;
    let line = |p: &mut Painter, x1: f32, y1: f32, x2: f32, y2: f32| {
        p.line(x(x1), y(y1), x(x2), y(y2), color, 1.35);
    };
    let rect = |p: &mut Painter, x1: f32, y1: f32, x2: f32, y2: f32| {
        p.stroke_rect(Rect::new(x(x1), y(y1), (x2 - x1) * s, (y2 - y1) * s), color, 1.25);
    };
    let circle = |p: &mut Painter, cx: f32, cy: f32, rr: f32| {
        p.stroke_circle(x(cx), y(cy), rr * s, color, 1.25);
    };
    match icon {
        Icon::New => {
            rect(p, 0.22, 0.08, 0.78, 0.92);
            line(p, 0.56, 0.08, 0.78, 0.30);
            line(p, 0.56, 0.08, 0.56, 0.30);
            line(p, 0.56, 0.30, 0.78, 0.30);
        }
        Icon::File => {
            rect(p, 0.22, 0.08, 0.78, 0.92);
            line(p, 0.56, 0.08, 0.78, 0.30);
            line(p, 0.34, 0.48, 0.66, 0.48);
            line(p, 0.34, 0.64, 0.66, 0.64);
        }
        Icon::Open | Icon::Folder => {
            let pts = [
                (x(0.10), y(0.72)),
                (x(0.10), y(0.24)),
                (x(0.42), y(0.24)),
                (x(0.50), y(0.36)),
                (x(0.88), y(0.36)),
                (x(0.88), y(0.72)),
            ];
            p.stroke_polyline(&pts, color, 1.3, false);
            line(p, 0.10, 0.72, 0.88, 0.72);
        }
        Icon::Save => {
            p.stroke_rect(Rect::new(x(0.14), y(0.14), s * 0.72, s * 0.72), color, 1.2);
            p.fill_rect(Rect::new(x(0.30), y(0.14), s * 0.40, s * 0.24), color);
            p.stroke_rect(Rect::new(x(0.26), y(0.54), s * 0.48, s * 0.32), color, 1.2);
            p.fill_rect(Rect::new(x(0.30), y(0.58), s * 0.26, s * 0.04), color);
        }
        Icon::Print => {
            rect(p, 0.16, 0.40, 0.84, 0.72);
            p.fill_rect(Rect::new(x(0.28), y(0.52), s * 0.44, s * 0.12), C_WHITE);
            p.fill_rect(Rect::new(x(0.30), y(0.10), s * 0.40, s * 0.26), color);
            p.fill_rect(Rect::new(x(0.30), y(0.72), s * 0.40, s * 0.18), color);
        }
        Icon::Preview => {
            rect(p, 0.10, 0.18, 0.90, 0.82);
            rect(p, 0.28, 0.14, 0.72, 0.46);
            circle(p, 0.50, 0.54, 0.12);
            line(p, 0.40, 0.76, 0.48, 0.54);
            line(p, 0.60, 0.76, 0.52, 0.54);
        }
        Icon::Info => {
            circle(p, 0.50, 0.50, 0.42);
            p.fill_circle(x(0.50), y(0.28), s * 0.055, color);
            line(p, 0.50, 0.42, 0.50, 0.72);
        }
        Icon::Calc => {
            rect(p, 0.16, 0.08, 0.84, 0.92);
            p.fill_rect(Rect::new(x(0.26), y(0.18), s * 0.36, s * 0.14), color);
            for row in 0..3 {
                for col in 0..3 {
                    p.fill_rect(
                        Rect::new(
                            x(0.26 + col as f32 * 0.20),
                            y(0.44 + row as f32 * 0.15),
                            s * 0.10,
                            s * 0.08,
                        ),
                        color,
                    );
                }
            }
        }
        Icon::Undo | Icon::Redo => {
            let flip = if icon == Icon::Redo { -1.0 } else { 1.0 };
            let cx = 0.5;
            let pts: Vec<(f32, f32)> = (0..=12)
                .map(|i| {
                    let a = -0.4 + i as f32 / 12.0 * 3.4;
                    (x(cx + flip * (0.32 * a.cos() - 0.05)), y(0.56 + 0.26 * a.sin()))
                })
                .collect();
            p.stroke_polyline(&pts, color, 1.35, false);
            let tipx = cx + flip * (0.32 * (-0.4f32).cos() - 0.05);
            let tipy = 0.56 + 0.26 * (-0.4f32).sin();
            let dir = if icon == Icon::Redo { 1.0 } else { -1.0 };
            p.fill_poly(
                &[
                    (x(tipx), y(tipy)),
                    (x(tipx - dir * 0.18), y(tipy - 0.07)),
                    (x(tipx - dir * 0.10), y(tipy + 0.14)),
                ],
                color,
            );
        }
        Icon::Redraw => {
            circle(p, 0.5, 0.5, 0.30);
            let pts = [
                (x(0.50), y(0.12)),
                (x(0.50), y(0.24)),
                (x(0.62), y(0.18)),
            ];
            p.fill_poly(&pts, color);
        }
        Icon::Pan => {
            circle(p, 0.50, 0.50, 0.10);
            line(p, 0.50, 0.22, 0.50, 0.10);
            line(p, 0.50, 0.78, 0.50, 0.90);
            line(p, 0.22, 0.50, 0.10, 0.50);
            line(p, 0.78, 0.50, 0.90, 0.50);
            line(p, 0.50, 0.12, 0.44, 0.20);
            line(p, 0.50, 0.12, 0.56, 0.20);
            line(p, 0.50, 0.88, 0.44, 0.80);
            line(p, 0.50, 0.88, 0.56, 0.80);
            line(p, 0.12, 0.50, 0.20, 0.44);
            line(p, 0.12, 0.50, 0.20, 0.56);
            line(p, 0.88, 0.50, 0.80, 0.44);
            line(p, 0.88, 0.50, 0.80, 0.56);
        }
        Icon::Zoom | Icon::ZoomWindow => {
            circle(p, 0.42, 0.42, 0.26);
            line(p, 0.62, 0.62, 0.86, 0.86);
            if icon == Icon::ZoomWindow {
                rect(p, 0.28, 0.28, 0.56, 0.56);
            } else {
                line(p, 0.42, 0.30, 0.42, 0.54);
                line(p, 0.30, 0.42, 0.54, 0.42);
            }
        }
        Icon::Fit => {
            rect(p, 0.12, 0.12, 0.88, 0.88);
            line(p, 0.30, 0.30, 0.30, 0.42);
            line(p, 0.30, 0.30, 0.42, 0.30);
            line(p, 0.70, 0.30, 0.70, 0.42);
            line(p, 0.70, 0.30, 0.58, 0.30);
            line(p, 0.30, 0.70, 0.30, 0.58);
            line(p, 0.30, 0.70, 0.42, 0.70);
            line(p, 0.70, 0.70, 0.70, 0.58);
            line(p, 0.70, 0.70, 0.58, 0.70);
        }
        Icon::Point => {
            line(p, 0.50, 0.28, 0.50, 0.72);
            line(p, 0.28, 0.50, 0.72, 0.50);
            p.fill_circle(x(0.50), y(0.50), s * 0.07, color);
        }
        Icon::Line => {
            line(p, 0.18, 0.80, 0.82, 0.20);
            p.stroke_rect(Rect::new(x(0.10), y(0.72), s * 0.16, s * 0.16), color, 1.2);
            p.stroke_rect(Rect::new(x(0.74), y(0.12), s * 0.16, s * 0.16), color, 1.2);
        }
        Icon::Polyline => {
            p.stroke_polyline(
                &[(x(0.14), y(0.74)), (x(0.36), y(0.28)), (x(0.58), y(0.64)), (x(0.86), y(0.22))],
                color,
                1.35,
                false,
            );
            for (px, py) in [(0.14, 0.74), (0.36, 0.28), (0.58, 0.64), (0.86, 0.22)] {
                p.fill_circle(x(px), y(py), s * 0.045, color);
            }
        }
        Icon::Rect => {
            rect(p, 0.14, 0.24, 0.86, 0.76);
            p.fill_circle(x(0.14), y(0.24), s * 0.04, color);
            p.fill_circle(x(0.86), y(0.76), s * 0.04, color);
        }
        Icon::Circle => {
            circle(p, 0.50, 0.50, 0.34);
            p.fill_circle(x(0.50), y(0.50), s * 0.035, color);
            line(p, 0.50, 0.50, 0.74, 0.50);
        }
        Icon::Arc => {
            p.stroke_arc(x(0.50), y(0.62), s * 0.35, -3.6, 2.9, color, 1.35);
            p.fill_circle(x(0.50), y(0.62), s * 0.035, color);
        }
        Icon::Delete => {
            line(p, 0.24, 0.22, 0.78, 0.78);
            line(p, 0.78, 0.22, 0.24, 0.78);
            p.stroke_circle(x(0.50), y(0.50), s * 0.42, color, 1.2);
        }
        Icon::Copy => {
            rect(p, 0.14, 0.22, 0.64, 0.86);
            rect(p, 0.34, 0.10, 0.84, 0.68);
        }
        Icon::Mirror => {
            p.dashed_line(x(0.50), y(0.08), x(0.50), y(0.92), color.with_alpha(170), 1.0, 3.0);
            p.fill_poly(
                &[(x(0.10), y(0.80)), (x(0.40), y(0.80)), (x(0.40), y(0.18))],
                color.with_alpha(220),
            );
            p.fill_poly(
                &[(x(0.90), y(0.80)), (x(0.60), y(0.80)), (x(0.60), y(0.18))],
                color.with_alpha(220),
            );
        }
        Icon::Offset => {
            p.stroke_polyline(
                &[(x(0.14), y(0.84)), (x(0.14), y(0.36)), (x(0.62), y(0.36))],
                color,
                1.35,
                false,
            );
            p.stroke_polyline(
                &[(x(0.34), y(0.86)), (x(0.34), y(0.56)), (x(0.80), y(0.56))],
                color,
                1.2,
                false,
            );
            p.line(x(0.50), y(0.44), x(0.50), y(0.52), color, 1.0);
            p.fill_poly(
                &[(x(0.50), y(0.38)), (x(0.46), y(0.46)), (x(0.54), y(0.46))],
                color,
            );
        }
        Icon::Array => {
            for r in 0..3 {
                for c in 0..3 {
                    p.stroke_rect(
                        Rect::new(x(0.16 + c as f32 * 0.26), y(0.16 + r as f32 * 0.26), s * 0.16, s * 0.16),
                        color,
                        1.0,
                    );
                }
            }
        }
        Icon::Move => {
            p.stroke_polyline(&[(x(0.18), y(0.50)), (x(0.82), y(0.50))], color, 1.3, false);
            p.stroke_polyline(&[(x(0.50), y(0.18)), (x(0.50), y(0.82))], color, 1.3, false);
            line(p, 0.18, 0.50, 0.28, 0.42);
            line(p, 0.18, 0.50, 0.28, 0.58);
            line(p, 0.82, 0.50, 0.72, 0.42);
            line(p, 0.82, 0.50, 0.72, 0.58);
            line(p, 0.50, 0.18, 0.42, 0.28);
            line(p, 0.50, 0.18, 0.58, 0.28);
            line(p, 0.50, 0.82, 0.42, 0.72);
            line(p, 0.50, 0.82, 0.58, 0.72);
        }
        Icon::Rotate => {
            p.stroke_arc(x(0.50), y(0.52), s * 0.30, -0.2, 4.8, color, 1.35);
            p.fill_poly(&[(x(0.78), y(0.26)), (x(0.86), y(0.48)), (x(0.64), y(0.42))], color);
        }
        Icon::Scale => {
            rect(p, 0.18, 0.56, 0.44, 0.84);
            rect(p, 0.34, 0.30, 0.84, 0.68);
            line(p, 0.18, 0.84, 0.34, 0.68);
            line(p, 0.70, 0.48, 0.84, 0.30);
        }
        Icon::Trim => {
            p.stroke_polyline(&[(x(0.16), y(0.30)), (x(0.84), y(0.70))], color, 1.25, false);
            p.stroke_polyline(&[(x(0.16), y(0.70)), (x(0.84), y(0.30))], color, 1.25, false);
            p.stroke_circle(x(0.50), y(0.50), s * 0.16, color, 1.2);
        }
        Icon::Snap => {
            p.stroke_arc(x(0.5), y(0.52), s * 0.26, 0.35, 2.8, color, 1.6);
            line(p, 0.26, 0.36, 0.26, 0.72);
            line(p, 0.74, 0.36, 0.74, 0.72);
            p.fill_rect(Rect::new(x(0.22), y(0.70), s * 0.08, s * 0.10), color);
            p.fill_rect(Rect::new(x(0.70), y(0.70), s * 0.08, s * 0.10), color);
        }
        Icon::SnapEnd | Icon::SnapMid | Icon::SnapCenter | Icon::SnapNode | Icon::SnapNear => {
            match icon {
                Icon::SnapEnd => {
                    p.stroke_rect(Rect::new(x(0.34), y(0.34), s * 0.32, s * 0.32), color, 1.4);
                    p.line(x(0.10), y(0.50), x(0.90), y(0.50), color.with_alpha(70), 0.9);
                }
                Icon::SnapMid => {
                    p.fill_poly(
                        &[(x(0.50), y(0.28)), (x(0.70), y(0.68)), (x(0.30), y(0.68))],
                        color.with_alpha(230),
                    );
                }
                Icon::SnapCenter => {
                    p.stroke_circle(x(0.50), y(0.50), s * 0.22, color, 1.5);
                    p.fill_circle(x(0.50), y(0.50), s * 0.055, color);
                }
                Icon::SnapNode => {
                    p.line(x(0.24), y(0.24), x(0.76), y(0.76), color, 1.5);
                    p.line(x(0.76), y(0.24), x(0.24), y(0.76), color, 1.5);
                }
                _ => {
                    p.line(x(0.16), y(0.74), x(0.50), y(0.44), color, 1.4);
                    p.line(x(0.50), y(0.44), x(0.84), y(0.74), color, 1.4);
                    p.fill_circle(x(0.50), y(0.44), s * 0.06, color);
                }
            }
        }
        Icon::Gear => {
            p.stroke_circle(x(0.50), y(0.50), s * 0.22, color, 1.8);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let x1 = 0.5 + 0.29 * a.cos();
                let y1 = 0.5 + 0.29 * a.sin();
                let x2 = 0.5 + 0.42 * a.cos();
                let y2 = 0.5 + 0.42 * a.sin();
                p.line(x(x1), y(y1), x(x2), y(y2), color, 2.2);
            }
            p.stroke_circle(x(0.50), y(0.50), s * 0.08, color, 1.2);
        }
        Icon::Code => {
            rect(p, 0.14, 0.10, 0.86, 0.90);
            line(p, 0.24, 0.28, 0.76, 0.28);
            p.text(x(0.26), y(0.72), "3B", s * 0.34, color);
        }
        Icon::Transfer => {
            rect(p, 0.10, 0.34, 0.46, 0.70);
            rect(p, 0.54, 0.34, 0.90, 0.70);
            line(p, 0.40, 0.52, 0.60, 0.52);
            line(p, 0.60, 0.52, 0.54, 0.46);
            line(p, 0.60, 0.52, 0.54, 0.58);
        }
        Icon::Check => {
            p.stroke_polyline(&[(x(0.18), y(0.52)), (x(0.42), y(0.76)), (x(0.84), y(0.24))], color, 1.8, false);
        }
        Icon::Help => {
            p.text(x(0.34), y(0.70), "?", s * 0.62, color);
        }
        Icon::Close => {
            line(p, 0.24, 0.24, 0.76, 0.76);
            line(p, 0.76, 0.24, 0.24, 0.76);
        }
        Icon::Arrow => {
            line(p, 0.18, 0.50, 0.78, 0.50);
            line(p, 0.78, 0.50, 0.62, 0.36);
            line(p, 0.78, 0.50, 0.62, 0.64);
        }
        Icon::Select => {
            p.stroke_polyline(
                &[(x(0.30), y(0.16)), (x(0.30), y(0.76)), (x(0.46), y(0.62)), (x(0.58), y(0.90)), (x(0.70), y(0.84)), (x(0.58), y(0.56)), (x(0.78), y(0.52))],
                color,
                1.25,
                true,
            );
        }
    }
}

