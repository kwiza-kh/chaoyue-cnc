//! The application: document state, tools, menus and all dialog windows.

use std::path::PathBuf;

use crate::codegen::{
    build_job, generate, validate, CodeKind, CodeOptions, CodeOutput, CutJob,
};
use crate::foam::{self, FoamOptions};
use crate::gfx::*;
use crate::model::{BBox, Doc, Kind, Pt};
use std::f32::consts::TAU;
use std::hash::{Hash, Hasher};
use crate::platform;
use tiny_skia::Pixmap;

pub const MENU_H: f32 = 26.0;
pub const TOOLBAR_H: f32 = 40.0;
pub const SIDE_W: f32 = 38.0;
pub const BOTTOM_H: f32 = 52.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Select,
    Move,
    Point,
    Line,
    Polyline,
    Rect,
    Circle,
    Arc,
    Copy,
    Mirror,
    Rotate,
    Trim,
    Measure,
    Pan,
    ZoomWindow,
}

impl Tool {
    fn label(self) -> &'static str {
        match self {
            Tool::Select => "选择",
            Tool::Move => "移动",
            Tool::Point => "点",
            Tool::Line => "直线",
            Tool::Polyline => "多段线",
            Tool::Rect => "矩形",
            Tool::Circle => "圆",
            Tool::Arc => "圆弧",
            Tool::Copy => "复制",
            Tool::Mirror => "镜像",
            Tool::Rotate => "旋转",
            Tool::Trim => "修剪",
            Tool::Measure => "测量",
            Tool::Pan => "平移",
            Tool::ZoomWindow => "窗口缩放",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Act {
    None,
    New,
    Open,
    Save,
    SaveAs,
    Import3B,
    ImportDxf,
    FileInfo,
    Preview,
    Exit,
    Undo,
    Redo,
    SelectAll,
    ClearSel,
    DeleteSel,
    EditOffset,
    EditArray,
    EditScale,
    SetTool(Tool),
    Redraw,
    FitAll,
    Pan,
    ZoomIn,
    ZoomOut,
    QueryDist,
    QueryArea,
    QueryLength,
    ToggleCenter,
    ToggleNode,
    ToggleCut,
    Settings,
    CutProgram,
    FoamProgram,
    CalcArea,
    CodeCheck,
    CodeShow,
    CodeSave,
    CodeTransfer,
    DeleteCut,
    Tips,
    About,
    OpenMenu(usize),
    CloseMenu,
    ToggleOrtho,
    TogglePolar,
    ToggleOsnap,
    ToggleGrid,
    ToggleSnap(u8),
    SnapAll,
    SnapNone,
    DlgButton(u8),
    DlgClose,
    DlgTab(u8),
    DlgToggle(u8),
    DlgCycle(u8),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Check {
    None,
    Center,
    Node,
    Cut,
    Ortho,
    Polar,
    Osnap,
    Grid,
}

#[derive(Clone)]
pub enum MenuItem {
    Action {
        label: &'static str,
        act: Act,
        shortcut: &'static str,
        check: Check,
    },
    Sep,
}

impl MenuItem {
    fn a(label: &'static str, act: Act, shortcut: &'static str) -> Self {
        MenuItem::Action { label, act, shortcut, check: Check::None }
    }
    fn c(label: &'static str, act: Act, shortcut: &'static str, check: Check) -> Self {
        MenuItem::Action { label, act, shortcut, check }
    }
}

#[derive(Clone)]
pub struct MenuDef {
    pub label: &'static str,
    pub items: Vec<MenuItem>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CursorKind {
    Arrow,
    Cross,
    Hand,
    IBeam,
    SizeAll,
}

#[derive(Clone)]
struct Hit {
    r: Rect,
    act: Act,
    tip: String,
}

#[derive(Clone)]
struct FieldHit {
    r: Rect,
    id: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConfirmAct {
    New,
    Open,
    Exit,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PromptAct {
    Offset,
    Scale,
}

#[derive(Clone)]
pub struct CutState {
    kind: usize,
    side: usize,
    px: String,
    py: String,
    comp: String,
    name: String,
    optimize: bool,
    error: Option<String>,
}

#[derive(Clone)]
pub struct FoamState {
    mode: usize,
    side: usize,
    sx: String,
    sy: String,
    kerf: String,
    feed: String,
    rapid: String,
    lead: String,
    tx: String,
    ty: String,
    optimize: bool,
    continuous: bool,
    name: String,
    error: Option<String>,
}

#[derive(Clone)]
pub struct CodeState {
    text: String,
    title: String,
    kind: CodeKind,
    scroll: f32,
    font: f32,
    summary: String,
}

#[derive(Clone)]
pub struct CalcState {
    thickness: String,
    length: f32,
    area: f32,
    hours: f32,
    money: f32,
}

#[derive(Clone)]
pub struct SettingsState {
    tab: usize,
    work_area: String,
    work_money: String,
    moly: String,
    unit: String,
    center: bool,
    node: bool,
    cut: bool,
    line_no: bool,
    bg: u8,
    code_kind: usize,
}

#[derive(Clone)]
pub struct FileInfoState {
    part: String,
    unit: String,
    programmer: String,
    note: String,
}

#[derive(Clone)]
pub struct ArrayState {
    rows: String,
    cols: String,
    dx: String,
    dy: String,
}

#[derive(Clone)]
pub struct PromptState {
    title: String,
    label: String,
    hint: String,
    value: String,
    action: PromptAct,
}

#[derive(Clone)]
pub struct TransferState {
    port: usize,
    baud: usize,
    progress: f32,
    message: String,
}

#[derive(Clone)]
pub enum Dialog {
    Message { title: String, text: String, error: bool },
    Confirm { title: String, text: String, action: ConfirmAct },
    Cut(CutState),
    Foam(FoamState),
    Code(CodeState),
    Calc(CalcState),
    Settings(SettingsState),
    FileInfo(FileInfoState),
    Array(ArrayState),
    Prompt(PromptState),
    Transfer(TransferState),
    Tips(usize),
    About,
    Preview,
}

#[derive(Clone)]
enum Drag {
    Pan { last: (f32, f32) },
    Move {
        start: Pt,
        applied: Pt,
        snapshot: Box<Doc>,
        ids: Vec<u64>,
        moved: bool,
    },
    BoxSel { start: (f32, f32), current: (f32, f32) },
    ZoomBox { start: (f32, f32), current: (f32, f32) },
}

struct CanvasCache {
    w: u32,
    h: u32,
    scale_bits: u32,
    cx_bits: u32,
    cy_bits: u32,
    bg: u8,
    grid: bool,
    pixmap: Pixmap,
}

struct StaticCache {
    w: u32,
    h: u32,
    scale_bits: u32,
    cx_bits: u32,
    cy_bits: u32,
    signature: u64,
    pixmap: Pixmap,
}

#[derive(Clone)]
pub struct Settings {
    pub work_area: f32,
    pub work_money: f32,
    pub moly: f32,
    pub unit: String,
    pub show_center: bool,
    pub show_node: bool,
    pub show_cut: bool,
    pub code_kind: CodeKind,
    pub line_numbers: bool,
    pub bg: u8,
    pub foam_four_axis: bool,
    pub foam_side: usize,
    pub foam_kerf: f32,
    pub foam_feed: f32,
    pub foam_rapid: f32,
    pub foam_lead: f32,
    pub foam_taper_x: f32,
    pub foam_taper_y: f32,
    pub foam_optimize: bool,
    pub foam_continuous: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            work_area: 1500.0,
            work_money: 12.0,
            moly: 0.18,
            unit: "mm".to_string(),
            show_center: true,
            show_node: false,
            show_cut: true,
            code_kind: CodeKind::ThreeB,
            line_numbers: true,
            bg: 0,
            foam_four_axis: false,
            foam_side: 0,
            foam_kerf: 0.4,
            foam_feed: 900.0,
            foam_rapid: 4000.0,
            foam_lead: 3.0,
            foam_taper_x: 0.0,
            foam_taper_y: 0.0,
            foam_optimize: true,
            foam_continuous: false,
        }
    }
}

impl Settings {
    fn load() -> Self {
        let mut s = Settings::default();
        let path = settings_path();
        if let Ok(text) = std::fs::read_to_string(&path) {
            for line in text.lines() {
                let Some((k, v)) = line.split_once('=') else { continue };
                let v = v.trim();
                match k.trim() {
                    "work_area" => s.work_area = v.parse().unwrap_or(s.work_area),
                    "work_money" => s.work_money = v.parse().unwrap_or(s.work_money),
                    "moly" => s.moly = v.parse().unwrap_or(s.moly),
                    "unit" => s.unit = v.to_string(),
                    "show_center" => s.show_center = v == "1",
                    "show_node" => s.show_node = v == "1",
                    "show_cut" => s.show_cut = v == "1",
                    "code_kind" => {
                        s.code_kind = CodeKind::from_index(v.parse().unwrap_or(0))
                    }
                    "line_numbers" => s.line_numbers = v == "1",
                    "bg" => s.bg = v.parse().unwrap_or(0),
                    "foam_four_axis" => s.foam_four_axis = v == "1",
                    "foam_side" => s.foam_side = v.parse().unwrap_or(0).min(2),
                    "foam_kerf" => s.foam_kerf = v.parse().unwrap_or(s.foam_kerf),
                    "foam_feed" => s.foam_feed = v.parse().unwrap_or(s.foam_feed),
                    "foam_rapid" => s.foam_rapid = v.parse().unwrap_or(s.foam_rapid),
                    "foam_lead" => s.foam_lead = v.parse().unwrap_or(s.foam_lead),
                    "foam_taper_x" => s.foam_taper_x = v.parse().unwrap_or(s.foam_taper_x),
                    "foam_taper_y" => s.foam_taper_y = v.parse().unwrap_or(s.foam_taper_y),
                    "foam_optimize" => s.foam_optimize = v == "1",
                    "foam_continuous" => s.foam_continuous = v == "1",
                    _ => {}
                }
            }
        }
        s
    }
    fn save(&self) {
        let path = settings_path();
        let text = format!(
            "work_area={}\nwork_money={}\nmoly={}\nunit={}\nshow_center={}\nshow_node={}\nshow_cut={}\ncode_kind={}\nline_numbers={}\nbg={}\nfoam_four_axis={}\nfoam_side={}\nfoam_kerf={}\nfoam_feed={}\nfoam_rapid={}\nfoam_lead={}\nfoam_taper_x={}\nfoam_taper_y={}\nfoam_optimize={}\nfoam_continuous={}\n",
            self.work_area,
            self.work_money,
            self.moly,
            self.unit,
            self.show_center as u8,
            self.show_node as u8,
            self.show_cut as u8,
            self.code_kind.index(),
            self.line_numbers as u8,
            self.bg,
            self.foam_four_axis as u8,
            self.foam_side,
            self.foam_kerf,
            self.foam_feed,
            self.foam_rapid,
            self.foam_lead,
            self.foam_taper_x,
            self.foam_taper_y,
            self.foam_optimize as u8,
            self.foam_continuous as u8
        );
        let _ = std::fs::write(path, text);
    }
}

fn settings_path() -> PathBuf {
    let mut p = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    p.pop();
    p.push("cnc_settings.ini");
    p
}

pub struct App {
    pub hwnd: isize,
    pub w: u32,
    pub h: u32,
    pub painter: Painter,
    pub doc: Doc,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub settings: Settings,
    pub undo: Vec<Doc>,
    pub redo: Vec<Doc>,
    pub view_center: Pt,
    pub scale: f32,
    pub fitted: bool,
    pub mouse: (f32, f32),
    pub mouse_in_canvas: bool,
    pub tool: Tool,
    pub pts: Vec<Pt>,
    pub snap: Option<(Pt, bool)>,
    pub selected: Vec<u64>,
    pub hover: Option<u64>,
    pub drag: Option<Drag>,
    pub left_down: bool,
    pub menus: Vec<MenuDef>,
    pub menu_open: Option<usize>,
    pub hits: Vec<Hit>,
    pub field_hits: Vec<FieldHit>,
    pub dialog_hits: Vec<(Rect, Act)>,
    pub menu_tabs: Vec<(Rect, usize)>,
    pub dialog: Option<Dialog>,
    pub focus: Option<String>,
    field_selected: bool,
    pub command: String,
    pub status: String,
    pub cut_job: Option<CutJob>,
    pub last_code: Option<CodeOutput>,
    pub canvas: Rect,
    pub input_rect: Rect,
    pub code_area: Option<Rect>,
    pub ortho: bool,
    pub polar: bool,
    pub osnap: bool,
    pub grid: bool,
    pub snap_modes: u8,
    pub tip_index: usize,
    screenshot_done: bool,
    pub screenshot_mode: bool,
    title_cache: String,
    pub cursor: CursorKind,
    background_cache: Option<CanvasCache>,
    static_cache: Option<StaticCache>,
    pub profile: bool,
    profile_frames: u32,
}

const TIPS: [&str; 7] = [
    "先画出封闭轮廓，再选择图元，就可以在“编程”菜单里生成切割路径。",
    "滚轮可以缩放图板，按住鼠标中键拖动可以平移视图。",
    "按 F8 切换正交，按 F9 切换对象捕捉，按 F7 切换网格。",
    "在下方命令栏输入坐标（例如 20,15）同样可以确定点位。",
    "切割编程支持 3B、ISO G 代码和 Mach3 G 代码，生成后可以先做代码校验再存盘。",
    "泡沫切割在“编程 → 泡沫切割 (Mach3)”中，F5 可直接打开；两轴或四轴 XYAB 均可输出。",
    "设置中的工时计算指数与单价，用来估算加工面积、工时和金额。",
];

impl App {
    pub fn new(hwnd: isize) -> Self {
        let settings = Settings::load();
        let (doc, outer) = Doc::demo();
        let pierce = Pt::new(-90.0, -77.5);
        let cut_job = build_job(&doc, &outer, pierce, 0.09, -1).ok();
        let menus = build_menus();
        Self {
            hwnd,
            w: 1360,
            h: 860,
            painter: Painter::new(1360, 860),
            doc,
            path: None,
            dirty: false,
            settings,
            undo: Vec::new(),
            redo: Vec::new(),
            view_center: Pt::new(0.0, 0.0),
            scale: 4.4,
            fitted: false,
            mouse: (-100.0, -100.0),
            mouse_in_canvas: false,
            tool: Tool::Select,
            pts: Vec::new(),
            snap: None,
            selected: Vec::new(),
            hover: None,
            drag: None,
            left_down: false,
            menus,
            menu_open: None,
            hits: Vec::new(),
            field_hits: Vec::new(),
            dialog_hits: Vec::new(),
            menu_tabs: Vec::new(),
            dialog: None,
            focus: None,
            field_selected: false,
            command: String::new(),
            status: "就绪：从工具栏选择绘图命令，或直接打开已有图形。".to_string(),
            cut_job,
            last_code: None,
            canvas: Rect::new(38.0, 66.0, 1284.0, 742.0),
            input_rect: Rect::new(38.0, 808.0, 1284.0, 26.0),
            code_area: None,
            ortho: false,
            polar: false,
            osnap: true,
            grid: true,
            snap_modes: 1 | 2 | 4,
            tip_index: 0,
            screenshot_done: false,
            screenshot_mode: false,
            title_cache: String::new(),
            cursor: CursorKind::Arrow,
            background_cache: None,
            static_cache: None,
            profile: false,
            profile_frames: 0,
        }
    }

    pub fn window_title(&self) -> String {
        let base = if self.doc.title.is_empty() {
            "未命名".to_string()
        } else {
            self.doc.title.clone()
        };
        format!(
            "{} - 超越数控线切割编程系统{}",
            base,
            if self.dirty { " *" } else { "" }
        )
    }

    pub fn screenshot_pointer(&mut self) {
        if self.screenshot_done {
            return;
        }
        self.screenshot_done = true;
        let p = Pt::new(72.0, 38.0);
        let s = self.w2s(p);
        self.mouse = s;
        self.mouse_in_canvas = self.canvas.contains(s.0, s.1);
        self.snap = Some((p, true));
    }

    // ---------- geometry helpers ----------

    fn w2s(&self, p: Pt) -> (f32, f32) {
        (
            self.canvas.x + self.canvas.w * 0.5 + (p.x - self.view_center.x) * self.scale,
            self.canvas.y + self.canvas.h * 0.5 - (p.y - self.view_center.y) * self.scale,
        )
    }

    fn screen_to_world(&self, x: f32, y: f32) -> Pt {
        Pt::new(
            self.view_center.x + (x - (self.canvas.x + self.canvas.w * 0.5)) / self.scale,
            self.view_center.y - (y - (self.canvas.y + self.canvas.h * 0.5)) / self.scale,
        )
    }

    fn fit_view(&mut self) {
        let bb = self.doc.bounds().unwrap_or(BBox {
            min: Pt::new(-100.0, -70.0),
            max: Pt::new(100.0, 70.0),
        });
        let bw = bb.width().max(10.0);
        let bh = bb.height().max(10.0);
        let sx = (self.canvas.w - 120.0) / bw;
        let sy = (self.canvas.h - 120.0) / bh;
        self.scale = sx.min(sy).clamp(0.05, 80.0);
        self.view_center = bb.center();
    }

    fn zoom_at(&mut self, factor: f32, sx: f32, sy: f32) {
        let before = self.screen_to_world(sx, sy);
        self.scale = (self.scale * factor).clamp(0.02, 300.0);
        let cx = self.canvas.x + self.canvas.w * 0.5;
        let cy = self.canvas.y + self.canvas.h * 0.5;
        self.view_center = Pt::new(
            before.x - (sx - cx) / self.scale,
            before.y + (sy - cy) / self.scale,
        );
    }

    fn grid_step(&self) -> f32 {
        let mut base = 1.0f32;
        loop {
            for m in [1.0, 2.0, 5.0] {
                let s = base * m;
                if s * self.scale >= 18.0 {
                    return s;
                }
            }
            base *= 10.0;
            if base > 1e6 {
                return 1e6;
            }
        }
    }

    fn hit_entity(&self, p: Pt, tol_screen: f32) -> Option<u64> {
        let tol = tol_screen / self.scale;
        let mut best: Option<(f32, u64)> = None;
        for e in &self.doc.entities {
            // A cheap bounds test makes hit-testing independent of drawing size.
            if !e.bbox().expand(tol).contains(p) {
                continue;
            }
            let d = e.hit(p);
            if d <= tol && best.as_ref().map(|b| d < b.0).unwrap_or(true) {
                best = Some((d, e.id));
            }
        }
        best.map(|b| b.1)
    }

    fn snap_mode_point(&self, e: &crate::model::Entity) -> Vec<(Pt, u8)> {
        match &e.kind {
            Kind::Point { p } => vec![(*p, 1)],
            Kind::Line { a, b } => vec![(*a, 1), (*b, 1), (a.lerp(*b, 0.5), 2)],
            Kind::Circle { c, .. } => vec![(*c, 4)],
            Kind::Arc { c, r, a0, sweep } => vec![
                (c.add(Pt::from_angle(*a0, *r)), 1),
                (c.add(Pt::from_angle(*a0 + *sweep, *r)), 1),
                (*c, 4),
            ],
        }
    }

    fn snap_world(&self, sx: f32, sy: f32) -> (Pt, bool) {
        let raw = self.screen_to_world(sx, sy);
        if self.osnap {
            let mut best: Option<(f32, Pt)> = None;
            for e in &self.doc.entities {
                for (p, mode) in self.snap_mode_point(e) {
                    if self.snap_modes & mode == 0 {
                        continue;
                    }
                    let s = self.w2s(p);
                    let d = ((s.0 - sx).powi(2) + (s.1 - sy).powi(2)).sqrt();
                    if d < 10.0 && best.as_ref().map(|b| d < b.0).unwrap_or(true) {
                        best = Some((d, p));
                    }
                }
            }
            if self.snap_modes & 8 != 0 {
                for p in self.doc.intersections() {
                    let s = self.w2s(p);
                    let d = ((s.0 - sx).powi(2) + (s.1 - sy).powi(2)).sqrt();
                    if d < 10.0 && best.as_ref().map(|b| d < b.0).unwrap_or(true) {
                        best = Some((d, p));
                    }
                }
            }
            if self.snap_modes & 16 != 0 {
                // Exact nearest point on each entity instead of sampling a
                // circle around the cursor; this is both faster and accurate
                // when zoomed in.
                for e in &self.doc.entities {
                    let (q, _) = e.nearest_point(raw);
                    let s = self.w2s(q);
                    let d = ((s.0 - sx).powi(2) + (s.1 - sy).powi(2)).sqrt();
                    if d < 10.0 && best.as_ref().map(|b| d < b.0).unwrap_or(true) {
                        best = Some((d, q));
                    }
                }
            }
            if let Some((_, p)) = best {
                return (p, true);
            }
        }
        // Orthographic / polar constraints for line-like rubber bands.
        let constrain = matches!(
            self.tool,
            Tool::Line | Tool::Polyline | Tool::Rect | Tool::Measure | Tool::Copy | Tool::Move
        );
        if constrain {
            if let Some(last) = self.pts.last().copied() {
                let mut p = raw;
                if self.ortho {
                    if (p.x - last.x).abs() >= (p.y - last.y).abs() {
                        p.y = last.y;
                    } else {
                        p.x = last.x;
                    }
                } else if self.polar {
                    let d = p.sub(last);
                    let a = (d.angle() / (std::f32::consts::FRAC_PI_4)).round()
                        * std::f32::consts::FRAC_PI_4;
                    let r = d.len();
                    p = last.add(Pt::from_angle(a, r));
                }
                if self.ortho || self.polar {
                    return (p, true);
                }
            }
        }
        (raw, false)
    }

    fn current_point(&self) -> Pt {
        if self.mouse_in_canvas {
            self.snap_world(self.mouse.0, self.mouse.1).0
        } else {
            Pt::new(0.0, 0.0)
        }
    }

    fn prompt(&self) -> String {
        match self.tool {
            Tool::Select | Tool::Move => {
                if !self.selected.is_empty() {
                    format!("已选择 {} 个图元；拖动可移动，Delete 删除", self.selected.len())
                } else {
                    "选择图元，或从“绘图”菜单开始".to_string()
                }
            }
            Tool::Point => "指定点".to_string(),
            Tool::Line => {
                if self.pts.is_empty() {
                    "指定第一点".to_string()
                } else {
                    "指定第二点".to_string()
                }
            }
            Tool::Polyline => "指定下一点，右键或回车结束".to_string(),
            Tool::Rect => {
                if self.pts.is_empty() {
                    "指定第一个角点".to_string()
                } else {
                    "指定另一个角点".to_string()
                }
            }
            Tool::Circle => {
                if self.pts.is_empty() {
                    "指定圆心".to_string()
                } else {
                    "指定圆上一点".to_string()
                }
            }
            Tool::Arc => match self.pts.len() {
                0 => "指定圆弧圆心".to_string(),
                1 => "指定圆弧起点".to_string(),
                _ => "指定圆弧终点".to_string(),
            },
            Tool::Copy => {
                if self.pts.is_empty() {
                    "指定复制基点".to_string()
                } else {
                    "指定目标点".to_string()
                }
            }
            Tool::Mirror => {
                if self.pts.is_empty() {
                    "指定镜像线第一点".to_string()
                } else {
                    "指定镜像线第二点".to_string()
                }
            }
            Tool::Rotate => match self.pts.len() {
                0 => "指定旋转中心".to_string(),
                1 => "指定参考点".to_string(),
                _ => "指定旋转后的方向".to_string(),
            },
            Tool::Trim => "选择要修剪的直线".to_string(),
            Tool::Measure => {
                if self.pts.is_empty() {
                    "测量：指定第一点".to_string()
                } else {
                    "测量：指定第二点".to_string()
                }
            }
            Tool::Pan => "按住左键拖动画布".to_string(),
            Tool::ZoomWindow => "拖出要放大的窗口".to_string(),
        }
    }

    fn reset_tool_state(&mut self) {
        self.pts.clear();
        self.drag = None;
    }

    fn set_tool(&mut self, t: Tool) {
        self.reset_tool_state();
        self.tool = t;
        self.menu_open = None;
        self.status = format!("{}：{}", t.label(), self.prompt());
    }

    // ---------- undo ----------

    fn push_undo(&mut self) {
        self.undo.push(self.doc.clone());
        if self.undo.len() > 60 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
    }

    fn undo(&mut self) {
        if let Some(prev) = self.undo.pop() {
            self.redo.push(self.doc.clone());
            self.doc = prev;
            self.selected.clear();
            self.dirty = true;
            self.cut_job = None;
            self.last_code = None;
            self.status = "已撤销上一步操作".to_string();
        }
    }

    fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(self.doc.clone());
            self.doc = next;
            self.selected.clear();
            self.dirty = true;
            self.cut_job = None;
            self.last_code = None;
            self.status = "已重做".to_string();
        }
    }

    // ---------- mouse / keyboard ----------

    pub fn on_mouse_move(&mut self, x: f32, y: f32) {
        if self.screenshot_mode {
            return;
        }
        self.mouse = (x, y);
        self.mouse_in_canvas = self.canvas.contains(x, y) && self.dialog.is_none() && self.menu_open.is_none();
        if self.menu_open.is_some() {
            if let Some((_, idx)) = self
                .menu_tabs
                .iter()
                .find(|(r, _)| r.contains(x, y))
            {
                if self.menu_open != Some(*idx) {
                    self.menu_open = Some(*idx);
                }
            }
        }
        let mut drag = self.drag.take();
        if let Some(d) = &mut drag {
            match d {
                Drag::Pan { last } => {
                    let dx = x - last.0;
                    let dy = y - last.1;
                    self.view_center.x -= dx / self.scale;
                    self.view_center.y += dy / self.scale;
                    *last = (x, y);
                }
                Drag::Move { start, applied, ids, moved, .. } => {
                    if self.left_down {
                        let p = self.screen_to_world(x, y);
                        let total = p.sub(*start);
                        if !*moved && total.len() * self.scale > 2.0 {
                            *moved = true;
                        }
                        if *moved {
                            let delta = total.sub(*applied);
                            if delta.len() > 1e-9 {
                                for e in &mut self.doc.entities {
                                    if ids.contains(&e.id) {
                                        e.translate(delta);
                                    }
                                }
                                *applied = total;
                                self.dirty = true;
                            }
                        }
                    }
                }
                Drag::BoxSel { current, .. } | Drag::ZoomBox { current, .. } => {
                    *current = (x, y);
                }
            }
        }
        self.drag = drag;
        if self.mouse_in_canvas && self.drag.is_none() {
            let (p, snapped) = self.snap_world(x, y);
            self.snap = Some((p, snapped));
            self.hover = self.hit_entity(self.screen_to_world(x, y), 6.0);
        } else {
            self.hover = None;
        }
        // Cursor shape.
        self.cursor = if self.dialog.is_some() {
            if self.focus.is_some() {
                CursorKind::IBeam
            } else {
                CursorKind::Arrow
            }
        } else if let Some(d) = &self.drag {
            match d {
                Drag::Pan { .. } => CursorKind::Hand,
                _ => CursorKind::Cross,
            }
        } else if self.mouse_in_canvas {
            if self.tool == Tool::Pan {
                CursorKind::Hand
            } else {
                CursorKind::Cross
            }
        } else {
            CursorKind::Arrow
        };
    }

    pub fn on_left_down(&mut self, x: f32, y: f32, shift: bool) {
        self.mouse = (x, y);
        self.left_down = true;
        if self.dialog.is_some() {
            if let Some((_, act)) = self
                .dialog_hits
                .iter()
                .rev()
                .find(|(r, _)| r.contains(x, y))
                .cloned()
            {
                self.on_dialog_act(act);
                return;
            }
            if let Some(f) = self.field_hits.iter().rev().find(|f| f.r.contains(x, y)).cloned() {
                self.focus = Some(f.id);
                self.field_selected = true;
                return;
            }
            return;
        }
        if self.menu_open.is_some() {
            if let Some(hit) = self
                .hits
                .iter()
                .rev()
                .find(|h| h.r.contains(x, y))
                .cloned()
            {
                self.menu_open = None;
                self.do_act(hit.act);
            } else {
                self.menu_open = None;
            }
            return;
        }
        if let Some((_, act)) = self
            .hits
            .iter()
            .rev()
            .find(|h| h.r.contains(x, y))
            .map(|h| (h.r, h.act))
        {
            self.do_act(act);
            return;
        }
        if self.canvas.contains(x, y) {
            self.canvas_down(x, y, shift);
        }
    }

    pub fn on_left_up(&mut self, x: f32, y: f32) {
        self.left_down = false;
        self.mouse = (x, y);
        let drag = self.drag.take();
        match drag {
            Some(Drag::Move { snapshot, ids, moved, .. }) => {
                if moved {
                    self.undo.push(*snapshot);
                    if self.undo.len() > 60 {
                        self.undo.remove(0);
                    }
                    self.redo.clear();
                    self.dirty = true;
                    self.status = format!("已移动 {} 个图元", ids.len());
                }
            }
            Some(Drag::BoxSel { start, current }) => {
                let p1 = self.screen_to_world(start.0.min(current.0), start.1.min(current.1));
                let p2 = self.screen_to_world(start.0.max(current.0), start.1.max(current.1));
                let bb = BBox {
                    min: Pt::new(p1.x.min(p2.x), p1.y.min(p2.y)),
                    max: Pt::new(p1.x.max(p2.x), p1.y.max(p2.y)),
                };
                if (current.0 - start.0).abs() > 4.0 || (current.1 - start.1).abs() > 4.0 {
                    let mut ids = Vec::new();
                    for e in &self.doc.entities {
                        if e.bbox().intersects(&bb) {
                            ids.push(e.id);
                        }
                    }
                    self.selected = ids;
                    self.status = format!("已选择 {} 个图元", self.selected.len());
                } else {
                    self.selected.clear();
                    self.status = "已清除选择".to_string();
                }
            }
            Some(Drag::ZoomBox { start, current }) => {
                let x0 = start.0.min(current.0);
                let y0 = start.1.min(current.1);
                let x1 = start.0.max(current.0);
                let y1 = start.1.max(current.1);
                if x1 - x0 > 12.0 && y1 - y0 > 12.0 {
                    let a = self.screen_to_world(x0, y1);
                    let b = self.screen_to_world(x1, y0);
                    let bw = (b.x - a.x).abs().max(0.01);
                    let bh = (b.y - a.y).abs().max(0.01);
                    self.scale = (self.canvas.w / bw).min(self.canvas.h / bh).clamp(0.02, 300.0);
                    self.view_center = Pt::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
                    self.tool = Tool::Select;
                }
            }
            _ => {}
        }
    }

    pub fn on_middle_down(&mut self, x: f32, y: f32) {
        self.mouse = (x, y);
        if self.dialog.is_none() && self.menu_open.is_none() {
            self.drag = Some(Drag::Pan { last: (x, y) });
        }
    }

    pub fn on_middle_up(&mut self) {
        if matches!(self.drag, Some(Drag::Pan { .. })) {
            self.drag = None;
        }
    }

    pub fn on_right_down(&mut self, x: f32, y: f32) {
        if self.dialog.is_some() {
            self.close_dialog();
            return;
        }
        if self.menu_open.is_some() {
            self.menu_open = None;
            return;
        }
        if !self.canvas.contains(x, y) {
            return;
        }
        if self.tool == Tool::Polyline {
            self.finish_polyline(false);
            return;
        }
        if !self.pts.is_empty() {
            self.reset_tool_state();
            self.status = "已取消当前操作".to_string();
        } else if !self.selected.is_empty() {
            self.selected.clear();
            self.status = "已清除选择".to_string();
        } else {
            self.tool = Tool::Select;
        }
    }

    pub fn on_wheel(&mut self, x: f32, y: f32, delta: i32) {
        if let Some(Dialog::Code(st)) = &mut self.dialog {
            if self.code_area.map(|r| r.contains(x, y)).unwrap_or(false) {
                st.scroll -= delta as f32 * 0.5;
                st.scroll = st.scroll.max(0.0);
                return;
            }
        }
        if self.canvas.contains(x, y) && self.dialog.is_none() {
            let steps = delta as f32 / 120.0;
            let factor = 1.12f32.powf(steps);
            self.zoom_at(factor, x, y);
            self.status = format!("缩放 {:.0}%", self.scale / 4.0 * 100.0);
        }
    }

    pub fn on_key(&mut self, vk: u32, ctrl: bool, shift: bool) {
        const VK_BACK: u32 = 8;
        const VK_RETURN: u32 = 13;
        const VK_ESCAPE: u32 = 27;
        const VK_PRIOR: u32 = 33;
        const VK_NEXT: u32 = 34;
        const VK_DELETE: u32 = 46;
        const VK_F5: u32 = 116;
        const VK_F7: u32 = 118;
        const VK_F8: u32 = 119;
        const VK_F9: u32 = 120;
        const VK_F10: u32 = 121;
        if ctrl {
            match vk {
                65 => self.do_act(Act::SelectAll),          // A
                78 => self.do_act(Act::New),                // N
                79 => self.do_act(Act::Open),               // O
                83 => {
                    if shift {
                        self.do_act(Act::SaveAs);
                    } else {
                        self.do_act(Act::Save);
                    }
                }
                89 => self.redo(),                          // Y
                90 => self.undo(),                          // Z
                _ => {}
            }
            return;
        }
        match vk {
            VK_ESCAPE => {
                if self.dialog.is_some() {
                    self.close_dialog();
                } else if self.menu_open.is_some() {
                    self.menu_open = None;
                } else if self.tool != Tool::Select || !self.pts.is_empty() {
                    self.reset_tool_state();
                    self.tool = Tool::Select;
                    self.status = "已取消".to_string();
                } else {
                    self.selected.clear();
                    self.command.clear();
                }
            }
            VK_RETURN => {
                if self.dialog.is_some() {
                    self.on_dialog_confirm();
                } else if self.tool == Tool::Polyline {
                    self.finish_polyline(false);
                } else {
                    self.submit_command();
                }
            }
            VK_BACK => {
                if self.dialog.is_some() {
                    self.backspace_field();
                } else {
                    self.command.pop();
                }
            }
            VK_DELETE => {
                if self.dialog.is_none() {
                    self.delete_selected();
                }
            }
            VK_PRIOR | VK_NEXT => {
                if let Some(Dialog::Code(s)) = &mut self.dialog {
                    let page = 320.0;
                    if vk == VK_PRIOR {
                        s.scroll = (s.scroll - page).max(0.0);
                    } else {
                        s.scroll += page;
                    }
                }
            }
            VK_F5 => {
                if self.dialog.is_none() && self.menu_open.is_none() {
                    self.do_act(Act::FoamProgram);
                }
            }
            VK_F7 => {
                self.grid = !self.grid;
                self.status = format!("网格{}", if self.grid { "开" } else { "关" });
            }
            VK_F8 => {
                self.ortho = !self.ortho;
                self.status = format!("正交{}", if self.ortho { "开" } else { "关" });
            }
            VK_F9 => {
                self.osnap = !self.osnap;
                self.status = format!("对象捕捉{}", if self.osnap { "开" } else { "关" });
            }
            VK_F10 => {
                self.polar = !self.polar;
                self.status = format!("极轴追踪{}", if self.polar { "开" } else { "关" });
            }
            _ => {}
        }
    }

    pub fn on_char(&mut self, c: char) {
        if self.dialog.is_some() {
            if c.is_control() {
                return;
            }
            self.append_field(c);
            return;
        }
        if !c.is_control() {
            self.command.push(c);
        }
    }

    pub fn on_double_click(&mut self) {
        if self.tool == Tool::Polyline {
            self.finish_polyline(false);
        }
    }

    pub fn cursor_kind(&self) -> CursorKind {
        self.cursor
    }

    pub fn menu_tab_center(&self, idx: usize) -> Option<(f32, f32)> {
        self.menu_tabs
            .iter()
            .find(|(_, i)| *i == idx)
            .map(|(r, _)| r.center())
    }

    pub fn hit_center(&self, act: Act) -> Option<(f32, f32)> {
        self.hits
            .iter()
            .rev()
            .find(|h| h.act == act)
            .map(|h| h.r.center())
    }

    pub fn dialog_button_center(&self, id: u8) -> Option<(f32, f32)> {
        self.dialog_hits
            .iter()
            .rev()
            .find(|(_, a)| *a == Act::DlgButton(id))
            .map(|(r, _)| r.center())
    }

    pub fn dialog_hit_center(&self, act: Act) -> Option<(f32, f32)> {
        self.dialog_hits
            .iter()
            .rev()
            .find(|(_, a)| *a == act)
            .map(|(r, _)| r.center())
    }

    pub fn field_center(&self, id: &str) -> Option<(f32, f32)> {
        self.field_hits
            .iter()
            .rev()
            .find(|f| f.id == id)
            .map(|f| f.r.center())
    }

    pub fn w2s_public(&self, p: Pt) -> (f32, f32) {
        self.w2s(p)
    }

    // ---------- tool execution ----------

    fn canvas_down(&mut self, x: f32, y: f32, shift: bool) {
        let raw = self.screen_to_world(x, y);
        match self.tool {
            Tool::Pan => {
                self.drag = Some(Drag::Pan { last: (x, y) });
                return;
            }
            Tool::ZoomWindow => {
                self.drag = Some(Drag::ZoomBox { start: (x, y), current: (x, y) });
                return;
            }
            _ => {}
        }
        if self.tool == Tool::Select || self.tool == Tool::Move {
            if let Some(id) = self.hit_entity(raw, 7.0) {
                if shift {
                    if let Some(pos) = self.selected.iter().position(|v| *v == id) {
                        self.selected.remove(pos);
                    } else {
                        self.selected.push(id);
                    }
                } else if !self.selected.contains(&id) {
                    self.selected = vec![id];
                }
                if self.selected.contains(&id) {
                    self.drag = Some(Drag::Move {
                        start: raw,
                        applied: Pt::new(0.0, 0.0),
                        snapshot: Box::new(self.doc.clone()),
                        ids: self.selected.clone(),
                        moved: false,
                    });
                }
            } else {
                if !shift {
                    self.selected.clear();
                }
                self.drag = Some(Drag::BoxSel { start: (x, y), current: (x, y) });
            }
            return;
        }
        let (p, _) = self.snap_world(x, y);
        match self.tool {
            Tool::Trim => {
                self.trim_click(raw);
            }
            Tool::Measure => {
                if self.pts.is_empty() {
                    self.pts.push(p);
                    self.status = "测量：指定第二点".to_string();
                } else {
                    let a = self.pts[0];
                    let b = p;
                    let d = a.dist(b);
                    self.pts.clear();
                    self.open_message(
                        "距离测量",
                        &format!(
                            "两点距离：{:.3} mm\nΔX：{:.3} mm\nΔY：{:.3} mm\n角度：{:.2}°",
                            d,
                            b.x - a.x,
                            b.y - a.y,
                            (b.sub(a).angle() * 180.0 / std::f32::consts::PI)
                        ),
                        false,
                    );
                }
            }
            Tool::Copy => {
                if self.selected.is_empty() {
                    self.status = "请先选择要复制的图元".to_string();
                    return;
                }
                self.pts.push(p);
                if self.pts.len() == 2 {
                    let d = self.pts[1].sub(self.pts[0]);
                    self.push_undo();
                    let mut new_ids = Vec::new();
                    let src: Vec<crate::model::Entity> = self
                        .doc
                        .entities
                        .iter()
                        .filter(|e| self.selected.contains(&e.id))
                        .cloned()
                        .collect();
                    for e in src {
                        let mut k = e.kind.clone();
                        let mut ent = crate::model::Entity::new(0, k.clone());
                        ent.translate(d);
                        k = ent.kind;
                        new_ids.push(self.doc.add(k));
                    }
                    self.selected = new_ids;
                    self.pts.clear();
                    self.status = "已复制图元".to_string();
                }
            }
            Tool::Mirror => {
                if self.selected.is_empty() {
                    self.status = "请先选择要镜像的图元".to_string();
                    return;
                }
                self.pts.push(p);
                if self.pts.len() == 2 {
                    let p1 = self.pts[0];
                    let p2 = self.pts[1];
                    let mut new_ids = Vec::new();
                    self.push_undo();
                    let src: Vec<crate::model::Entity> = self
                        .doc
                        .entities
                        .iter()
                        .filter(|e| self.selected.contains(&e.id))
                        .cloned()
                        .collect();
                    for e in src {
                        let mut ent = crate::model::Entity::new(0, e.kind.clone());
                        ent.mirror(p1, p2);
                        let k = ent.kind;
                        new_ids.push(self.doc.add(k));
                    }
                    self.selected = new_ids;
                    self.pts.clear();
                    self.status = "已镜像图元".to_string();
                }
            }
            Tool::Rotate => {
                if self.selected.is_empty() {
                    self.status = "请先选择要旋转的图元".to_string();
                    return;
                }
                self.pts.push(p);
                if self.pts.len() == 3 {
                    let c = self.pts[0];
                    let a0 = self.pts[1].sub(c).angle();
                    let a1 = self.pts[2].sub(c).angle();
                    let ang = a1 - a0;
                    self.push_undo();
                    for e in &mut self.doc.entities {
                        if self.selected.contains(&e.id) {
                            e.rotate(c, ang);
                        }
                    }
                    self.pts.clear();
                    self.status = format!("已旋转 {:.2}°", ang * 180.0 / std::f32::consts::PI);
                }
            }
            Tool::Point => {
                self.push_undo();
                let id = self.doc.add(Kind::Point { p });
                self.selected = vec![id];
                self.status = format!("已绘制点 {:.3},{:.3}", p.x, p.y);
            }
            Tool::Line => {
                self.pts.push(p);
                if self.pts.len() == 2 {
                    self.push_undo();
                    let id = self.doc.add(Kind::Line { a: self.pts[0], b: self.pts[1] });
                    self.selected = vec![id];
                    self.pts.clear();
                    self.status = "已绘制直线".to_string();
                }
            }
            Tool::Polyline => {
                self.pts.push(p);
                self.status = "指定下一点，右键或回车结束".to_string();
            }
            Tool::Rect => {
                self.pts.push(p);
                if self.pts.len() == 2 {
                    let a = self.pts[0];
                    let b = self.pts[1];
                    self.push_undo();
                    let pts = [
                        Pt::new(a.x, a.y),
                        Pt::new(b.x, a.y),
                        Pt::new(b.x, b.y),
                        Pt::new(a.x, b.y),
                    ];
                    let mut ids = Vec::new();
                    for i in 0..4 {
                        ids.push(self.doc.add(Kind::Line { a: pts[i], b: pts[(i + 1) % 4] }));
                    }
                    self.selected = ids;
                    self.pts.clear();
                    self.status = "已绘制矩形".to_string();
                }
            }
            Tool::Circle => {
                self.pts.push(p);
                if self.pts.len() == 2 {
                    let c = self.pts[0];
                    let r = c.dist(self.pts[1]);
                    if r > 0.01 {
                        self.push_undo();
                        let id = self.doc.add(Kind::Circle { c, r });
                        self.selected = vec![id];
                        self.status = format!("已绘制圆 R{:.3}", r);
                    }
                    self.pts.clear();
                }
            }
            Tool::Arc => {
                self.pts.push(p);
                if self.pts.len() == 3 {
                    let c = self.pts[0];
                    let r = c.dist(self.pts[1]);
                    let a0 = self.pts[1].sub(c).angle();
                    let a1 = self.pts[2].sub(c).angle();
                    let mut sweep = (a1 - a0).rem_euclid(TAU);
                    if sweep < 0.001 {
                        sweep = TAU;
                    }
                    if r > 0.01 {
                        self.push_undo();
                        let id = self.doc.add(Kind::Arc { c, r, a0, sweep });
                        self.selected = vec![id];
                        self.status = format!("已绘制圆弧 R{:.3}", r);
                    }
                    self.pts.clear();
                }
            }
            _ => {}
        }
    }

    fn finish_polyline(&mut self, close: bool) {
        let mut pts: Vec<Pt> = Vec::new();
        for p in &self.pts {
            if pts.last().map(|q: &Pt| q.dist(*p) > 0.001).unwrap_or(true) {
                pts.push(*p);
            }
        }
        if pts.len() < 2 {
            self.pts.clear();
            return;
        }
        self.pts = pts.clone();
        self.push_undo();
        let mut last = None;
        for i in 0..pts.len() - 1 {
            last = Some(self.doc.add(Kind::Line { a: pts[i], b: pts[i + 1] }));
        }
        if close || pts[0].dist(*pts.last().unwrap()) < 0.01 {
            last = Some(self.doc.add(Kind::Line {
                a: *pts.last().unwrap(),
                b: pts[0],
            }));
        }
        if let Some(id) = last {
            self.selected = vec![id];
        }
        self.pts.clear();
        self.status = "已绘制多段线".to_string();
    }

    fn delete_selected(&mut self) {
        if self.selected.is_empty() {
            self.status = "没有选中的图元".to_string();
            return;
        }
        self.push_undo();
        let ids = self.selected.clone();
        self.doc.remove_ids(&ids);
        self.selected.clear();
        self.cut_job = None;
        self.last_code = None;
        self.status = "已删除图元".to_string();
    }

    fn trim_click(&mut self, p: Pt) {
        let Some(id) = self.hit_entity(p, 8.0) else {
            return;
        };
        let Some(ent) = self.doc.get(id).cloned() else { return };
        let Kind::Line { a, b } = ent.kind else {
            self.status = "当前版本只支持修剪直线".to_string();
            return;
        };
        let ab = b.sub(a);
        let l2 = ab.dot(ab);
        if l2 < 1e-9 {
            return;
        }
        let t_click = (p.sub(a).dot(ab) / l2).clamp(0.0, 1.0);
        let mut ts = Vec::new();
        for e in &self.doc.entities {
            if e.id == id {
                continue;
            }
            if let Kind::Line { a: c, b: d } = e.kind {
                if let Some(ip) = crate::model::segment_intersection(a, b, c, d) {
                    let t = ip.sub(a).dot(ab) / l2;
                    if t > 0.001 && t < 0.999 {
                        ts.push(t);
                    }
                }
            }
        }
        if ts.is_empty() {
            self.status = "这条直线没有可修剪的交点".to_string();
            return;
        }
        ts.sort_by(|x, y| x.partial_cmp(y).unwrap());
        ts.dedup_by(|x, y| (*x - *y).abs() < 1e-4);
        let mut bounds = vec![0.0];
        bounds.extend(ts);
        bounds.push(1.0);
        let mut remove_idx = None;
        for i in 0..bounds.len() - 1 {
            if t_click >= bounds[i] && t_click <= bounds[i + 1] {
                remove_idx = Some(i);
                break;
            }
        }
        let Some(rm) = remove_idx else { return };
        self.push_undo();
        self.doc.remove_ids(&[id]);
        for (i, w) in bounds.windows(2).enumerate() {
            if i == rm {
                continue;
            }
            let p1 = a.add(ab.mul(w[0]));
            let p2 = a.add(ab.mul(w[1]));
            self.doc.add(Kind::Line { a: p1, b: p2 });
        }
        self.selected.clear();
        self.status = "已修剪直线".to_string();
    }

    fn submit_command(&mut self) {
        let text = self.command.trim().to_string();
        if text.is_empty() {
            return;
        }
        let cleaned = text.replace('，', ",");
        let parts: Vec<&str> = if cleaned.contains(',') {
            cleaned.split(',').map(|s| s.trim()).collect()
        } else {
            cleaned.split_whitespace().collect()
        };
        if parts.len() >= 2 {
            if let (Ok(x), Ok(y)) = (parts[0].parse::<f32>(), parts[1].parse::<f32>()) {
                self.command.clear();
                let s = self.w2s(Pt::new(x, y));
                self.mouse = s;
                self.mouse_in_canvas = self.canvas.contains(s.0, s.1);
                if self.tool != Tool::Select {
                    self.canvas_down(s.0, s.1, false);
                } else {
                    self.status = format!("坐标 {:.3},{:.3}", x, y);
                }
                return;
            }
        }
        self.command.clear();
        self.status = "请输入坐标，例如 20,15".to_string();
    }

    // ---------- menus / actions ----------

    pub fn do_act(&mut self, act: Act) {
        if !matches!(act, Act::OpenMenu(_)) {
            self.menu_open = None;
        }
        match act {
            Act::None => {}
            Act::New => self.request_new(),
            Act::Open => self.request_open(),
            Act::Save => self.save(false),
            Act::SaveAs => self.save(true),
            Act::Import3B => self.import_3b(),
            Act::ImportDxf => self.import_dxf(),
            Act::FileInfo => self.open_file_info(),
            Act::Preview => {
                self.menu_open = None;
                self.dialog = Some(Dialog::Preview);
                self.focus = None;
            }
            Act::Exit => self.request_close(),
            Act::Undo => {
                self.undo();
                self.menu_open = None;
            }
            Act::Redo => {
                self.redo();
                self.menu_open = None;
            }
            Act::SelectAll => {
                self.selected = self.doc.all_ids();
                self.status = format!("已选择 {} 个图元", self.selected.len());
                self.menu_open = None;
            }
            Act::ClearSel => {
                self.selected.clear();
                self.reset_tool_state();
                self.menu_open = None;
            }
            Act::DeleteSel => {
                self.delete_selected();
                self.menu_open = None;
            }
            Act::EditOffset => {
                self.open_prompt(PromptAct::Offset);
                self.menu_open = None;
            }
            Act::EditArray => {
                self.open_array();
                self.menu_open = None;
            }
            Act::EditScale => {
                self.open_prompt(PromptAct::Scale);
                self.menu_open = None;
            }
            Act::SetTool(t) => self.set_tool(t),
            Act::Redraw => {
                self.status = "图板已重画".to_string();
                self.menu_open = None;
            }
            Act::FitAll => {
                self.fit_view();
                self.status = "已显示全部图形".to_string();
                self.menu_open = None;
            }
            Act::Pan => self.set_tool(Tool::Pan),
            Act::ZoomIn => {
                let cx = self.canvas.x + self.canvas.w * 0.5;
                let cy = self.canvas.y + self.canvas.h * 0.5;
                self.zoom_at(1.25, cx, cy);
                self.menu_open = None;
            }
            Act::ZoomOut => {
                let cx = self.canvas.x + self.canvas.w * 0.5;
                let cy = self.canvas.y + self.canvas.h * 0.5;
                self.zoom_at(0.8, cx, cy);
                self.menu_open = None;
            }
            Act::QueryDist => {
                self.set_tool(Tool::Measure);
            }
            Act::QueryArea => self.query_area(),
            Act::QueryLength => self.query_length(),
            Act::ToggleCenter => {
                self.settings.show_center = !self.settings.show_center;
                self.settings.save();
                self.menu_open = None;
            }
            Act::ToggleNode => {
                self.settings.show_node = !self.settings.show_node;
                self.settings.save();
                self.menu_open = None;
            }
            Act::ToggleCut => {
                self.settings.show_cut = !self.settings.show_cut;
                self.settings.save();
                self.menu_open = None;
            }
            Act::Settings => self.open_settings(),
            Act::CutProgram => self.open_cut_dialog(),
            Act::FoamProgram => self.open_foam_dialog(),
            Act::CalcArea => self.open_calc(),
            Act::CodeCheck => self.code_check(),
            Act::CodeShow => self.open_code_dialog(),
            Act::CodeSave => self.save_code(),
            Act::CodeTransfer => self.open_transfer(),
            Act::DeleteCut => {
                self.cut_job = None;
                self.last_code = None;
                self.status = "已删除切割路径".to_string();
                self.menu_open = None;
            }
            Act::Tips => {
                self.dialog = Some(Dialog::Tips(self.tip_index % TIPS.len()));
                self.tip_index += 1;
                self.focus = None;
            }
            Act::About => {
                self.dialog = Some(Dialog::About);
                self.focus = None;
            }
            Act::OpenMenu(i) => {
                if self.menu_open == Some(i) {
                    self.menu_open = None;
                } else {
                    self.menu_open = Some(i);
                }
                self.focus = None;
            }
            Act::CloseMenu => self.menu_open = None,
            Act::ToggleOrtho => self.ortho = !self.ortho,
            Act::TogglePolar => self.polar = !self.polar,
            Act::ToggleOsnap => self.osnap = !self.osnap,
            Act::ToggleGrid => self.grid = !self.grid,
            Act::ToggleSnap(bit) => {
                if bit == 0xff {
                    self.snap_modes = 0;
                } else {
                    self.snap_modes ^= bit;
                    if self.snap_modes != 0 {
                        self.osnap = true;
                    }
                }
            }
            Act::SnapAll => {
                self.snap_modes = 1 | 2 | 4 | 8 | 16;
                self.osnap = true;
            }
            Act::SnapNone => self.snap_modes = 0,
            _ => {}
        }
    }

    fn import_dxf(&mut self) {
        let Some(path) =
            platform::open_file_dialog(self.hwnd, "导入 DXF 图形", "DXF 图形", "dxf")
        else {
            return;
        };
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                self.open_message("导入 DXF", &format!("无法读取文件：{e}"), true);
                return;
            }
        };
        match crate::dxf::import(&text) {
            Ok(kinds) => {
                let count = kinds.len();
                let empty = self.doc.entities.is_empty();
                for k in kinds {
                    self.doc.add(k);
                }
                if empty {
                    if let Some(name) = path.file_stem() {
                        let stem = name.to_string_lossy().to_string();
                        self.doc.title = format!("{stem}.cnc");
                        self.doc.part = stem;
                    }
                }
                self.path = None;
                self.dirty = true;
                self.fitted = false;
                self.cut_job = None;
                self.last_code = None;
                self.selected.clear();
                self.status = format!("已从 DXF 导入 {} 个图元", count);
                self.menu_open = None;
            }
            Err(e) => self.open_message("导入 DXF", &e, true),
        }
    }

    fn import_3b(&mut self) {        let Some(path) = platform::open_file_dialog(
            self.hwnd,
            "读入 3B 程序",
            "3B 代码",
            "3b",
        ) else {
            return;
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let n = text.lines().filter(|l| !l.trim().is_empty()).count();
                self.open_message(
                    "读入 3B 程序",
                    &format!(
                        "已读入 {} 行 3B 代码。\n此版本只读取文本作参考，不把 3B 代码还原成图形；\n当前图中的轮廓仍按“编程 → 切割编程”生成代码。",
                        n
                    ),
                    false,
                );
            }
            Err(e) => self.open_message("读取失败", &format!("无法读取文件：{e}"), true),
        }
    }

    fn request_new(&mut self) {
        if self.dirty {
            self.dialog = Some(Dialog::Confirm {
                title: "新建图形".to_string(),
                text: "当前图形还没有保存，新建会丢失修改。要继续吗？".to_string(),
                action: ConfirmAct::New,
            });
            self.focus = None;
        } else {
            self.new_doc();
        }
    }

    fn request_open(&mut self) {
        if self.dirty {
            self.dialog = Some(Dialog::Confirm {
                title: "打开图形".to_string(),
                text: "当前图形还没有保存，继续打开会丢失修改。要继续吗？".to_string(),
                action: ConfirmAct::Open,
            });
            self.focus = None;
        } else {
            self.open_native();
        }
    }

    fn new_doc(&mut self) {
        self.doc = Doc::new();
        self.path = None;
        self.dirty = false;
        self.selected.clear();
        self.cut_job = None;
        self.last_code = None;
        self.undo.clear();
        self.redo.clear();
        self.fitted = false;
        self.view_center = Pt::new(0.0, 0.0);
        self.scale = 4.0;
        self.reset_tool_state();
        self.tool = Tool::Select;
        self.dialog = None;
        self.status = "已新建空白图形".to_string();
    }

    fn open_native(&mut self) {
        let Some(path) = platform::open_file_dialog(self.hwnd, "打开图形", "CNC 图形文件", "cnc") else {
            return;
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => match Doc::from_text(&text) {
                Ok(doc) => {
                    self.doc = doc;
                    self.path = Some(path.clone());
                    self.dirty = false;
                    self.selected.clear();
                    self.cut_job = None;
                    self.last_code = None;
                    self.undo.clear();
                    self.redo.clear();
                    self.fitted = false;
                    self.dialog = None;
                    self.status = format!("已打开 {}", path.display());
                }
                Err(e) => self.open_message("文件格式错误", &format!("无法打开此文件：{e}"), true),
            },
            Err(e) => self.open_message("读取失败", &format!("无法读取文件：{e}"), true),
        }
    }

    fn save(&mut self, as_new: bool) {
        if !as_new {
            if let Some(path) = self.path.clone() {
                self.write_doc(&path);
                return;
            }
        }
        let default = if self.doc.title.ends_with(".cnc") && self.doc.title != "未命名" {
            self.doc.title.clone()
        } else if self.doc.part.trim().is_empty() || self.doc.part == "未命名" {
            "未命名.cnc".to_string()
        } else {
            format!("{}.cnc", self.doc.part)
        };
        let Some(path) =
            platform::save_file_dialog(self.hwnd, "图形存盘", "CNC 图形文件", "cnc", &default)
        else {
            return;
        };
        self.write_doc(&path);
    }

    fn write_doc(&mut self, path: &std::path::Path) {
        match std::fs::write(path, self.doc.save_text()) {
            Ok(_) => {
                self.path = Some(path.to_path_buf());
                if let Some(name) = path.file_name() {
                    self.doc.title = name.to_string_lossy().to_string();
                }
                self.dirty = false;
                self.status = format!("已保存到 {}", path.display());
            }
            Err(e) => self.open_message("保存失败", &format!("无法写入文件：{e}"), true),
        }
    }

    fn save_native_text(&mut self, text: &str, title: &str, ext: &str, default: &str) -> bool {
        let Some(path) = platform::save_file_dialog(self.hwnd, title, "代码文件", ext, default) else {
            return false;
        };
        match std::fs::write(&path, text) {
            Ok(_) => {
                self.status = format!("已保存到 {}", path.display());
                true
            }
            Err(e) => {
                self.open_message("保存失败", &format!("无法写入文件：{e}"), true);
                false
            }
        }
    }

    pub fn request_close(&mut self) {
        if self.dirty {
            self.dialog = Some(Dialog::Confirm {
                title: "退出系统".to_string(),
                text: "图形还没有保存，退出会丢失修改。要退出吗？".to_string(),
                action: ConfirmAct::Exit,
            });
            self.focus = None;
        } else {
            platform::destroy_window(self.hwnd);
        }
    }

    pub fn open_demo(&mut self, name: &str) {
        match name {
            "cut" => self.open_cut_dialog(),
            "foam" => self.open_foam_dialog(),
            "code" => {
                self.ensure_demo_code();
                self.open_code_dialog();
            }
            "settings" => self.open_settings(),
            "calc" => self.open_calc(),
            "fileinfo" => self.open_file_info(),
            "array" => {
                self.selected = self.doc.all_ids();
                self.open_array();
            }
            "prompt" => {
                self.selected = self.doc.all_ids();
                self.open_prompt(PromptAct::Offset);
            }
            "transfer" => {
                self.ensure_demo_code();
                self.open_transfer();
            }
            "tips" => {
                self.dialog = Some(Dialog::Tips(0));
                self.focus = None;
            }
            "about" => {
                self.dialog = Some(Dialog::About);
                self.focus = None;
            }
            "preview" => {
                self.dialog = Some(Dialog::Preview);
                self.focus = None;
            }
            "message" => self.open_message(
                "提示",
                "这是一个提示窗口示例。\n提示文字会自动换行，适合显示校验结果和操作说明。",
                false,
            ),
            _ => {}
        }
    }

    fn ensure_demo_code(&mut self) {
        if self.last_code.is_some() {
            return;
        }
        let ids = self.doc.all_ids();
        let opt = CodeOptions {
            kind: self.settings.code_kind,
            pierce: Pt::new(-90.0, -77.5),
            comp: 0.09,
            side: -1,
            part: self.doc.part.clone(),
            optimize: true,
            feed: 180.0,
            rapid: 3000.0,
        };
        if let Ok(mut out) = generate(&self.doc, &ids, &opt) {
            out.path_count = out.job.paths.len();
            self.cut_job = Some(out.job.clone());
            self.last_code = Some(out);
        }
    }
    fn open_message(&mut self, title: &str, text: &str, error: bool) {
        self.dialog = Some(Dialog::Message {
            title: title.to_string(),
            text: text.to_string(),
            error,
        });
        self.focus = None;
    }

    fn open_file_info(&mut self) {
        self.dialog = Some(Dialog::FileInfo(FileInfoState {
            part: self.doc.part.clone(),
            unit: self.doc.unit.clone(),
            programmer: self.doc.programmer.clone(),
            note: self.doc.note.clone(),
        }));
        self.focus = Some("fi_part".to_string());
        self.field_selected = true;
    }

    fn open_settings(&mut self) {
        self.dialog = Some(Dialog::Settings(SettingsState {
            tab: 0,
            work_area: format!("{:.0}", self.settings.work_area),
            work_money: format!("{:.2}", self.settings.work_money),
            moly: format!("{:.3}", self.settings.moly),
            unit: self.settings.unit.clone(),
            center: self.settings.show_center,
            node: self.settings.show_node,
            cut: self.settings.show_cut,
            line_no: self.settings.line_numbers,
            bg: self.settings.bg,
            code_kind: self.settings.code_kind.index(),
        }));
        self.focus = Some("set_area".to_string());
        self.field_selected = true;
    }

    fn selected_or_all(&self) -> Vec<u64> {
        if self.selected.is_empty() {
            self.doc.all_ids()
        } else {
            self.selected.clone()
        }
    }

    fn open_cut_dialog(&mut self) {
        if self.doc.entities.is_empty() {
            self.open_message("切割编程", "图上还没有图形，请先绘制轮廓。", true);
            return;
        }
        let bbox = self.doc.bounds().unwrap();
        let pierce = self
            .cut_job
            .as_ref()
            .and_then(|j| j.paths.first().map(|p| p.pierce))
            .unwrap_or_else(|| Pt::new(bbox.min.x, bbox.min.y - 8.0));
        let comp = (self.settings.moly * 0.5 + 0.02).max(0.03);
        self.dialog = Some(Dialog::Cut(CutState {
            kind: self.settings.code_kind.index(),
            side: 1,
            px: format!("{:.3}", pierce.x),
            py: format!("{:.3}", pierce.y),
            comp: format!("{:.3}", comp),
            name: self.doc.part.clone(),
            optimize: true,
            error: None,
        }));
        self.focus = Some("cut_px".to_string());
        self.field_selected = true;
        self.menu_open = None;
    }

    fn generate_from_cut(&mut self, st: &CutState) {
        let px = st.px.trim().parse::<f32>().unwrap_or(f32::NAN);
        let py = st.py.trim().parse::<f32>().unwrap_or(f32::NAN);
        let comp = st.comp.trim().parse::<f32>().unwrap_or(f32::NAN);
        if !px.is_finite() || !py.is_finite() || !comp.is_finite() || comp < 0.0 {
            if let Some(Dialog::Cut(s)) = &mut self.dialog {
                s.error = Some("请输入有效的穿丝孔坐标和补偿量".to_string());
            }
            return;
        }
        let ids = self.selected_or_all();
        let kind = CodeKind::from_index(st.kind);
        let side = match st.side {
            0 => 1,
            1 => -1,
            _ => 0,
        };
        let opt = CodeOptions {
            kind,
            pierce: Pt::new(px, py),
            comp,
            side,
            part: st.name.clone(),
            optimize: st.optimize,
            feed: 180.0,
            rapid: 3000.0,
        };
        match generate(&self.doc, &ids, &opt) {
            Ok(mut out) => {
                out.path_count = out.job.paths.len();
                self.cut_job = Some(out.job.clone());
                self.settings.code_kind = kind;
                self.settings.save();
                self.last_code = Some(out);
                self.status = format!("已生成 {} 代码", kind.label());
                self.close_dialog();
                self.open_code_dialog();
            }
            Err(e) => {
                if let Some(Dialog::Cut(s)) = &mut self.dialog {
                    s.error = Some(e);
                }
            }
        }
    }

    fn open_foam_dialog(&mut self) {
        if self.doc.entities.is_empty() {
            self.open_message("泡沫切割", "图上还没有图形，请先绘制要切割的轮廓。", true);
            return;
        }
        let bbox = self.doc.bounds().unwrap();
        let start = Pt::new(bbox.min.x - 12.0, bbox.min.y - 12.0);
        self.dialog = Some(Dialog::Foam(FoamState {
            mode: if self.settings.foam_four_axis { 1 } else { 0 },
            side: self.settings.foam_side.min(2),
            sx: format!("{:.3}", start.x),
            sy: format!("{:.3}", start.y),
            kerf: format!("{:.3}", self.settings.foam_kerf),
            feed: format!("{:.0}", self.settings.foam_feed),
            rapid: format!("{:.0}", self.settings.foam_rapid),
            lead: format!("{:.2}", self.settings.foam_lead),
            tx: format!("{:.3}", self.settings.foam_taper_x),
            ty: format!("{:.3}", self.settings.foam_taper_y),
            optimize: self.settings.foam_optimize,
            continuous: self.settings.foam_continuous,
            name: self.doc.part.clone(),
            error: None,
        }));
        self.focus = Some("foam_sx".to_string());
        self.field_selected = true;
        self.menu_open = None;
    }

    fn generate_foam_from_state(&mut self, st: &FoamState) {
        let sx = st.sx.trim().parse::<f32>().unwrap_or(f32::NAN);
        let sy = st.sy.trim().parse::<f32>().unwrap_or(f32::NAN);
        let kerf = st.kerf.trim().parse::<f32>().unwrap_or(f32::NAN);
        let feed = st.feed.trim().parse::<f32>().unwrap_or(f32::NAN);
        let rapid = st.rapid.trim().parse::<f32>().unwrap_or(f32::NAN);
        let lead = st.lead.trim().parse::<f32>().unwrap_or(f32::NAN);
        let tx = st.tx.trim().parse::<f32>().unwrap_or(f32::NAN);
        let ty = st.ty.trim().parse::<f32>().unwrap_or(f32::NAN);
        if !sx.is_finite()
            || !sy.is_finite()
            || !kerf.is_finite()
            || !feed.is_finite()
            || !rapid.is_finite()
            || !lead.is_finite()
            || !tx.is_finite()
            || !ty.is_finite()
        {
            if let Some(Dialog::Foam(s)) = &mut self.dialog {
                s.error = Some("请输入有效的坐标、割缝、速度和引入线参数".to_string());
            }
            return;
        }
        let side = match st.side {
            0 => -1,
            1 => 1,
            _ => 0,
        };
        let opt = FoamOptions {
            four_axis: st.mode == 1,
            side,
            start: Pt::new(sx, sy),
            kerf,
            feed,
            rapid,
            lead,
            taper_x: tx,
            taper_y: ty,
            continuous: st.continuous,
            optimize: st.optimize,
            part: st.name.clone(),
        };
        match foam::generate(&self.doc, &self.selected_or_all(), &opt) {
            Ok(out) => {
                let path_count = out.path_count;
                let mode = if out.four_axis { "四轴 XYAB" } else { "两轴 XY" };
                let continuous = if out.continuous { " · 连续切割" } else { "" };
                let code = CodeOutput {
                    text: out.text,
                    job: out.job,
                    kind: CodeKind::Mach3,
                    line_count: out.line_count,
                    path_count,
                    extra: out.extra,
                };
                self.cut_job = Some(code.job.clone());
                self.last_code = Some(code);
                self.settings.foam_four_axis = st.mode == 1;
                self.settings.foam_side = st.side;
                self.settings.foam_kerf = kerf;
                self.settings.foam_feed = feed;
                self.settings.foam_rapid = rapid;
                self.settings.foam_lead = lead;
                self.settings.foam_taper_x = tx;
                self.settings.foam_taper_y = ty;
                self.settings.foam_optimize = st.optimize;
                self.settings.foam_continuous = st.continuous;
                self.settings.save();
                self.status = format!(
                    "已生成 Mach3 泡沫切割程序：{} · {} 条路径{}",
                    mode, path_count, continuous
                );
                self.close_dialog();
                self.open_code_dialog();
            }
            Err(e) => {
                if let Some(Dialog::Foam(s)) = &mut self.dialog {
                    s.error = Some(e);
                }
            }
        }
    }

    fn open_code_dialog(&mut self) {
        let Some(code) = &self.last_code else {
            self.open_message("代码显示", "还没有生成代码。请先在“编程 → 切割编程”或“编程 → 泡沫切割”中生成程序。", true);
            return;
        };
        let title = match code.kind {
            CodeKind::ThreeB => "3B 代码",
            CodeKind::GCode => "G 代码",
            CodeKind::Mach3 => "Mach3 代码",
        };
        let mut summary = format!(
            "{} 行 · {} 条路径 · 切割长度 {:.2} mm",
            code.line_count,
            code.job.paths.len(),
            code.job.length()
        );
        if !code.extra.is_empty() {
            summary.push_str(" · ");
            summary.push_str(&code.extra);
        }
        self.dialog = Some(Dialog::Code(CodeState {
            text: code.text.clone(),
            title: title.to_string(),
            kind: code.kind,
            scroll: 0.0,
            font: 13.0,
            summary,
        }));
        self.focus = None;
        self.menu_open = None;
    }

    fn save_code(&mut self) {
        let Some(code) = self.last_code.clone() else {
            self.open_message("代码存盘", "还没有生成代码。请先执行“切割编程”。", true);
            return;
        };
        let stem = self
            .doc
            .part
            .trim()
            .trim_end_matches(".cnc")
            .to_string();
        let stem = if stem.is_empty() || stem == "未命名" {
            "程序".to_string()
        } else {
            stem
        };
        let default = format!("{}.{}", stem, code.kind.extension());
        let text = code.text.clone();
        let ext = code.kind.extension();
        self.save_native_text(&text, "代码存盘", ext, &default);
    }

    fn code_check(&mut self) {
        let Some(code) = &self.last_code else {
            self.open_message("代码校验", "还没有生成代码。请先执行“切割编程”。", true);
            return;
        };
        let result = validate(&code.text, code.kind);
        match result {
            Ok(msg) => {
                self.status = msg.clone();
                if matches!(self.dialog, Some(Dialog::Code(_))) {
                    return;
                }
                self.open_message("代码校验", &msg, false)
            }
            Err(e) => {
                self.status = format!("校验失败：{e}");
                if matches!(self.dialog, Some(Dialog::Code(_))) {
                    return;
                }
                self.open_message("代码校验", &e, true)
            }
        }
    }

    fn open_transfer(&mut self) {
        if self.last_code.is_none() {
            self.open_message("代码传送", "还没有生成代码，无法传送。", true);
            return;
        }
        self.dialog = Some(Dialog::Transfer(TransferState {
            port: 0,
            baud: 2,
            progress: 0.0,
            message: "演示版本：未连接机床，开始传送只模拟进度。".to_string(),
        }));
        self.focus = None;
    }

    fn open_calc(&mut self) {
        let length = self.cut_job.as_ref().map(|j| j.length()).unwrap_or_else(|| {
            build_job(&self.doc, &self.selected_or_all(), Pt::new(0.0, 0.0), 0.0, 0)
                .map(|j| j.length())
                .unwrap_or_else(|_| self.doc.total_length())
        });
        let thickness = 20.0f32;
        let area = length * thickness;
        let hours = if self.settings.work_area > 0.0 {
            area / self.settings.work_area
        } else {
            0.0
        };
        self.dialog = Some(Dialog::Calc(CalcState {
            thickness: format!("{:.1}", thickness),
            length,
            area,
            hours,
            money: hours * self.settings.work_money,
        }));
        self.focus = Some("calc_th".to_string());
        self.field_selected = true;
    }

    fn query_area(&mut self) {
        let ids = self.selected_or_all();
        if ids.is_empty() {
            self.open_message("面积查询", "图上还没有图形。", true);
            return;
        }
        match build_job(&self.doc, &ids, Pt::new(0.0, 0.0), 0.0, 0) {
            Ok(job) => {
                let area: f32 = job.area();
                let len: f32 = job.length();
                self.open_message(
                    "封闭环面积",
                    &format!(
                        "封闭轮廓面积：{:.3} mm²\n切割长度：{:.3} mm\n路径数：{}",
                        area,
                        len,
                        job.paths.len()
                    ),
                    false,
                );
            }
            Err(e) => self.open_message("面积查询", &e, true),
        }
    }

    fn query_length(&mut self) {
        let ids = self.selected_or_all();
        if ids.is_empty() {
            self.open_message("周长查询", "图上还没有图形。", true);
            return;
        }
        let len: f32 = ids
            .iter()
            .filter_map(|id| self.doc.get(*id))
            .map(|e| e.length())
            .sum();
        let closed = build_job(&self.doc, &ids, Pt::new(0.0, 0.0), 0.0, 0)
            .map(|j| j.length())
            .unwrap_or(len);
        self.open_message(
            "周长查询",
            &format!("图元总长度：{:.3} mm\n按路径统计：{:.3} mm", len, closed),
            false,
        );
    }

    fn open_array(&mut self) {
        if self.selected.is_empty() {
            self.open_message("阵列", "请先选择要阵列的图元。", true);
            return;
        }
        self.dialog = Some(Dialog::Array(ArrayState {
            rows: "2".to_string(),
            cols: "2".to_string(),
            dx: "30".to_string(),
            dy: "30".to_string(),
        }));
        self.focus = Some("arr_rows".to_string());
        self.field_selected = true;
    }

    fn apply_array(&mut self, st: &ArrayState) {
        let rows = st.rows.trim().parse::<i32>().unwrap_or(0);
        let cols = st.cols.trim().parse::<i32>().unwrap_or(0);
        let dx = st.dx.trim().parse::<f32>().unwrap_or(0.0);
        let dy = st.dy.trim().parse::<f32>().unwrap_or(0.0);
        if rows < 1 || cols < 1 || rows * cols > 2000 {
            self.open_message("阵列", "行数和列数必须是正整数，且总数不超过 2000。", true);
            return;
        }
        self.push_undo();
        let src: Vec<crate::model::Entity> = self
            .doc
            .entities
            .iter()
            .filter(|e| self.selected.contains(&e.id))
            .cloned()
            .collect();
        let mut new_ids = Vec::new();
        for r in 0..rows {
            for c in 0..cols {
                if r == 0 && c == 0 {
                    continue;
                }
                for e in &src {
                    let mut ent = crate::model::Entity::new(0, e.kind.clone());
                    ent.translate(Pt::new(dx * c as f32, dy * r as f32));
                    new_ids.push(self.doc.add(ent.kind));
                }
            }
        }
        self.selected = new_ids;
        self.status = format!("已阵列 {} 组图元", rows * cols);
        self.close_dialog();
    }

    fn open_prompt(&mut self, action: PromptAct) {
        if self.selected.is_empty() {
            self.open_message("编辑", "请先选择要编辑的图元。", true);
            return;
        }
        let (title, label, hint, value) = match action {
            PromptAct::Offset => (
                "偏移",
                "偏移距离 (mm)",
                "正值向图元左侧偏移，负值向右。圆和圆弧按半径增减。",
                "2.0",
            ),
            PromptAct::Scale => (
                "缩放",
                "缩放比例",
                "以选中图元的中心为基点，小于 1 缩小，大于 1 放大。",
                "0.5",
            ),
        };
        self.dialog = Some(Dialog::Prompt(PromptState {
            title: title.to_string(),
            label: label.to_string(),
            hint: hint.to_string(),
            value: value.to_string(),
            action,
        }));
        self.focus = Some("prompt_value".to_string());
        self.field_selected = true;
    }

    fn apply_prompt(&mut self, st: &PromptState) {
        let v = st.value.trim().parse::<f32>().unwrap_or(f32::NAN);
        if !v.is_finite() {
            self.open_message("输入错误", "请输入有效的数字。", true);
            return;
        }
        match st.action {
            PromptAct::Offset => {
                self.push_undo();
                let src: Vec<crate::model::Entity> = self
                    .doc
                    .entities
                    .iter()
                    .filter(|e| self.selected.contains(&e.id))
                    .cloned()
                    .collect();
                let mut new_ids = Vec::new();
                for e in src {
                    if let Some(k) = e.offset(v) {
                        new_ids.push(self.doc.add(k));
                    }
                }
                self.selected = new_ids;
                self.status = format!("已偏移 {:.3} mm", v);
            }
            PromptAct::Scale => {
                if v.abs() < 0.001 {
                    self.open_message("输入错误", "缩放比例不能为 0。", true);
                    return;
                }
                let bb = self
                    .doc
                    .entities
                    .iter()
                    .filter(|e| self.selected.contains(&e.id))
                    .map(|e| e.bbox())
                    .reduce(|a, b| {
                        let mut c = a;
                        c.union(b);
                        c
                    });
                let Some(bb) = bb else { return };
                let c = bb.center();
                self.push_undo();
                for e in &mut self.doc.entities {
                    if self.selected.contains(&e.id) {
                        e.scale(c, v);
                    }
                }
                self.status = format!("已缩放 {:.3} 倍", v);
            }
        }
        self.close_dialog();
    }

    // ---------- dialogs ----------

    fn close_dialog(&mut self) {
        self.dialog = None;
        self.focus = None;
        self.field_selected = false;
        self.field_hits.clear();
        self.dialog_hits.clear();
    }

    fn on_dialog_act(&mut self, act: Act) {
        match act {
            Act::DlgClose => self.close_dialog(),
            Act::DlgButton(i) => self.on_dialog_button(i),
            Act::DlgTab(i) => {
                if let Some(Dialog::Settings(s)) = &mut self.dialog {
                    s.tab = i as usize;
                }
                self.focus = None;
                self.field_selected = false;
            }
            Act::DlgToggle(i) => {
                if let Some(Dialog::Settings(s)) = &mut self.dialog {
                    match i {
                        0 => s.center = !s.center,
                        1 => s.node = !s.node,
                        2 => s.cut = !s.cut,
                        3 => s.line_no = !s.line_no,
                        _ => {}
                    }
                } else if let Some(Dialog::Cut(s)) = &mut self.dialog {
                    if i == 4 {
                        s.optimize = !s.optimize;
                    }
                } else if let Some(Dialog::Foam(s)) = &mut self.dialog {
                    match i {
                        5 => s.optimize = !s.optimize,
                        6 => s.continuous = !s.continuous,
                        _ => {}
                    }
                }
            }
            Act::DlgCycle(i) => {
                if let Some(Dialog::Settings(s)) = &mut self.dialog {
                    match i {
                        10 => s.bg = (s.bg + 1) % 3,
                        11 => s.code_kind = (s.code_kind + 1) % 3,
                        _ => {}
                    }
                } else if let Some(Dialog::Cut(s)) = &mut self.dialog {
                    match i {
                        0 => s.kind = (s.kind + 1) % 3,
                        1 => s.side = (s.side + 1) % 3,
                        _ => {}
                    }
                } else if let Some(Dialog::Foam(s)) = &mut self.dialog {
                    match i {
                        20 => s.mode = (s.mode + 1) % 2,
                        21 => s.side = (s.side + 1) % 3,
                        _ => {}
                    }
                } else if let Some(Dialog::Transfer(s)) = &mut self.dialog {
                    match i {
                        30 => s.port = (s.port + 1) % 8,
                        31 => s.baud = (s.baud + 1) % 4,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn on_dialog_confirm(&mut self) {
        match self.dialog.clone() {
            Some(Dialog::Message { .. }) | Some(Dialog::Tips(_)) | Some(Dialog::About) | Some(Dialog::Preview) => {
                self.close_dialog()
            }
            Some(Dialog::Confirm { .. }) => self.on_dialog_button(0),
            Some(Dialog::Cut(_)) => self.on_dialog_button(0),
            Some(Dialog::Foam(_)) => self.on_dialog_button(0),
            Some(Dialog::Calc(_)) => self.on_dialog_button(0),
            Some(Dialog::Settings(_)) => self.on_dialog_button(0),
            Some(Dialog::FileInfo(_)) => self.on_dialog_button(0),
            Some(Dialog::Array(_)) => self.on_dialog_button(0),
            Some(Dialog::Prompt(_)) => self.on_dialog_button(0),
            Some(Dialog::Transfer(_)) => self.on_dialog_button(0),
            Some(Dialog::Code(_)) => self.on_dialog_button(2),
            None => {}
        }
    }

    fn on_dialog_button(&mut self, id: u8) {
        let Some(dlg) = self.dialog.clone() else { return };
        match dlg {
            Dialog::Message { .. } => self.close_dialog(),
            Dialog::Confirm { action, .. } => {
                self.close_dialog();
                match action {
                    ConfirmAct::New => {
                        self.dirty = false;
                        self.new_doc();
                    }
                    ConfirmAct::Open => {
                        self.open_native();
                    }
                    ConfirmAct::Exit => {
                        platform::destroy_window(self.hwnd);
                    }
                }
            }
            Dialog::Cut(st) => match id {
                0 => self.generate_from_cut(&st),
                1 => self.close_dialog(),
                _ => {}
            },
            Dialog::Foam(st) => match id {
                0 => self.generate_foam_from_state(&st),
                1 => self.close_dialog(),
                _ => {}
            },
            Dialog::Code(_) => match id {
                0 => {
                    self.code_check();
                }
                1 => {
                    self.save_code();
                }
                2 => self.close_dialog(),
                3 => {
                    if let Some(Dialog::Code(s)) = &mut self.dialog {
                        s.font = (s.font - 1.0).clamp(10.0, 20.0);
                    }
                }
                4 => {
                    if let Some(Dialog::Code(s)) = &mut self.dialog {
                        s.font = (s.font + 1.0).clamp(10.0, 20.0);
                    }
                }
                _ => {}
            },
            Dialog::Calc(st) => match id {
                0 => {
                    let th = st.thickness.trim().parse::<f32>().unwrap_or(0.0).max(0.0);
                    let mut area = st.length * th;
                    let mut length = st.length;
                    if let Some(j) = &self.cut_job {
                        length = j.length();
                        area = length * th;
                    }
                    let hours = if self.settings.work_area > 0.0 {
                        area / self.settings.work_area
                    } else {
                        0.0
                    };
                    if let Some(Dialog::Calc(s)) = &mut self.dialog {
                        s.area = area;
                        s.hours = hours;
                        s.money = hours * self.settings.work_money;
                        s.length = length;
                    }
                }
                1 => self.close_dialog(),
                _ => {}
            },
            Dialog::Settings(st) => match id {
                0 => {
                    self.settings.work_area = st.work_area.trim().parse().unwrap_or(self.settings.work_area).max(1.0);
                    self.settings.work_money = st.work_money.trim().parse().unwrap_or(self.settings.work_money).max(0.0);
                    self.settings.moly = st.moly.trim().parse().unwrap_or(self.settings.moly).clamp(0.0, 2.0);
                    if !st.unit.trim().is_empty() {
                        self.settings.unit = st.unit.trim().to_string();
                    }
                    self.settings.show_center = st.center;
                    self.settings.show_node = st.node;
                    self.settings.show_cut = st.cut;
                    self.settings.line_numbers = st.line_no;
                    self.settings.bg = st.bg;
                    self.settings.code_kind = CodeKind::from_index(st.code_kind);
                    self.settings.save();
                    self.status = "系统设置已保存".to_string();
                    self.close_dialog();
                }
                1 => self.close_dialog(),
                2 => {
                    self.settings = Settings::default();
                    self.settings.save();
                    self.open_settings();
                    self.status = "已恢复默认设置".to_string();
                }
                _ => {}
            },
            Dialog::FileInfo(st) => match id {
                0 => {
                    self.doc.part = if st.part.trim().is_empty() {
                        "未命名".to_string()
                    } else {
                        st.part.trim().to_string()
                    };
                    self.doc.unit = st.unit.trim().to_string();
                    self.doc.programmer = st.programmer.trim().to_string();
                    self.doc.note = st.note.trim().to_string();
                    self.dirty = true;
                    self.close_dialog();
                    self.status = "图形信息已更新".to_string();
                }
                1 => self.close_dialog(),
                _ => {}
            },
            Dialog::Array(st) => match id {
                0 => self.apply_array(&st),
                1 => self.close_dialog(),
                _ => {}
            },
            Dialog::Prompt(st) => match id {
                0 => self.apply_prompt(&st),
                1 => self.close_dialog(),
                _ => {}
            },
            Dialog::Transfer(st) => match id {
                0 => {
                    if let Some(Dialog::Transfer(s)) = &mut self.dialog {
                        s.progress = 1.0;
                        let baud = [9600, 19200, 38400, 115200][st.baud.min(3)];
                        s.message = format!(
                            "模拟传送完成：{} 行，COM{} @ {} bps。\n未连接机床，实际机床上请配置联机线后重试。",
                            self.last_code.as_ref().map(|c| c.line_count).unwrap_or(0),
                            st.port + 1,
                            baud
                        );
                    }
                }
                1 => self.close_dialog(),
                _ => {}
            },
            Dialog::Tips(idx) => match id {
                0 => {
                    self.dialog = Some(Dialog::Tips((idx + 1) % TIPS.len()));
                }
                1 => self.close_dialog(),
                _ => {}
            },
            Dialog::About => self.close_dialog(),
            Dialog::Preview => match id {
                0 => self.open_message(
                    "打印",
                    "此版本未连接打印机。可以先用“图形存盘”保存图形，或使用打印预览检查排版。",
                    false,
                ),
                1 => self.close_dialog(),
                _ => {}
            },
        }
    }

    fn append_field(&mut self, c: char) {
        let Some(focus) = self.focus.clone() else { return };
        if let Some(Dialog::Code(_)) = self.dialog {
            return;
        }
        let replace = self.field_selected;
        if let Some(d) = &mut self.dialog {
            if let Some(s) = field_mut(d, &focus) {
                if replace {
                    s.clear();
                }
                if s.len() < 260 {
                    s.push(c);
                }
            }
        }
        if replace {
            self.field_selected = false;
        }
    }

    fn backspace_field(&mut self) {
        let Some(focus) = self.focus.clone() else { return };
        let replace = self.field_selected;
        if let Some(d) = &mut self.dialog {
            if let Some(s) = field_mut(d, &focus) {
                if replace {
                    s.clear();
                } else {
                    s.pop();
                }
            }
        }
        if replace {
            self.field_selected = false;
        }
    }

    fn input_command_field(&mut self) -> bool {
        self.focus.as_deref() == Some("command")
    }

    // ---------- drawing ----------

    pub fn render(&mut self) {
        let (w, h) = platform::client_size(self.hwnd);
        if w < 200 || h < 200 {
            return;
        }
        self.w = w;
        self.h = h;
        self.painter.resize(w, h);
        self.hits.clear();
        self.field_hits.clear();
        self.dialog_hits.clear();
        self.menu_tabs.clear();
        self.code_area = None;
        self.canvas = Rect::new(
            SIDE_W,
            MENU_H + TOOLBAR_H,
            w as f32 - SIDE_W * 2.0,
            (h as f32 - MENU_H - TOOLBAR_H - BOTTOM_H).max(50.0),
        );
        self.input_rect = Rect::new(
            SIDE_W,
            self.canvas.bottom(),
            w as f32 - SIDE_W * 2.0,
            BOTTOM_H - 26.0,
        );
        if !self.fitted && !self.doc.entities.is_empty() {
            self.fit_view();
            self.fitted = true;
        }
        let t0 = std::time::Instant::now();
        self.draw_canvas();
        let t1 = std::time::Instant::now();
        self.draw_chrome();
        let t2 = std::time::Instant::now();
        if self.menu_open.is_some() && self.dialog.is_none() {
            self.draw_dropdown();
        }
        if self.dialog.is_none() && self.menu_open.is_none() {
            self.draw_tooltip();
        }
        if self.dialog.is_some() {
            self.draw_dialog_ui();
        }
        let t3 = std::time::Instant::now();
        if self.profile && self.profile_frames < 4 {
            eprintln!(
                "render profile: canvas {:.1} ms, chrome {:.1} ms, overlays {:.1} ms, total {:.1} ms",
                (t1 - t0).as_secs_f64() * 1000.0,
                (t2 - t1).as_secs_f64() * 1000.0,
                (t3 - t2).as_secs_f64() * 1000.0,
                (t3 - t0).as_secs_f64() * 1000.0
            );
            self.profile_frames += 1;
        }
        if self.mouse_in_canvas {
            self.cursor = if self.tool == Tool::Pan || matches!(self.drag, Some(Drag::Pan { .. })) {
                CursorKind::Hand
            } else {
                CursorKind::Cross
            };
        } else if self.dialog.is_some() && self.focus.is_some() {
            self.cursor = CursorKind::IBeam;
        } else {
            self.cursor = CursorKind::Arrow;
        }
        let title = self.window_title();
        if title != self.title_cache {
            self.title_cache = title.clone();
            platform::set_title(self.hwnd, &title);
        }
    }

    fn canvas_palette(&self) -> (Color, Color, Color, Color, Color, Color) {
        match self.settings.bg {
            1 => (
                Color::from_hex(0x071522),
                Color::from_hex(0x0E2436),
                Color::from_hex(0x153A55),
                Color::from_hex(0xBEE7FF),
                Color::from_hex(0x4CB6E8),
                Color::from_hex(0xE9F6FF),
            ),
            2 => (
                Color::from_hex(0xF6F2E8),
                Color::from_hex(0xE7E0D0),
                Color::from_hex(0xD7CDB8),
                Color::from_hex(0x22303E),
                Color::from_hex(0x1F6FA8),
                Color::from_hex(0x2A3745),
            ),
            _ => (
                Color::from_hex(0x0B1118),
                Color::from_hex(0x0F1922),
                Color::from_hex(0x223142),
                Color::from_hex(0xD9E4F0),
                Color::from_hex(0x22D3EE),
                Color::from_hex(0xEAF2FA),
            ),
        }
    }

    fn draw_canvas(&mut self) {
        let c = self.canvas;
        let (bg, grid, grid_major, _, select, _) = self.canvas_palette();
        let cw = c.w.max(1.0) as u32;
        let ch = c.h.max(1.0) as u32;
        let scale_bits = self.scale.to_bits();
        let cx_bits = self.view_center.x.to_bits();
        let cy_bits = self.view_center.y.to_bits();
        let valid = self
            .background_cache
            .as_ref()
            .map(|cc| {
                cc.w == cw
                    && cc.h == ch
                    && cc.scale_bits == scale_bits
                    && cc.cx_bits == cx_bits
                    && cc.cy_bits == cy_bits
                    && cc.bg == self.settings.bg
                    && cc.grid == self.grid
            })
            .unwrap_or(false);
        if !valid {
            let mut pixmap = Pixmap::new(cw, ch).expect("canvas background");
            let real_canvas = self.canvas;
            let real_pixmap = std::mem::replace(&mut self.painter.pixmap, pixmap);
            self.canvas = Rect::new(0.0, 0.0, cw as f32, ch as f32);
            self.painter
                .vgradient(self.canvas, bg, bg.mix(Color::from_hex(0x000000), 0.12));
            self.draw_grid(self.canvas, grid, grid_major);
            pixmap = std::mem::replace(&mut self.painter.pixmap, real_pixmap);
            self.canvas = real_canvas;
            self.background_cache = Some(CanvasCache {
                w: cw,
                h: ch,
                scale_bits,
                cx_bits,
                cy_bits,
                bg: self.settings.bg,
                grid: self.grid,
                pixmap,
            });
        }
        if let Some(cc) = &self.background_cache {
            self.painter
                .copy_pixmap_at(c.x.round() as i32, c.y.round() as i32, &cc.pixmap);
        }

        // Static content (geometry, nodes, cut path, title block) is cached and
        // only rebuilt when the drawing or view changes. Hover/selection/mouse
        // feedback is drawn on top, so moving the crosshair costs one cached
        // blit plus a few overlays instead of re-stroking every entity.
        let signature = self.static_signature();
        let static_valid = self
            .static_cache
            .as_ref()
            .map(|sc| {
                sc.w == cw
                    && sc.h == ch
                    && sc.scale_bits == scale_bits
                    && sc.cx_bits == cx_bits
                    && sc.cy_bits == cy_bits
                    && sc.signature == signature
            })
            .unwrap_or(false);
        if !static_valid {
            if let Some(pixmap) = self.build_static_layer() {
                self.static_cache = Some(StaticCache {
                    w: cw,
                    h: ch,
                    scale_bits,
                    cx_bits,
                    cy_bits,
                    signature,
                    pixmap,
                });
            }
        }
        if let Some(sc) = &self.static_cache {
            self.painter
                .blend_pixmap_at(c.x.round() as i32, c.y.round() as i32, &sc.pixmap);
        }

        let entities = self.doc.entities.clone();
        let selected = self.selected.clone();
        let hover = self.hover;
        let mut handles: Vec<(f32, f32)> = Vec::new();
        for e in &entities {
            let is_sel = selected.contains(&e.id);
            let is_hover = hover == Some(e.id);
            if is_sel {
                let glow = select.with_alpha(38);
                self.stroke_entity(e, glow, 5.0);
                self.stroke_entity(e, select, 2.0);
                for p in entity_handles(e) {
                    let s = self.w2s(p);
                    handles.push(s);
                }
            } else if is_hover {
                self.stroke_entity(e, select.with_alpha(190), 1.45);
            }
        }
        for h in handles {
            self.painter.fill_rect(Rect::new(h.0 - 2.5, h.1 - 2.5, 5.0, 5.0), select);
            self.painter.stroke_rect(Rect::new(h.0 - 2.5, h.1 - 2.5, 5.0, 5.0), bg, 1.0);
        }
        self.draw_preview();
        self.draw_drag_box();
        self.draw_crosshair();
    }

    fn build_static_layer(&mut self) -> Option<Pixmap> {
        let c = self.canvas;
        let cw = c.w.max(1.0) as u32;
        let ch = c.h.max(1.0) as u32;
        let mut pixmap = Pixmap::new(cw, ch)?;
        let real_canvas = self.canvas;
        let real_pixmap = std::mem::replace(&mut self.painter.pixmap, pixmap);
        self.canvas = Rect::new(0.0, 0.0, cw as f32, ch as f32);
        let (_, _, _, entity, select, text) = self.canvas_palette();
        let entities = self.doc.entities.clone();
        for e in &entities {
            self.stroke_entity(e, entity, 1.45);
            if self.settings.show_center {
                if let Some(center) = entity_center(e) {
                    let s = self.w2s(center);
                    self.painter.line(s.0 - 5.0, s.1, s.0 + 5.0, s.1, text.with_alpha(150), 1.0);
                    self.painter.line(s.0, s.1 - 5.0, s.0, s.1 + 5.0, text.with_alpha(150), 1.0);
                }
            }
        }
        if self.settings.show_node {
            for p in self.doc.intersections() {
                let s = self.w2s(p);
                self.painter.line(s.0 - 4.0, s.1 - 4.0, s.0 + 4.0, s.1 + 4.0, text.with_alpha(180), 1.0);
                self.painter.line(s.0 - 4.0, s.1 + 4.0, s.0 + 4.0, s.1 - 4.0, text.with_alpha(180), 1.0);
            }
        }
        if self.settings.show_cut {
            if let Some(job) = self.cut_job.clone() {
                self.draw_cut_job(&job, select);
            }
        }
        self.draw_title_block();
        if self.doc.entities.is_empty() {
            let msg = "图板为空 · 从“绘图”菜单或工具栏开始画图";
            let w = self.painter.text_width(msg, 14.0);
            self.painter.text(
                cw as f32 * 0.5 - w * 0.5,
                ch as f32 * 0.5,
                msg,
                14.0,
                text.with_alpha(120),
            );
        }
        pixmap = std::mem::replace(&mut self.painter.pixmap, real_pixmap);
        self.canvas = real_canvas;
        Some(pixmap)
    }

    fn static_signature(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut h = DefaultHasher::new();
        self.doc.title.hash(&mut h);
        self.doc.part.hash(&mut h);
        self.doc.unit.hash(&mut h);
        self.doc.programmer.hash(&mut h);
        self.doc.note.hash(&mut h);
        self.doc.entities.len().hash(&mut h);
        for e in &self.doc.entities {
            e.id.hash(&mut h);
            match &e.kind {
                Kind::Point { p } => {
                    0u8.hash(&mut h);
                    p.x.to_bits().hash(&mut h);
                    p.y.to_bits().hash(&mut h);
                }
                Kind::Line { a, b } => {
                    1u8.hash(&mut h);
                    a.x.to_bits().hash(&mut h);
                    a.y.to_bits().hash(&mut h);
                    b.x.to_bits().hash(&mut h);
                    b.y.to_bits().hash(&mut h);
                }
                Kind::Circle { c, r } => {
                    2u8.hash(&mut h);
                    c.x.to_bits().hash(&mut h);
                    c.y.to_bits().hash(&mut h);
                    r.to_bits().hash(&mut h);
                }
                Kind::Arc { c, r, a0, sweep } => {
                    3u8.hash(&mut h);
                    c.x.to_bits().hash(&mut h);
                    c.y.to_bits().hash(&mut h);
                    r.to_bits().hash(&mut h);
                    a0.to_bits().hash(&mut h);
                    sweep.to_bits().hash(&mut h);
                }
            }
        }
        self.settings.show_center.hash(&mut h);
        self.settings.show_node.hash(&mut h);
        self.settings.show_cut.hash(&mut h);
        self.settings.bg.hash(&mut h);
        if let Some(job) = &self.cut_job {
            job.paths.len().hash(&mut h);
            for p in &job.paths {
                p.segs.len().hash(&mut h);
                p.closed.hash(&mut h);
                p.pierce.x.to_bits().hash(&mut h);
                p.pierce.y.to_bits().hash(&mut h);
                if let Some(lead) = &p.lead {
                    lead.start().x.to_bits().hash(&mut h);
                    lead.start().y.to_bits().hash(&mut h);
                    lead.end().x.to_bits().hash(&mut h);
                    lead.end().y.to_bits().hash(&mut h);
                }
                for seg in &p.segs {
                    match seg {
                        crate::codegen::Seg::Line { a, b } => {
                            a.x.to_bits().hash(&mut h);
                            b.x.to_bits().hash(&mut h);
                            a.y.to_bits().hash(&mut h);
                            b.y.to_bits().hash(&mut h);
                        }
                        crate::codegen::Seg::Arc { c, r, a0, sweep } => {
                            c.x.to_bits().hash(&mut h);
                            c.y.to_bits().hash(&mut h);
                            r.to_bits().hash(&mut h);
                            a0.to_bits().hash(&mut h);
                            sweep.to_bits().hash(&mut h);
                        }
                    }
                }
            }
        }
        h.finish()
    }

    fn draw_grid(&mut self, c: Rect, grid: Color, major: Color) {
        let step = self.grid_step();
        let world_min = self.screen_to_world(c.x, c.bottom());
        let world_max = self.screen_to_world(c.right(), c.y);
        let x0 = (world_min.x / step).floor() as i64;
        let x1 = (world_max.x / step).ceil() as i64;
        let y0 = (world_min.y / step).floor() as i64;
        let y1 = (world_max.y / step).ceil() as i64;
        if (x1 - x0) * (y1 - y0) > 40000 {
            return;
        }
        for i in x0..=x1 {
            let wx = i as f32 * step;
            let sx = self.w2s(Pt::new(wx, 0.0)).0;
            if sx < c.x - 1.0 || sx > c.right() + 1.0 {
                continue;
            }
            let col = if wx.abs() < 0.001 { C_AXIS_Y } else if i % 5 == 0 { major } else { grid };
            let width = if wx.abs() < 0.001 { 2.0 } else { 1.0 };
            self.painter
                .fill_rect(Rect::new(sx.round(), c.y, width, c.h), col);
        }
        for i in y0..=y1 {
            let wy = i as f32 * step;
            let sy = self.w2s(Pt::new(0.0, wy)).1;
            if sy < c.y - 1.0 || sy > c.bottom() + 1.0 {
                continue;
            }
            let col = if wy.abs() < 0.001 { C_AXIS_X } else if i % 5 == 0 { major } else { grid };
            let width = if wy.abs() < 0.001 { 2.0 } else { 1.0 };
            self.painter
                .fill_rect(Rect::new(c.x, sy.round(), c.w, width), col);
        }
        let origin = self.w2s(Pt::new(0.0, 0.0));
        if c.contains(origin.0, origin.1) {
            self.painter.fill_circle(origin.0, origin.1, 2.4, C_PIERCE.with_alpha(180));
        }
        // Axis colours already distinguish the two axes; keep the board clean.
    }

    fn stroke_entity(&mut self, e: &crate::model::Entity, color: Color, width: f32) {
        match &e.kind {
            Kind::Point { p } => {
                let s = self.w2s(*p);
                self.painter.line(s.0 - 5.0, s.1, s.0 + 5.0, s.1, color, width);
                self.painter.line(s.0, s.1 - 5.0, s.0, s.1 + 5.0, color, width);
                self.painter.stroke_circle(s.0, s.1, 2.2, color, width);
            }
            Kind::Line { a, b } => {
                let sa = self.w2s(*a);
                let sb = self.w2s(*b);
                self.painter.line(sa.0, sa.1, sb.0, sb.1, color, width);
            }
            Kind::Circle { c, r } => {
                let s = self.w2s(*c);
                self.painter.stroke_circle(s.0, s.1, r * self.scale, color, width);
            }
            Kind::Arc { c, r, a0, sweep } => {
                let s = self.w2s(*c);
                self.painter
                    .stroke_arc(s.0, s.1, r * self.scale, -*a0, -*sweep, color, width);
            }
        }
    }

    fn draw_cut_job(&mut self, job: &CutJob, select: Color) {
        for path in &job.paths {
            let pts: Vec<Pt> = path.sampled();
            let screen: Vec<(f32, f32)> = pts.iter().map(|p| self.w2s(*p)).collect();
            if screen.len() > 1 {
                self.painter
                    .stroke_polyline_dashed(&screen, C_CUT.with_alpha(225), 1.35, 5.0);
            }
            // Direction arrows.
            for seg in &path.segs {
                let mid = match seg {
                    crate::codegen::Seg::Line { a, b } => a.lerp(*b, 0.5),
                    crate::codegen::Seg::Arc { c, r, a0, sweep } => {
                        c.add(Pt::from_angle(*a0 + *sweep * 0.5, *r))
                    }
                };
                let eps = 0.8 / self.scale;
                let ahead = match seg {
                    crate::codegen::Seg::Line { a, b } => a.lerp(*b, 0.56),
                    crate::codegen::Seg::Arc { c, r, a0, sweep } => {
                        c.add(Pt::from_angle(*a0 + *sweep * 0.5 + if *sweep >= 0.0 { eps / r } else { -eps / r }, *r))
                    }
                };
                let m = self.w2s(mid);
                let a = self.w2s(ahead);
                let d = ((a.0 - m.0).powi(2) + (a.1 - m.1).powi(2)).sqrt();
                if d > 0.01 {
                    let ux = (a.0 - m.0) / d;
                    let uy = (a.1 - m.1) / d;
                    let px = -uy;
                    let py = ux;
                    let size = 5.0;
                    self.painter.fill_poly(
                        &[
                            (m.0 + ux * size, m.1 + uy * size),
                            (m.0 - ux * 2.0 + px * size * 0.6, m.1 - uy * 2.0 + py * size * 0.6),
                            (m.0 - ux * 2.0 - px * size * 0.6, m.1 - uy * 2.0 - py * size * 0.6),
                        ],
                        C_CUT,
                    );
                }
            }
            let s = self.w2s(path.pierce);
            self.painter.fill_circle(s.0, s.1, 3.0, C_PIERCE);
            self.painter.stroke_circle(s.0, s.1, 7.0, C_PIERCE.with_alpha(180), 1.2);
            self.painter.line(s.0 - 9.0, s.1, s.0 + 9.0, s.1, C_PIERCE.with_alpha(120), 1.0);
            self.painter.line(s.0, s.1 - 9.0, s.0, s.1 + 9.0, C_PIERCE.with_alpha(120), 1.0);
            if let Some(lead) = &path.lead {
                let a = self.w2s(lead.start());
                let b = self.w2s(lead.end());
                self.painter.dashed_line(a.0, a.1, b.0, b.1, C_PIERCE.with_alpha(200), 1.2, 4.0);
            }
            let _ = select;
        }
    }

    fn draw_drag_box(&mut self) {
        match &self.drag {
            Some(Drag::BoxSel { start, current }) => {
                let r = Rect::new(
                    start.0.min(current.0),
                    start.1.min(current.1),
                    (current.0 - start.0).abs(),
                    (current.1 - start.1).abs(),
                );
                self.painter.fill_rect(r, C_SELECT.with_alpha(18));
                self.painter.stroke_rect(r, C_SELECT.with_alpha(220), 1.0);
            }
            Some(Drag::ZoomBox { start, current }) => {
                let r = Rect::new(
                    start.0.min(current.0),
                    start.1.min(current.1),
                    (current.0 - start.0).abs(),
                    (current.1 - start.1).abs(),
                );
                self.painter.stroke_rect(r, C_PIERCE.with_alpha(230), 1.2);
            }
            _ => {}
        }
    }

    fn draw_preview(&mut self) {
        if !self.mouse_in_canvas || self.pts.is_empty() {
            return;
        }
        let Some((cur, snapped)) = self.snap else { return };
        let pts = self.pts.clone();
        let color = C_SELECT.with_alpha(200);
        let dash = C_SELECT.with_alpha(160);
        match self.tool {
            Tool::Line => {
                let a = self.w2s(pts[0]);
                let b = self.w2s(cur);
                self.painter.dashed_line(a.0, a.1, b.0, b.1, dash, 1.2, 5.0);
            }
            Tool::Rect => {
                let a = self.w2s(pts[0]);
                let b = self.w2s(cur);
                let r = Rect::new(a.0.min(b.0), a.1.min(b.1), (b.0 - a.0).abs(), (b.1 - a.1).abs());
                self.painter.stroke_rect(r, dash, 1.2);
            }
            Tool::Circle => {
                let a = self.w2s(pts[0]);
                let b = self.w2s(cur);
                let r = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
                self.painter.stroke_circle(a.0, a.1, r, dash, 1.2);
                self.painter.dashed_line(a.0, a.1, b.0, b.1, dash.with_alpha(100), 1.0, 3.0);
            }
            Tool::Arc => {
                let a = self.w2s(pts[0]);
                if pts.len() == 1 {
                    let b = self.w2s(cur);
                    let r = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
                    self.painter.stroke_circle(a.0, a.1, r, dash, 1.2);
                    self.painter.dashed_line(a.0, a.1, b.0, b.1, dash.with_alpha(100), 1.0, 3.0);
                } else {
                    let r = pts[0].dist(pts[1]);
                    let a0 = pts[1].sub(pts[0]).angle();
                    let a1 = cur.sub(pts[0]).angle();
                    let mut sweep = (a1 - a0).rem_euclid(TAU);
                    if sweep < 0.001 {
                        sweep = TAU;
                    }
                    let s = self.w2s(pts[0]);
                    self.painter
                        .stroke_arc(s.0, s.1, r * self.scale, -a0, -sweep, dash, 1.3);
                }
            }
            Tool::Polyline => {
                let mut screen: Vec<(f32, f32)> = pts.iter().map(|p| self.w2s(*p)).collect();
                screen.push(self.w2s(cur));
                self.painter.stroke_polyline(&screen, color, 1.4, false);
                for s in screen.iter().take(screen.len() - 1) {
                    self.painter.fill_rect(Rect::new(s.0 - 2.0, s.1 - 2.0, 4.0, 4.0), color);
                }
            }
            Tool::Copy | Tool::Mirror | Tool::Rotate => {
                let a = self.w2s(pts[0]);
                let b = self.w2s(cur);
                self.painter.dashed_line(a.0, a.1, b.0, b.1, dash, 1.2, 5.0);
                self.painter.fill_circle(a.0, a.1, 3.0, color);
            }
            Tool::Measure => {
                let a = self.w2s(pts[0]);
                let b = self.w2s(cur);
                self.painter.dashed_line(a.0, a.1, b.0, b.1, dash, 1.2, 5.0);
                let d = pts[0].dist(cur);
                let txt = format!("{:.2} mm", d);
                self.painter.fill_round_rect(
                    Rect::new((a.0 + b.0) * 0.5 + 8.0, (a.1 + b.1) * 0.5 - 20.0, 76.0, 20.0),
                    4.0,
                    Color::rgba(8, 16, 24, 220),
                );
                self.painter.text_top(
                    (a.0 + b.0) * 0.5 + 14.0,
                    (a.1 + b.1) * 0.5 - 17.0,
                    &txt,
                    11.0,
                    C_WHITE,
                );
            }
            _ => {}
        }
        if snapped {
            let s = self.w2s(cur);
            self.painter.stroke_circle(s.0, s.1, 5.0, C_PIERCE.with_alpha(220), 1.2);
        }
    }

    fn draw_crosshair(&mut self) {
        if !self.mouse_in_canvas || self.dialog.is_some() || self.menu_open.is_some() {
            return;
        }
        let (mx, my) = self.mouse;
        let c = self.canvas;
        if !c.contains(mx, my) {
            return;
        }
        let col = Color::rgba(120, 170, 210, 55);
        self.painter.dashed_line(c.x, my, c.right(), my, col, 1.0, 4.0);
        self.painter.dashed_line(mx, c.y, mx, c.bottom(), col, 1.0, 4.0);
        if let Some((p, snapped)) = self.snap {
            let s = self.w2s(p);
            if snapped {
                self.painter.stroke_rect(Rect::new(s.0 - 4.0, s.1 - 4.0, 8.0, 8.0), C_PIERCE, 1.2);
            }
            let txt = format!("{:.3}, {:.3}", p.x, p.y);
            let tw = self.painter.text_width(&txt, 11.0);
            let mut x = mx + 14.0;
            let mut y = my + 16.0;
            if x + tw + 14.0 > c.right() {
                x = mx - tw - 24.0;
            }
            if y + 20.0 > c.bottom() {
                y = my - 26.0;
            }
            self.painter.fill_round_rect(
                Rect::new(x, y, tw + 12.0, 19.0),
                4.0,
                Color::rgba(8, 16, 24, 210),
            );
            self.painter
                .text_top(x + 6.0, y + 2.0, &txt, 11.0, Color::from_hex(0xBFD8EF));
        }
    }

    fn draw_title_block(&mut self) {
        if self.doc.entities.is_empty() {
            return;
        }
        let c = self.canvas;
        let (_, _, _, _, _, text) = self.canvas_palette();
        let part = if self.doc.part.is_empty() { "未命名" } else { &self.doc.part };
        let len = self.doc.total_length();
        let cut = self.cut_job.as_ref().map(|j| j.length()).unwrap_or(0.0);
        let mut info = format!(
            "{}   ·   单位 {}   ·   图元 {}   ·   周长 {:.2} mm",
            truncate_str(part, 16),
            self.doc.unit,
            self.doc.entities.len(),
            len
        );
        if cut > 0.0 {
            info.push_str(&format!("   ·   切割 {:.2} mm", cut));
        }
        let tw = self.painter.text_width(&info, 11.0);
        let r = Rect::new(c.x + 12.0, c.y + 10.0, tw + 20.0, 22.0);
        self.painter.fill_rect(r, Color::rgba(6, 12, 18, 175));
        self.painter.stroke_rect(r, text.with_alpha(35), 1.0);
        self.painter.text_top(r.x + 10.0, r.y + 4.0, &info, 11.0, text.with_alpha(215));
    }

    fn draw_chrome(&mut self) {
        let w = self.w as f32;
        let h = self.h as f32;
        let tc0 = std::time::Instant::now();
        // Menu bar.
        self.painter.vgradient(Rect::new(0.0, 0.0, w, MENU_H), C_MENU, Color::from_hex(0xEDF2F7));
        self.painter.line(0.0, MENU_H - 0.5, w, MENU_H - 0.5, C_BORDER, 1.0);
        let mut x = 8.0;
        for (i, m) in self.menus.iter().enumerate() {
            let tw = self.painter.text_width(m.label, 14.0) + 24.0;
            let r = Rect::new(x, 0.0, tw, MENU_H);
            let hovered = r.contains(self.mouse.0, self.mouse.1);
            if self.menu_open == Some(i) {
                self.painter.fill_rect(r, C_ACCENT_SOFT);
                self.painter.line(r.x + 6.0, r.bottom() - 2.0, r.right() - 6.0, r.bottom() - 2.0, C_ACCENT, 2.0);
            } else if hovered {
                self.painter.fill_rect(r, Color::from_hex(0xEDF4FB));
            }
            self.painter.text_centered(r, m.label, 14.0, C_TEXT);
            self.hits.push(Hit { r, act: Act::OpenMenu(i), tip: String::new() });
            self.menu_tabs.push((r, i));
            x += tw;
        }
        // Small title mark at the far right.
        let badge_w = 178.0;
        let br = Rect::new(w - badge_w - 12.0, 4.0, badge_w, 18.0);
        self.painter.fill_round_rect(br, 9.0, Color::from_hex(0xE6EEF7));
        self.painter.text_top(
            br.x + 26.0,
            br.y + 1.0,
            "超越线切割 · WEDM",
            11.0,
            C_TEXT_DIM,
        );
        draw_icon(&mut self.painter, Icon::Gear, Rect::new(br.x + 5.0, br.y + 1.0, 16.0, 16.0), C_ACCENT);
        let tc1 = std::time::Instant::now();
        // Toolbar.
        let tb = Rect::new(0.0, MENU_H, w, TOOLBAR_H);
        self.painter.vgradient(tb, C_TOOLBAR_TOP, C_TOOLBAR_BOTTOM);
        self.painter.line(0.0, tb.bottom() - 0.5, w, tb.bottom() - 0.5, C_BORDER, 1.0);
        self.draw_toolbar(tb);
        let tc2 = std::time::Instant::now();
        // Side strips.
        let side_top = MENU_H + TOOLBAR_H;
        let side_h = self.canvas.h;
        let left = Rect::new(0.0, side_top, SIDE_W, side_h);
        let right = Rect::new(w - SIDE_W, side_top, SIDE_W, side_h);
        self.painter.vgradient(left, Color::from_hex(0xEFF4F9), Color::from_hex(0xE3EAF2));
        self.painter.vgradient(right, Color::from_hex(0xEFF4F9), Color::from_hex(0xE3EAF2));
        self.painter.line(SIDE_W - 0.5, left.y, SIDE_W - 0.5, left.bottom(), C_BORDER, 1.0);
        self.painter.line(right.x + 0.5, right.y, right.x + 0.5, right.bottom(), C_BORDER, 1.0);
        self.draw_left_strip(left);
        self.draw_right_strip(right);
        let tc3 = std::time::Instant::now();
        // Input bar.
        let ib = self.input_rect;
        self.painter.fill_rect(ib, Color::from_hex(0xF4F7FA));
        self.painter.line(ib.x, ib.y, w - SIDE_W, ib.y, C_BORDER, 1.0);
        self.painter.text_top_bold(ib.x + 10.0, ib.y + 5.0, "命令:", 13.0, C_ACCENT_DARK);
        let px = ib.x + 52.0;
        let field = Rect::new((w - SIDE_W - 380.0).max(px + 300.0), ib.y + 3.0, 370.0, 20.0);
        let prompt = self.prompt();
        let prompt = truncate_to_width(&prompt, (field.x - px - 12.0).max(80.0), 12.5);
        self.painter.text_top(px, ib.y + 6.0, &prompt, 12.5, C_TEXT_DIM);
        let focused = self.input_command_field();
        self.painter.fill_rect(field, C_WHITE);
        self.painter.stroke_rect(field, if focused { C_ACCENT } else { C_BORDER }, if focused { 1.5 } else { 1.0 });
        if self.command.is_empty() {
            self.painter
                .text_top(field.x + 7.0, field.y + 3.0, "输入坐标，如 20,15", 11.5, C_TEXT_LIGHT);
        } else {
            let txt = truncate_to_width(&self.command, field.w - 12.0, 12.0);
            self.painter.text_top(field.x + 7.0, field.y + 3.0, &txt, 12.0, C_TEXT);
        }
        if focused {
            let caret_x = field.x + 7.0 + self.painter.text_width(&self.command, 12.0).min(field.w - 16.0);
            self.painter.line(caret_x, field.y + 4.0, caret_x, field.bottom() - 4.0, C_TEXT, 1.0);
        }
        self.field_hits.push(FieldHit { r: field, id: "command".to_string() });
        let tc4 = std::time::Instant::now();
        // Status bar.
        self.draw_status_bar(Rect::new(0.0, h - 26.0, w, 26.0));
        if self.profile && self.profile_frames < 4 {
            let tc5 = std::time::Instant::now();
            eprintln!(
                "chrome: menu {:.1}, toolbar {:.1}, sides {:.1}, input {:.1}, status {:.1}",
                (tc1 - tc0).as_secs_f64() * 1000.0,
                (tc2 - tc1).as_secs_f64() * 1000.0,
                (tc3 - tc2).as_secs_f64() * 1000.0,
                (tc4 - tc3).as_secs_f64() * 1000.0,
                (tc5 - tc4).as_secs_f64() * 1000.0
            );
        }
    }

    fn draw_toolbar(&mut self, tb: Rect) {
        let mut x = 8.0;
        let y = tb.y + 5.0;
        let standard = [
            TBarItem::Btn(Icon::New, Act::New, "新建图形  Ctrl+N"),
            TBarItem::Btn(Icon::Open, Act::Open, "打开图形  Ctrl+O"),
            TBarItem::Btn(Icon::Save, Act::Save, "图形存盘  Ctrl+S"),
            TBarItem::Sep,
            TBarItem::Btn(Icon::Preview, Act::Preview, "打印预览"),
            TBarItem::Btn(Icon::Info, Act::FileInfo, "图形辅助信息"),
            TBarItem::Btn(Icon::Calc, Act::CalcArea, "计算切割面积与金额"),
            TBarItem::Btn(Icon::Code, Act::FoamProgram, "泡沫切割 / Mach3  F5"),
            TBarItem::Sep,
            TBarItem::Btn(Icon::Undo, Act::Undo, "撤销  Ctrl+Z"),
            TBarItem::Btn(Icon::Redo, Act::Redo, "重做  Ctrl+Y"),
            TBarItem::Sep,
            TBarItem::Btn(Icon::Redraw, Act::Redraw, "重画"),
            TBarItem::Btn(Icon::Pan, Act::Pan, "实时平移"),
            TBarItem::Btn(Icon::ZoomWindow, Act::SetTool(Tool::ZoomWindow), "窗口缩放"),
            TBarItem::Btn(Icon::Fit, Act::FitAll, "显示全部"),
        ];
        self.toolbar_items(&standard, &mut x, y);
        x += 18.0;
        let draw = [
            TBarItem::Btn(Icon::Point, Act::SetTool(Tool::Point), "绘制点"),
            TBarItem::Btn(Icon::Line, Act::SetTool(Tool::Line), "绘制直线"),
            TBarItem::Btn(Icon::Polyline, Act::SetTool(Tool::Polyline), "绘制多段线"),
            TBarItem::Btn(Icon::Rect, Act::SetTool(Tool::Rect), "绘制矩形"),
            TBarItem::Btn(Icon::Circle, Act::SetTool(Tool::Circle), "绘制圆"),
            TBarItem::Btn(Icon::Arc, Act::SetTool(Tool::Arc), "绘制圆弧"),
            TBarItem::Sep,
            TBarItem::Btn(Icon::Trim, Act::SetTool(Tool::Trim), "修剪直线"),
            TBarItem::Btn(Icon::Select, Act::SetTool(Tool::Select), "选择 / 移动"),
        ];
        self.toolbar_items(&draw, &mut x, y);
        // Quick zoom buttons on the right.
        let mut bx = tb.right() - 8.0;
        for (icon, act, tip) in [
            (Icon::Fit, Act::FitAll, "显示全部"),
            (Icon::Zoom, Act::ZoomIn, "放大"),
            (Icon::Zoom, Act::ZoomOut, "缩小"),
        ] {
            bx -= 30.0;
            let r = Rect::new(bx, y, 30.0, 30.0);
            let hover = r.contains(self.mouse.0, self.mouse.1);
            if hover {
                self.painter.fill_round_rect(r, 4.0, Color::from_hex(0xDCE9F6));
            }
            let col = if act == Act::ZoomOut { Color::from_hex(0x5A7086) } else { Color::from_hex(0x29465F) };
            draw_icon(&mut self.painter, icon, r.inset(7.0), col);
            self.hits.push(Hit { r, act, tip: tip.to_string() });
            bx -= 3.0;
        }
    }

    fn toolbar_items(&mut self, items: &[TBarItem], x: &mut f32, y: f32) {
        for item in items {
            match item {
                TBarItem::Sep => {
                    *x += 9.0;
                    self.painter.line(*x, y + 4.0, *x, y + 26.0, C_BORDER, 1.0);
                    *x += 9.0;
                }
                TBarItem::Btn(icon, act, tip) => {
                    let r = Rect::new(*x, y, 30.0, 30.0);
                    let hover = r.contains(self.mouse.0, self.mouse.1);
                    let active = match act {
                        Act::SetTool(t) => *t == self.tool,
                        Act::Pan => self.tool == Tool::Pan,
                        _ => false,
                    };
                    if active {
                        self.painter.fill_round_rect(r, 4.0, C_ACCENT_SOFT);
                        self.painter.stroke_round_rect(r, 4.0, C_ACCENT.with_alpha(120), 1.0);
                    } else if hover {
                        self.painter.fill_round_rect(r, 4.0, Color::from_hex(0xDCE9F6));
                        self.painter.stroke_round_rect(r, 4.0, C_BORDER, 1.0);
                    }
                    draw_icon(
                        &mut self.painter,
                        *icon,
                        r.inset(6.0),
                        if active { C_ACCENT_DARK } else { Color::from_hex(0x29465F) },
                    );
                    self.hits.push(Hit { r, act: *act, tip: (*tip).to_string() });
                    *x += 32.0;
                }
            }
        }
    }

    fn draw_left_strip(&mut self, strip: Rect) {
        let mut y = strip.y + 8.0;
        let tools = [
            (Icon::Select, Act::SetTool(Tool::Select), "选择 / 移动"),
            (Icon::Delete, Act::DeleteSel, "删除选中的图元"),
            (Icon::Copy, Act::SetTool(Tool::Copy), "复制图元"),
            (Icon::Mirror, Act::SetTool(Tool::Mirror), "镜像图元"),
            (Icon::Offset, Act::EditOffset, "偏移图元"),
            (Icon::Array, Act::EditArray, "阵列图元"),
            (Icon::Move, Act::SetTool(Tool::Move), "移动：拖动选中的图元"),
            (Icon::Rotate, Act::SetTool(Tool::Rotate), "旋转图元"),
            (Icon::Scale, Act::EditScale, "缩放图元"),
            (Icon::Trim, Act::SetTool(Tool::Trim), "修剪直线"),
        ];
        for (i, (icon, act, tip)) in tools.iter().enumerate() {
            let r = Rect::new(strip.x + 4.0, y, 30.0, 30.0);
            let hover = r.contains(self.mouse.0, self.mouse.1);
            let active = match act {
                Act::SetTool(t) => *t == self.tool,
                _ => false,
            };
            if active {
                self.painter.fill_round_rect(r, 4.0, C_ACCENT_SOFT);
                self.painter.stroke_rect(r, C_ACCENT.with_alpha(110), 1.0);
            } else if hover {
                self.painter.fill_round_rect(r, 4.0, Color::from_hex(0xDCE9F6));
            }
            draw_icon(&mut self.painter, *icon, r.inset(6.0), if active { C_ACCENT_DARK } else { Color::from_hex(0x29465F) });
            self.hits.push(Hit { r, act: *act, tip: tip.to_string() });
            y += 32.0;
            if i == 5 {
                self.painter.line(strip.x + 6.0, y - 3.0, strip.right() - 6.0, y - 3.0, C_BORDER, 1.0);
            }
        }
        // Offset, array and scale need a click action that opens a dialog / prompt.
        let _ = (0..0);
    }

    fn draw_right_strip(&mut self, strip: Rect) {
        let mut y = strip.y + 8.0;
        let modes = [
            (1u8, Icon::SnapEnd, "端点捕捉"),
            (2, Icon::SnapMid, "中点捕捉"),
            (4, Icon::SnapCenter, "圆心捕捉"),
            (8, Icon::SnapNode, "交点捕捉"),
            (16, Icon::SnapNear, "最近点捕捉"),
        ];
        for (bit, icon, tip) in modes {
            let r = Rect::new(strip.x + 4.0, y, 30.0, 30.0);
            let on = self.snap_modes & bit != 0;
            let hover = r.contains(self.mouse.0, self.mouse.1);
            if on {
                self.painter.fill_round_rect(r, 4.0, C_ACCENT_SOFT);
                self.painter.stroke_rect(r, C_ACCENT.with_alpha(110), 1.0);
            } else if hover {
                self.painter.fill_round_rect(r, 4.0, Color::from_hex(0xDCE9F6));
            }
            draw_icon(&mut self.painter, icon, r.inset(5.0), if on { C_ACCENT_DARK } else { Color::from_hex(0x7C8B9B) });
            self.hits.push(Hit { r, act: Act::ToggleSnap(bit), tip: tip.to_string() });
            y += 32.0;
        }
        self.painter.line(strip.x + 6.0, y - 3.0, strip.right() - 6.0, y - 3.0, C_BORDER, 1.0);
        y += 4.0;
        for (icon, act, tip) in [
            (Icon::Snap, Act::SnapAll, "全部捕捉"),
            (Icon::Close, Act::SnapNone, "关闭捕捉"),
            (Icon::Gear, Act::Settings, "捕捉 / 系统设置"),
        ] {
            let r = Rect::new(strip.x + 4.0, y, 30.0, 30.0);
            let hover = r.contains(self.mouse.0, self.mouse.1);
            if hover {
                self.painter.fill_round_rect(r, 4.0, Color::from_hex(0xDCE9F6));
            }
            draw_icon(&mut self.painter, icon, r.inset(6.0), Color::from_hex(0x29465F));
            self.hits.push(Hit { r, act, tip: tip.to_string() });
            y += 32.0;
        }
    }

    fn draw_status_bar(&mut self, sb: Rect) {
        self.painter.vgradient(sb, Color::from_hex(0xE4EBF3), Color::from_hex(0xDAE3EC));
        self.painter.line(sb.x, sb.y + 0.5, sb.right(), sb.y + 0.5, C_BORDER, 1.0);
        let mut x = 8.0;
        let p = self.snap.map(|s| s.0).unwrap_or_else(|| self.current_point());
        let coord = format!("X {:.3}   Y {:.3}", p.x, p.y);
        self.painter.text_top(x, sb.y + 6.0, &coord, 11.5, C_TEXT);
        x += 190.0;
        self.status_sep(sb, x);
        x += 8.0;
        let ratio = self.scale / 4.0;
        self.painter
            .text_top(x, sb.y + 6.0, &format!("视图 {:.2}x", ratio), 11.5, C_TEXT_DIM);
        x += 92.0;
        self.status_sep(sb, x);
        x += 8.0;
        for (label, act, on, width) in [
            ("正交", Act::ToggleOrtho, self.ortho, 62.0),
            ("极轴", Act::TogglePolar, self.polar, 62.0),
            ("对象捕捉", Act::ToggleOsnap, self.osnap, 84.0),
            ("网格", Act::ToggleGrid, self.grid, 62.0),
        ] {
            let r = Rect::new(x, sb.y + 3.0, width, 20.0);
            if on {
                self.painter.fill_rect(r, Color::from_hex(0xC8DCF0));
                self.painter.stroke_rect(r, C_ACCENT.with_alpha(80), 1.0);
                self.painter.text_centered(r, label, 11.5, C_ACCENT_DARK);
            } else {
                self.painter.text_centered(r, label, 11.5, C_TEXT_DIM);
            }
            let hov = r.contains(self.mouse.0, self.mouse.1);
            if hov {
                self.painter.stroke_rect(r, C_ACCENT.with_alpha(120), 1.0);
            }
            self.hits.push(Hit { r, act, tip: format!("{}（F8/F9 等可切换）", label) });
            x += width + 2.0;
        }
        self.status_sep(sb, x + 2.0);
        x += 10.0;
        let msg = truncate_str(&self.status, ((sb.right() - x - 10.0) / 7.0) as usize);
        self.painter.text_top(x, sb.y + 6.0, &msg, 11.5, C_TEXT_DIM);
    }

    fn status_sep(&mut self, sb: Rect, x: f32) {
        self.painter.line(x, sb.y + 5.0, x, sb.bottom() - 5.0, C_BORDER, 1.0);
    }

    fn draw_tooltip(&mut self) {
        let Some(hit) = self
            .hits
            .iter()
            .rev()
            .find(|h| !h.tip.is_empty() && h.r.contains(self.mouse.0, self.mouse.1))
            .cloned()
        else {
            return;
        };
        let tw = self.painter.text_width(&hit.tip, 11.5) + 16.0;
        let mut x = self.mouse.0 + 10.0;
        let mut y = self.mouse.1 + 26.0;
        if x + tw > self.w as f32 - 8.0 {
            x = self.w as f32 - tw - 8.0;
        }
        if y + 24.0 > self.h as f32 - 8.0 {
            y = self.mouse.1 - 30.0;
        }
        self.painter.fill_round_rect(Rect::new(x, y, tw, 23.0), 4.0, Color::from_hex(0x25313D));
        self.painter.text_top(x + 8.0, y + 4.0, &hit.tip, 11.5, C_WHITE);
    }

    fn draw_dropdown(&mut self) {
        let Some(idx) = self.menu_open else { return };
        let Some(menu) = self.menus.get(idx) else { return };
        let Some((tab, _)) = self.menu_tabs.get(idx) else { return };
        let item_h = 27.0;
        let sep_h = 8.0;
        let mut width = 190.0f32;
        for it in &menu.items {
            if let MenuItem::Action { label, shortcut, .. } = it {
                let w = self.painter.text_width(label, 13.0)
                    + if shortcut.is_empty() { 60.0 } else { 128.0 };
                width = width.max(w);
            }
        }
        width = width.min(320.0);
        let mut height = 8.0;
        for it in &menu.items {
            height += match it {
                MenuItem::Sep => sep_h,
                _ => item_h,
            };
        }
        let mut x = tab.x;
        if x + width > self.w as f32 - 4.0 {
            x = self.w as f32 - width - 4.0;
        }
        let mut y = MENU_H;
        if y + height > self.h as f32 - 4.0 {
            y = self.h as f32 - height - 4.0;
        }
        let panel = Rect::new(x, y, width, height);
        self.painter.fill_rect(panel.offset(4.0, 5.0), Color::rgba(10, 18, 28, 60));
        self.painter.fill_rect(panel, C_WHITE);
        self.painter.stroke_rect(panel, C_BORDER, 1.0);
        let mut iy = panel.y + 4.0;
        for it in &menu.items {
            match it {
                MenuItem::Sep => {
                    self.painter.line(panel.x + 10.0, iy + 3.0, panel.right() - 10.0, iy + 3.0, C_BORDER_SOFT, 1.0);
                    iy += sep_h;
                }
                MenuItem::Action { label, act, shortcut, check } => {
                    let r = Rect::new(panel.x + 3.0, iy, panel.w - 6.0, item_h);
                    let hovered = r.contains(self.mouse.0, self.mouse.1);
                    if hovered {
                        self.painter.fill_rect(r, C_ACCENT_SOFT);
                        self.painter.stroke_rect(r, C_ACCENT.with_alpha(90), 1.0);
                    }
                    let checked = match check {
                        Check::Center => self.settings.show_center,
                        Check::Node => self.settings.show_node,
                        Check::Cut => self.settings.show_cut,
                        Check::Ortho => self.ortho,
                        Check::Polar => self.polar,
                        Check::Osnap => self.osnap,
                        Check::Grid => self.grid,
                        Check::None => false,
                    };
                    if checked {
                        draw_icon(&mut self.painter, Icon::Check, Rect::new(r.x + 7.0, iy + 7.0, 13.0, 13.0), C_ACCENT);
                    }
                    self.painter.text_top(r.x + 28.0, iy + 5.0, label, 13.0, C_TEXT);
                    if !shortcut.is_empty() {
                        self.painter.text_top_right(r.right() - 12.0, iy + 6.0, shortcut, 11.0, C_TEXT_LIGHT);
                    }
                    self.hits.push(Hit { r, act: *act, tip: String::new() });
                    iy += item_h;
                }
            }
        }
    }
}

// ---------- dialog painting ----------

impl App {
    fn draw_dialog_ui(&mut self) {
        let Some(dlg) = self.dialog.clone() else { return };
        let full = Rect::new(0.0, 0.0, self.w as f32, self.h as f32);
        self.painter.fill_rect(full, Color::rgba(8, 14, 22, 120));
        let (pw, ph) = dialog_size(&dlg);
        let panel = Rect::new(
            ((self.w as f32 - pw) * 0.5).max(10.0),
            ((self.h as f32 - ph) * 0.5 - 8.0).max(10.0),
            pw,
            ph,
        );
        self.painter.fill_round_rect(panel.offset(7.0, 9.0), 7.0, Color::rgba(0, 0, 0, 90));
        self.painter.fill_round_rect(panel, 7.0, Color::from_hex(0xF4F6F9));
        self.painter.stroke_rect(panel, Color::from_hex(0xAAB8C7), 1.0);
        let title = Rect::new(panel.x, panel.y, panel.w, 36.0);
        self.painter.fill_round_rect(title, 7.0, Color::from_hex(0x2A5D8C));
        self.painter.fill_rect(Rect::new(title.x, title.y + 24.0, title.w, 12.0), Color::from_hex(0x2A5D8C));
        self.painter.text_top_bold(title.x + 14.0, title.y + 9.0, dialog_title(&dlg), 14.5, C_WHITE);
        let close = Rect::new(title.right() - 32.0, title.y + 7.0, 22.0, 22.0);
        if close.contains(self.mouse.0, self.mouse.1) {
            self.painter.fill_round_rect(close, 3.0, Color::rgba(255, 255, 255, 40));
        }
        draw_icon(&mut self.painter, Icon::Close, close.inset(5.0), C_WHITE);
        self.dialog_hits.push((close, Act::DlgClose));
        let body = Rect::new(panel.x + 18.0, panel.y + 48.0, panel.w - 36.0, panel.h - 48.0 - 56.0);
        let footer = Rect::new(panel.x + 18.0, panel.bottom() - 50.0, panel.w - 36.0, 36.0);
        match &dlg {
            Dialog::Message { text, error, .. } => {
                if *error {
                    self.painter.fill_circle(body.x + 16.0, body.y + 22.0, 11.0, Color::from_hex(0xD9534F));
                    self.painter.text_centered(Rect::new(body.x + 6.0, body.y + 12.0, 20.0, 20.0), "!", 14.0, C_WHITE);
                } else {
                    self.painter.fill_circle(body.x + 16.0, body.y + 22.0, 11.0, Color::from_hex(0x2B8A5B));
                    self.painter.text_centered(Rect::new(body.x + 6.0, body.y + 12.0, 20.0, 20.0), "i", 14.0, C_WHITE);
                }
                self.painter.text_wrapped(body.x + 38.0, body.y + 10.0, body.w - 50.0, text, 13.0, C_TEXT, 22.0);
                self.footer_buttons(footer, &[("确定", 0, true)]);
            }
            Dialog::Confirm { text, .. } => {
                self.painter.text_wrapped(body.x + 6.0, body.y + 12.0, body.w - 20.0, text, 13.5, C_TEXT, 23.0);
                self.footer_buttons(footer, &[("取消", 1, false), ("确定", 0, true)]);
            }
            Dialog::Cut(st) => self.draw_cut_dialog(st, body, footer),
            Dialog::Foam(st) => self.draw_foam_dialog(st, body, footer),
            Dialog::Code(st) => self.draw_code_dialog(st, body, footer),
            Dialog::Calc(st) => self.draw_calc_dialog(st, body, footer),
            Dialog::Settings(st) => self.draw_settings_dialog(st, body, footer),
            Dialog::FileInfo(st) => self.draw_fileinfo_dialog(st, body, footer),
            Dialog::Array(st) => self.draw_array_dialog(st, body, footer),
            Dialog::Prompt(st) => self.draw_prompt_dialog(st, body, footer),
            Dialog::Transfer(st) => self.draw_transfer_dialog(st, body, footer),
            Dialog::Tips(idx) => self.draw_tips_dialog(*idx, body, footer),
            Dialog::About => self.draw_about_dialog(body, footer),
            Dialog::Preview => self.draw_preview_dialog(body, footer),
        }
    }

    fn footer_buttons(&mut self, footer: Rect, specs: &[(&str, u8, bool)]) {
        let mut x = footer.right();
        let y = footer.y + 4.0;
        for (label, id, primary) in specs.iter().rev() {
            let w = (self.painter.text_width(label, 13.0) + 36.0).max(78.0);
            x -= w;
            let r = Rect::new(x, y, w, 28.0);
            let hover = r.contains(self.mouse.0, self.mouse.1);
            let bg = if *primary {
                if hover { Color::from_hex(0x3579BE) } else { Color::from_hex(0x2B6CB0) }
            } else if hover {
                Color::from_hex(0xE3EBF4)
            } else {
                Color::from_hex(0xEDF1F6)
            };
            self.painter.fill_round_rect(r, 4.0, bg);
            self.painter.stroke_rect(r, if *primary { Color::from_hex(0x1F5A96) } else { C_BORDER }, 1.0);
            self.painter
                .text_centered(r, label, 13.0, if *primary { C_WHITE } else { C_TEXT });
            self.dialog_hits.push((r, Act::DlgButton(*id)));
            x -= 10.0;
        }
    }

    fn dialog_label(&mut self, x: f32, y: f32, text: &str) {
        self.painter.text_top(x, y + 4.0, text, 12.5, C_TEXT_DIM);
    }

    fn dialog_field(&mut self, label: &str, r: Rect, value: &str, id: &str, label_w: f32) {
        self.dialog_label(r.x - label_w - 8.0, r.y, label);
        let focus = self.focus.as_deref() == Some(id);
        self.painter.fill_rect(r, C_WHITE);
        self.painter.stroke_rect(r, if focus { C_ACCENT } else { C_BORDER }, if focus { 1.5 } else { 1.0 });
        let text = truncate_to_width(value, r.w - 12.0, 12.5);
        if focus && self.field_selected {
            let tw = self.painter.text_width(&text, 12.5).min(r.w - 12.0);
            self.painter
                .fill_rect(Rect::new(r.x + 5.0, r.y + 3.0, tw + 2.0, r.h - 7.0), Color::rgba(43, 108, 176, 55));
        }
        self.painter.text_top(r.x + 6.0, r.y + 3.0, &text, 12.5, C_TEXT);
        if focus {
            let caret = r.x + 6.0 + self.painter.text_width(&text, 12.5).min(r.w - 12.0);
            self.painter.line(caret, r.y + 3.0, caret, r.bottom() - 3.0, C_TEXT, 1.0);
        }
        self.field_hits.push(FieldHit { r, id: id.to_string() });
    }

    fn dialog_combo(&mut self, r: Rect, value: &str, id: u8) {
        self.painter.fill_rect(r, C_WHITE);
        self.painter.stroke_rect(r, C_BORDER, 1.0);
        self.painter.text_top(r.x + 7.0, r.y + 4.0, value, 12.5, C_TEXT);
        let ax = r.right() - 14.0;
        let ay = r.y + r.h * 0.5 - 2.0;
        self.painter.fill_poly(
            &[(ax, ay), (ax + 8.0, ay), (ax + 4.0, ay + 5.0)],
            C_TEXT_DIM,
        );
        self.dialog_hits.push((r, Act::DlgCycle(id)));
    }

    fn dialog_check(&mut self, r: Rect, label: &str, checked: bool, id: u8) {
        let box_r = Rect::new(r.x, r.y + 2.0, 16.0, 16.0);
        self.painter.fill_rect(box_r, C_WHITE);
        self.painter.stroke_rect(box_r, C_BORDER, 1.0);
        if checked {
            draw_icon(&mut self.painter, Icon::Check, box_r.inset(2.0), C_ACCENT);
        }
        self.painter.text_top(r.x + 24.0, r.y + 1.0, label, 12.5, C_TEXT);
        self.dialog_hits.push((Rect::new(r.x, r.y, 280.0, 24.0), Act::DlgToggle(id)));
    }

    fn draw_cut_dialog(&mut self, st: &CutState, body: Rect, footer: Rect) {
        let x = body.x;
        let mut y = body.y + 4.0;
        self.painter.text_top(x, y, "选择轮廓与工艺参数后，生成切割路径和机床代码。", 11.5, C_TEXT_DIM);
        y += 26.0;
        self.dialog_label(x, y + 3.0, "代码类型");
        self.dialog_combo(
            Rect::new(x + 76.0, y, 210.0, 26.0),
            match st.kind {
                0 => "3B 代码",
                1 => "G 代码 (ISO)",
                _ => "Mach3 G 代码",
            },
            0,
        );
        self.dialog_label(x + 306.0, y + 3.0, "补偿方向");
        self.dialog_combo(
            Rect::new(x + 378.0, y, 130.0, 26.0),
            match st.side {
                0 => "左补偿",
                1 => "右补偿",
                _ => "不补偿",
            },
            1,
        );
        y += 44.0;
        self.dialog_field("穿丝孔 X", Rect::new(x + 76.0, y, 120.0, 26.0), &st.px, "cut_px", 70.0);
        self.dialog_field("Y", Rect::new(x + 240.0, y, 120.0, 26.0), &st.py, "cut_py", 18.0);
        self.painter.text_top(x + 380.0, y + 5.0, "mm", 12.0, C_TEXT_DIM);
        y += 42.0;
        self.dialog_field("补偿量", Rect::new(x + 76.0, y, 120.0, 26.0), &st.comp, "cut_comp", 70.0);
        self.painter.text_top(x + 204.0, y + 5.0, "mm", 12.0, C_TEXT_DIM);
        y += 42.0;
        self.dialog_field("零件名", Rect::new(x + 76.0, y, 300.0, 26.0), &st.name, "cut_name", 70.0);
        y += 38.0;
        self.dialog_check(
            Rect::new(x, y, 330.0, 22.0),
            "自动优化轮廓顺序（减少空走）",
            st.optimize,
            4,
        );
        y += 34.0;
        let hint = "未选择图元时，会按图上全部图元生成；\n可先框选外轮廓，再执行切割编程。Mach3 代码用于 Mach3 控制器，使用 M3/M5 控制热丝或继电器。";
        self.painter.text_wrapped(x, y, body.w, hint, 11.5, C_TEXT_DIM, 18.0);
        if let Some(err) = &st.error {
            self.painter.text_wrapped(x, y + 46.0, body.w, err, 12.0, Color::from_hex(0xC0392B), 18.0);
        }
        self.footer_buttons(footer, &[("取消", 1, false), ("生成代码", 0, true)]);
    }

    fn draw_foam_dialog(&mut self, st: &FoamState, body: Rect, footer: Rect) {
        let x = body.x;
        let mut y = body.y + 2.0;
        self.painter.text_top(
            x,
            y,
            "泡沫热丝切割：自动就近进刀、旋转闭合轮廓起点、减少空走，输出 Mach3 G 代码。",
            11.5,
            C_TEXT_DIM,
        );
        y += 26.0;
        self.dialog_label(x, y + 3.0, "切割方式");
        self.dialog_combo(
            Rect::new(x + 76.0, y, 190.0, 26.0),
            if st.mode == 1 { "四轴 X/Y + A/B" } else { "两轴 X/Y" },
            20,
        );
        self.dialog_label(x + 290.0, y + 3.0, "补偿");
        self.dialog_combo(
            Rect::new(x + 352.0, y, 150.0, 26.0),
            match st.side {
                0 => "外轮廓（向外）",
                1 => "内轮廓（向内）",
                _ => "不补偿",
            },
            21,
        );
        y += 44.0;
        self.dialog_field("起点 X", Rect::new(x + 70.0, y, 114.0, 26.0), &st.sx, "foam_sx", 64.0);
        self.dialog_field("Y", Rect::new(x + 228.0, y, 114.0, 26.0), &st.sy, "foam_sy", 18.0);
        self.painter.text_top(x + 352.0, y + 5.0, "mm（热丝停放点）", 12.0, C_TEXT_DIM);
        y += 42.0;
        self.dialog_field("割缝宽", Rect::new(x + 70.0, y, 114.0, 26.0), &st.kerf, "foam_kerf", 64.0);
        self.painter.text_top(x + 188.0, y + 5.0, "mm", 12.0, C_TEXT_DIM);
        self.dialog_field("切割速度", Rect::new(x + 340.0, y, 114.0, 26.0), &st.feed, "foam_feed", 68.0);
        self.painter.text_top(x + 460.0, y + 5.0, "mm/min", 12.0, C_TEXT_DIM);
        y += 42.0;
        self.dialog_field("空走速度", Rect::new(x + 70.0, y, 114.0, 26.0), &st.rapid, "foam_rapid", 70.0);
        self.painter.text_top(x + 188.0, y + 5.0, "mm/min", 12.0, C_TEXT_DIM);
        self.dialog_field("引入线", Rect::new(x + 340.0, y, 114.0, 26.0), &st.lead, "foam_lead", 58.0);
        self.painter.text_top(x + 460.0, y + 5.0, "mm", 12.0, C_TEXT_DIM);
        y += 42.0;
        self.dialog_field("上端 X", Rect::new(x + 70.0, y, 114.0, 26.0), &st.tx, "foam_tx", 64.0);
        self.dialog_field("偏移 Y", Rect::new(x + 340.0, y, 114.0, 26.0), &st.ty, "foam_ty", 58.0);
        self.painter.text_top(x + 460.0, y + 5.0, "mm（仅四轴）", 12.0, C_TEXT_DIM);
        y += 42.0;
        self.dialog_field("零件名", Rect::new(x + 70.0, y, 330.0, 26.0), &st.name, "foam_name", 64.0);
        y += 40.0;
        self.dialog_check(
            Rect::new(x, y, 290.0, 22.0),
            "自动排序 + 就近进刀 + 旋转闭合起点",
            st.optimize,
            5,
        );
        self.dialog_check(
            Rect::new(x + 300.0, y, 290.0, 22.0),
            "连续切割（不同轮廓间不关丝）",
            st.continuous,
            6,
        );
        y += 32.0;
        let hint = "两轴用于平切轮廓；四轴时 A/B 是热丝上导向，按上端偏移跟随 X/Y。\n割缝补偿取割缝宽度的一半。Mach3 中请把热丝继电器接到 M3/M5 或主轴输出。";
        self.painter.text_wrapped(x, y, body.w, hint, 11.0, C_TEXT_DIM, 17.0);
        if let Some(err) = &st.error {
            self.painter.text_wrapped(x, y + 40.0, body.w, err, 12.0, Color::from_hex(0xC0392B), 18.0);
        }
        self.footer_buttons(footer, &[("取消", 1, false), ("生成 Mach3 程序", 0, true)]);
    }

    fn draw_code_dialog(&mut self, st: &CodeState, body: Rect, footer: Rect) {
        let top = body.y;
        let buttons = [("校验", 0u8), ("存盘", 1u8), ("字体 -", 3u8), ("字体 +", 4u8)];
        let mut bx = body.x;
        for (label, id) in buttons {
            let w = 62.0;
            let r = Rect::new(bx, top, w, 26.0);
            let hover = r.contains(self.mouse.0, self.mouse.1);
            self.painter.fill_round_rect(r, 4.0, if hover { Color::from_hex(0xE1EBF5) } else { Color::from_hex(0xEDF2F8) });
            self.painter.stroke_rect(r, C_BORDER, 1.0);
            self.painter.text_centered(r, label, 12.0, C_TEXT);
            self.dialog_hits.push((r, Act::DlgButton(id)));
            bx += w + 6.0;
        }
        let summary = truncate_to_width(&st.summary, (body.w - 300.0).max(120.0), 11.5);
        self.painter.text_top_right(
            body.right(),
            top + 6.0,
            &summary,
            11.5,
            C_TEXT_DIM,
        );
        let area = Rect::new(body.x, body.y + 34.0, body.w, body.h - 34.0);
        self.code_area = Some(area);
        self.painter.fill_rect(area, Color::from_hex(0x101820));
        self.painter.stroke_rect(area, Color::from_hex(0x2A3A4C), 1.0);
        let font = st.font;
        let line_h = font * 1.5;
        let total = st.text.lines().count().max(1) as f32;
        let max_scroll = (total * line_h - area.h + 16.0).max(0.0);
        let scroll = st.scroll.clamp(0.0, max_scroll);
        let first = (scroll / line_h).floor() as usize;
        let mut y = area.y + 8.0 - (scroll - first as f32 * line_h);
        let mut line_no = first + 1;
        for line in st.text.lines().skip(first) {
            if y > area.bottom() - 4.0 {
                break;
            }
            let num = format!("{:>4}", line_no);
            if self.settings.line_numbers {
                self.painter.text_top(area.x + 8.0, y, &num, font - 1.0, Color::from_hex(0x5E7893));
            }
            let text_x = area.x + if self.settings.line_numbers { 48.0 } else { 10.0 };
            let txt = truncate_to_width(line, area.right() - text_x - 10.0, font);
            let col = if line.trim_start().starts_with('(') || line.trim_start().starts_with("(3B") {
                Color::from_hex(0x7F96AC)
            } else {
                Color::from_hex(0xD8E6F2)
            };
            self.painter.text_top(text_x, y, &txt, font, col);
            y += line_h;
            line_no += 1;
        }
        if max_scroll > 0.0 {
            let track = Rect::new(area.right() - 10.0, area.y + 4.0, 8.0, area.h - 8.0);
            self.painter.fill_rect(track, Color::from_hex(0x182534));
            let frac = (scroll / max_scroll).clamp(0.0, 1.0);
            let thumb_h = (track.h * (area.h / (total * line_h)).min(1.0)).max(24.0);
            self.painter.fill_rect(
                Rect::new(track.x, track.y + (track.h - thumb_h) * frac, 8.0, thumb_h),
                Color::from_hex(0x6C8BA6),
            );
        }
        self.footer_buttons(footer, &[("关闭", 2, true)]);
    }

    fn draw_calc_dialog(&mut self, st: &CalcState, body: Rect, footer: Rect) {
        let x = body.x;
        let mut y = body.y + 6.0;
        self.dialog_field("工件厚度", Rect::new(x + 110.0, y, 110.0, 26.0), &st.thickness, "calc_th", 90.0);
        self.painter.text_top(x + 230.0, y + 5.0, "mm", 12.0, C_TEXT_DIM);
        y += 48.0;
        self.painter.fill_rect(Rect::new(x, y, body.w, 1.0), C_BORDER);
        y += 14.0;
        for (label, value) in [
            ("切割长度", format!("{:.2} mm", st.length)),
            ("切割面积", format!("{:.2} mm²", st.area)),
            ("加工工时", format!("{:.2} 小时", st.hours)),
            ("加工金额", format!("{:.2} 元", st.money)),
        ] {
            self.painter.text_top(x + 6.0, y, label, 12.5, C_TEXT_DIM);
            self.painter.text_top_bold(x + 130.0, y, &value, 14.0, C_TEXT);
            y += 26.0;
        }
        self.painter.text_top(
            x + 6.0,
            y + 2.0,
            &format!(
                "按工时指数 {:.0} mm²/时、单价 {:.2} 元/时计算",
                self.settings.work_area, self.settings.work_money
            ),
            11.0,
            C_TEXT_LIGHT,
        );
        self.footer_buttons(footer, &[("关闭", 1, false), ("计算", 0, true)]);
    }

    fn draw_settings_dialog(&mut self, st: &SettingsState, body: Rect, footer: Rect) {
        let tabs = ["常用设置", "显示", "代码"];
        let mut tx = body.x;
        for (i, tab) in tabs.iter().enumerate() {
            let tw = self.painter.text_width(tab, 13.0) + 30.0;
            let r = Rect::new(tx, body.y, tw, 28.0);
            let active = st.tab == i;
            self.painter.fill_round_rect(r, 4.0, if active { C_WHITE } else { Color::from_hex(0xE8EDF3) });
            self.painter.stroke_rect(r, if active { C_ACCENT } else { C_BORDER }, 1.0);
            self.painter.text_centered(r, tab, 13.0, if active { C_ACCENT_DARK } else { C_TEXT_DIM });
            self.dialog_hits.push((r, Act::DlgTab(i as u8)));
            tx += tw + 4.0;
        }
        let content = Rect::new(body.x, body.y + 38.0, body.w, body.h - 38.0);
        match st.tab {
            0 => {
                let mut y = content.y + 6.0;
                self.dialog_field("工时计算指数", Rect::new(content.x + 130.0, y, 130.0, 26.0), &st.work_area, "set_area", 110.0);
                self.painter.text_top(content.x + 270.0, y + 5.0, "mm² / 小时", 12.0, C_TEXT_DIM);
                y += 42.0;
                self.dialog_field("工时单价", Rect::new(content.x + 130.0, y, 130.0, 26.0), &st.work_money, "set_money", 110.0);
                self.painter.text_top(content.x + 270.0, y + 5.0, "元 / 小时", 12.0, C_TEXT_DIM);
                y += 42.0;
                self.dialog_field("钼丝直径", Rect::new(content.x + 130.0, y, 130.0, 26.0), &st.moly, "set_moly", 110.0);
                self.painter.text_top(content.x + 270.0, y + 5.0, "mm（用于默认补偿）", 12.0, C_TEXT_DIM);
                y += 42.0;
                self.dialog_field("尺寸单位", Rect::new(content.x + 130.0, y, 130.0, 26.0), &st.unit, "set_unit", 110.0);
                self.painter.text_wrapped(
                    content.x,
                    y + 40.0,
                    content.w,
                    "这些参数只影响计算和默认值，不会改变已经画好的图形。",
                    11.5,
                    C_TEXT_DIM,
                    18.0,
                );
            }
            1 => {
                let mut y = content.y + 4.0;
                self.dialog_check(Rect::new(content.x, y, 280.0, 22.0), "显示圆心标记", st.center, 0);
                y += 30.0;
                self.dialog_check(Rect::new(content.x, y, 280.0, 22.0), "显示交点标记", st.node, 1);
                y += 30.0;
                self.dialog_check(Rect::new(content.x, y, 280.0, 22.0), "显示切割路径", st.cut, 2);
                y += 40.0;
                self.dialog_label(content.x, y + 4.0, "图板背景");
                self.dialog_combo(
                    Rect::new(content.x + 100.0, y, 160.0, 26.0),
                    match st.bg {
                        0 => "深灰色",
                        1 => "蓝黑色",
                        _ => "浅色图纸",
                    },
                    10,
                );
                y += 46.0;
                self.painter.text_wrapped(
                    content.x,
                    y,
                    content.w,
                    "图板颜色、圆心与交点标记会立即应用到当前视图。",
                    11.5,
                    C_TEXT_DIM,
                    18.0,
                );
            }
            _ => {
                let mut y = content.y + 4.0;
                self.dialog_label(content.x, y + 4.0, "默认代码类型");
                self.dialog_combo(
                    Rect::new(content.x + 120.0, y, 180.0, 26.0),
                    match st.code_kind {
                        0 => "3B 代码",
                        1 => "G 代码 (ISO)",
                        _ => "Mach3 G 代码",
                    },
                    11,
                );
                y += 42.0;
                self.dialog_check(Rect::new(content.x, y, 300.0, 22.0), "代码显示时带行号", st.line_no, 3);
                y += 42.0;
                self.painter.text_wrapped(
                    content.x,
                    y,
                    content.w,
                    "切割编程对话框中仍可临时切换代码类型。3B 代码使用微米整数，G 代码使用毫米小数；Mach3 G 代码使用 M3/M5 控制热丝/继电器。泡沫切割的默认参数保存在右侧“泡沫切割”对话框中。",
                    11.5,
                    C_TEXT_DIM,
                    18.0,
                );
            }
        }
        self.footer_buttons(footer, &[("恢复默认", 2, false), ("取消", 1, false), ("确定", 0, true)]);
    }

    fn draw_fileinfo_dialog(&mut self, st: &FileInfoState, body: Rect, footer: Rect) {
        let mut y = body.y + 8.0;
        let x = body.x;
        self.dialog_field("单位", Rect::new(x + 96.0, y, 160.0, 26.0), &st.unit, "fi_unit", 76.0);
        y += 42.0;
        self.dialog_field("工件名", Rect::new(x + 96.0, y, 320.0, 26.0), &st.part, "fi_part", 76.0);
        y += 42.0;
        self.dialog_field("编程", Rect::new(x + 96.0, y, 200.0, 26.0), &st.programmer, "fi_prog", 76.0);
        y += 42.0;
        self.dialog_field("备注", Rect::new(x + 96.0, y, body.w - 96.0, 26.0), &st.note, "fi_note", 76.0);
        self.painter.text_top(
            x,
            y + 36.0,
            "这些信息会随图形一起保存，便于以后查找零件。",
            11.0,
            C_TEXT_DIM,
        );
        self.footer_buttons(footer, &[("取消", 1, false), ("确定", 0, true)]);
    }

    fn draw_array_dialog(&mut self, st: &ArrayState, body: Rect, footer: Rect) {
        let x = body.x;
        let mut y = body.y + 8.0;
        self.painter.text_top(x, y, "以选中图元为基准复制成矩形阵列。", 11.5, C_TEXT_DIM);
        y += 28.0;
        self.dialog_field("行数", Rect::new(x + 80.0, y, 90.0, 26.0), &st.rows, "arr_rows", 60.0);
        self.dialog_field("列数", Rect::new(x + 250.0, y, 90.0, 26.0), &st.cols, "arr_cols", 60.0);
        y += 42.0;
        self.dialog_field("行距", Rect::new(x + 80.0, y, 90.0, 26.0), &st.dy, "arr_dy", 60.0);
        self.painter.text_top(x + 178.0, y + 5.0, "mm", 12.0, C_TEXT_DIM);
        self.dialog_field("列距", Rect::new(x + 250.0, y, 90.0, 26.0), &st.dx, "arr_dx", 60.0);
        self.painter.text_top(x + 348.0, y + 5.0, "mm", 12.0, C_TEXT_DIM);
        self.painter.text_top(x, y + 40.0, "行距、列距可以为负数；原图元保留在当前位置。", 11.0, C_TEXT_DIM);
        self.footer_buttons(footer, &[("取消", 1, false), ("确定", 0, true)]);
    }

    fn draw_prompt_dialog(&mut self, st: &PromptState, body: Rect, footer: Rect) {
        let x = body.x;
        let mut y = body.y + 12.0;
        self.painter.text_top(x, y, &st.label, 13.0, C_TEXT);
        y += 26.0;
        self.dialog_field("数值", Rect::new(x + 70.0, y, 200.0, 28.0), &st.value, "prompt_value", 56.0);
        self.painter.text_wrapped(x, y + 42.0, body.w, &st.hint, 11.0, C_TEXT_DIM, 17.0);
        self.footer_buttons(footer, &[("取消", 1, false), ("确定", 0, true)]);
    }

    fn draw_transfer_dialog(&mut self, st: &TransferState, body: Rect, footer: Rect) {
        let x = body.x;
        let mut y = body.y + 8.0;
        self.dialog_label(x, y + 4.0, "传送端口");
        self.dialog_combo(Rect::new(x + 100.0, y, 140.0, 26.0), &format!("COM{}", st.port + 1), 30);
        self.dialog_label(x + 260.0, y + 4.0, "波特率");
        self.dialog_combo(
            Rect::new(x + 330.0, y, 140.0, 26.0),
            &format!("{}", [9600, 19200, 38400, 115200][st.baud.min(3)]),
            31,
        );
        y += 42.0;
        let bar = Rect::new(x, y, body.w, 18.0);
        self.painter.fill_rect(bar, Color::from_hex(0xE1E7EE));
        self.painter.stroke_rect(bar, C_BORDER, 1.0);
        self.painter.fill_rect(
            Rect::new(bar.x, bar.y, bar.w * st.progress.clamp(0.0, 1.0), bar.h),
            C_ACCENT,
        );
        self.painter.text_centered(bar, &format!("{:.0}%", st.progress * 100.0), 11.0, C_TEXT);
        y += 30.0;
        self.painter.text_wrapped(x, y, body.w, &st.message, 11.5, C_TEXT_DIM, 18.0);
        self.footer_buttons(footer, &[("关闭", 1, false), ("开始传送", 0, true)]);
    }

    fn draw_tips_dialog(&mut self, idx: usize, body: Rect, footer: Rect) {
        let x = body.x;
        let mut y = body.y + 8.0;
        self.painter.text_top_bold(x, y, "您知道吗？", 15.0, C_ACCENT_DARK);
        y += 34.0;
        self.painter.text_wrapped(x, y, body.w - 20.0, TIPS[idx % TIPS.len()], 13.5, C_TEXT, 24.0);
        self.footer_buttons(footer, &[("关闭", 1, false), ("下一条", 0, true)]);
    }

    fn draw_about_dialog(&mut self, body: Rect, footer: Rect) {
        let x = body.x + 8.0;
        let y = body.y + 4.0;
        self.painter.fill_round_rect(Rect::new(x, y, 64.0, 64.0), 10.0, Color::from_hex(0x2A5D8C));
        draw_icon(&mut self.painter, Icon::Gear, Rect::new(x + 12.0, y + 12.0, 40.0, 40.0), C_WHITE);
        self.painter.text_top_bold(x + 80.0, y + 6.0, "超越数控线切割编程系统", 18.0, C_TEXT);
        self.painter.text_top(x + 80.0, y + 32.0, "版本 1.1 · WEDM / FOAM", 12.0, C_TEXT_DIM);
        self.painter.text_top(x + 80.0, y + 50.0, "绘图 · 切割路径 · 3B / Mach3 G 代码", 12.0, C_TEXT_DIM);
        let text = "面向快走丝 / 中走丝线切割和热丝泡沫切割的绘图与编程工具。\n可以在同一张图板上绘制轮廓、穿丝孔、生成补偿切割路径，\n再输出 3B 或 Mach3 G 代码，并估算加工面积、工时和金额。\n\n泡沫切割模块会自动优化进刀顺序与空走路径，\n可按两轴 X/Y 或四轴 X/Y + A/B 输出。\n\n本程序依据经典线切割编程软件的工作流程重新制作。";
        self.painter
            .text_wrapped(x, y + 86.0, body.w - 20.0, text, 12.5, C_TEXT, 21.0);
        self.footer_buttons(footer, &[("关闭", 0, true)]);
    }

    fn draw_preview_dialog(&mut self, body: Rect, footer: Rect) {
        let page = Rect::new(body.x + 8.0, body.y + 4.0, body.w - 16.0, body.h - 12.0);
        self.painter.fill_rect(page, C_WHITE);
        self.painter.stroke_rect(page, Color::from_hex(0xB8C2CE), 1.0);
        let bb = self.doc.bounds().unwrap_or(BBox {
            min: Pt::new(0.0, 0.0),
            max: Pt::new(100.0, 100.0),
        });
        let margin = 40.0;
        let sx = (page.w - margin * 2.0) / bb.width().max(1.0);
        let sy = (page.h - margin * 2.0 - 40.0) / bb.height().max(1.0);
        let sc = sx.min(sy);
        let cx = page.x + page.w * 0.5;
        let cy = page.y + (page.h - 30.0) * 0.5;
        let c = bb.center();
        let w2s = |p: Pt| -> (f32, f32) {
            (cx + (p.x - c.x) * sc, cy - (p.y - c.y) * sc)
        };
        let ents = self.doc.entities.clone();
        for e in &ents {
            match &e.kind {
                Kind::Point { p } => {
                    let s = w2s(*p);
                    self.painter.line(s.0 - 4.0, s.1, s.0 + 4.0, s.1, Color::from_hex(0x111111), 1.0);
                    self.painter.line(s.0, s.1 - 4.0, s.0, s.1 + 4.0, Color::from_hex(0x111111), 1.0);
                }
                Kind::Line { a, b } => {
                    let sa = w2s(*a);
                    let sb = w2s(*b);
                    self.painter.line(sa.0, sa.1, sb.0, sb.1, Color::from_hex(0x111111), 1.2);
                }
                Kind::Circle { c: cc, r } => {
                    let s = w2s(*cc);
                    self.painter.stroke_circle(s.0, s.1, r * sc, Color::from_hex(0x111111), 1.2);
                }
                Kind::Arc { c: cc, r, a0, sweep } => {
                    let s = w2s(*cc);
                    self.painter.stroke_arc(s.0, s.1, r * sc, -*a0, -*sweep, Color::from_hex(0x111111), 1.2);
                }
            }
        }
        let info = format!(
            "{}    单位 {}    图元 {}    切割长度 {:.2} mm",
            self.doc.part,
            self.doc.unit,
            self.doc.entities.len(),
            self.cut_job.as_ref().map(|j| j.length()).unwrap_or_else(|| self.doc.total_length())
        );
        self.painter.text_top(page.x + 24.0, page.bottom() - 26.0, &info, 11.0, Color::from_hex(0x333333));
        self.painter.text_top_right(page.right() - 24.0, page.bottom() - 26.0, "打印预览 1:1", 10.0, Color::from_hex(0x777777));
        self.footer_buttons(footer, &[("关闭", 1, false), ("打印", 0, true)]);
    }
}

// ---------- helpers ----------

fn dialog_title(d: &Dialog) -> &'static str {
    match d {
        Dialog::Message { .. } => "提示",
        Dialog::Confirm { .. } => "确认",
        Dialog::Cut(_) => "切割编程",
        Dialog::Foam(_) => "泡沫切割 (Mach3)",
        Dialog::Code(_) => "代码显示",
        Dialog::Calc(_) => "计算切割面积",
        Dialog::Settings(_) => "参数设置",
        Dialog::FileInfo(_) => "图形辅助信息",
        Dialog::Array(_) => "阵列",
        Dialog::Prompt(_) => "输入参数",
        Dialog::Transfer(_) => "代码传送",
        Dialog::Tips(_) => "每日一贴",
        Dialog::About => "关于本软件",
        Dialog::Preview => "图形打印预览",
    }
}

fn dialog_size(d: &Dialog) -> (f32, f32) {
    match d {
        Dialog::Message { .. } => (540.0, 250.0),
        Dialog::Confirm { .. } => (520.0, 250.0),
        Dialog::Cut(_) => (560.0, 470.0),
        Dialog::Foam(_) => (640.0, 530.0),
        Dialog::Code(_) => (880.0, 640.0),
        Dialog::Calc(_) => (470.0, 380.0),
        Dialog::Settings(_) => (640.0, 400.0),
        Dialog::FileInfo(_) => (520.0, 360.0),
        Dialog::Array(_) => (470.0, 330.0),
        Dialog::Prompt(_) => (470.0, 260.0),
        Dialog::Transfer(_) => (520.0, 340.0),
        Dialog::Tips(_) => (620.0, 300.0),
        Dialog::About => (560.0, 360.0),
        Dialog::Preview => (900.0, 720.0),
    }
}

fn field_mut<'a>(d: &'a mut Dialog, id: &str) -> Option<&'a mut String> {
    match d {
        Dialog::Cut(s) => match id {
            "cut_px" => Some(&mut s.px),
            "cut_py" => Some(&mut s.py),
            "cut_comp" => Some(&mut s.comp),
            "cut_name" => Some(&mut s.name),
            _ => None,
        },
        Dialog::Foam(s) => match id {
            "foam_sx" => Some(&mut s.sx),
            "foam_sy" => Some(&mut s.sy),
            "foam_kerf" => Some(&mut s.kerf),
            "foam_feed" => Some(&mut s.feed),
            "foam_rapid" => Some(&mut s.rapid),
            "foam_lead" => Some(&mut s.lead),
            "foam_tx" => Some(&mut s.tx),
            "foam_ty" => Some(&mut s.ty),
            "foam_name" => Some(&mut s.name),
            _ => None,
        },
        Dialog::Calc(s) => match id {
            "calc_th" => Some(&mut s.thickness),
            _ => None,
        },
        Dialog::Settings(s) => match id {
            "set_area" => Some(&mut s.work_area),
            "set_money" => Some(&mut s.work_money),
            "set_moly" => Some(&mut s.moly),
            "set_unit" => Some(&mut s.unit),
            _ => None,
        },
        Dialog::FileInfo(s) => match id {
            "fi_unit" => Some(&mut s.unit),
            "fi_part" => Some(&mut s.part),
            "fi_prog" => Some(&mut s.programmer),
            "fi_note" => Some(&mut s.note),
            _ => None,
        },
        Dialog::Array(s) => match id {
            "arr_rows" => Some(&mut s.rows),
            "arr_cols" => Some(&mut s.cols),
            "arr_dx" => Some(&mut s.dx),
            "arr_dy" => Some(&mut s.dy),
            _ => None,
        },
        Dialog::Prompt(s) => match id {
            "prompt_value" => Some(&mut s.value),
            _ => None,
        },
        _ => None,
    }
}

fn entity_handles(e: &crate::model::Entity) -> Vec<Pt> {
    match &e.kind {
        Kind::Point { p } => vec![*p],
        Kind::Line { a, b } => vec![*a, *b],
        Kind::Circle { c, .. } => vec![*c],
        Kind::Arc { c, r, a0, sweep } => vec![
            *c,
            c.add(Pt::from_angle(*a0, *r)),
            c.add(Pt::from_angle(*a0 + *sweep, *r)),
        ],
    }
}

fn entity_center(e: &crate::model::Entity) -> Option<Pt> {
    match &e.kind {
        Kind::Circle { c, .. } | Kind::Arc { c, .. } => Some(*c),
        _ => None,
    }
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else if max_chars <= 1 {
        "…".to_string()
    } else {
        let mut out: String = chars[..max_chars - 1].iter().collect();
        out.push('…');
        out
    }
}

fn truncate_to_width(s: &str, max_w: f32, size: f32) -> String {
    let f = fonts();
    if f.ui.width(s, size) <= max_w {
        return s.to_string();
    }
    let mut out = String::new();
    for ch in s.chars() {
        let mut test = out.clone();
        test.push(ch);
        if f.ui.width(&test, size) > max_w {
            break;
        }
        out.push(ch);
    }
    out
}

// Slim rectangle helpers used above.
trait RectExt {
    fn cy(&self) -> f32;
}
impl RectExt for Rect {
    fn cy(&self) -> f32 {
        self.y + self.h * 0.5
    }
}

// Rounded-rectangle stroke helper not present in the painter's public API.
impl Painter {
    fn stroke_round_rect(&mut self, r: Rect, radius: f32, color: Color, width: f32) {
        let radius = radius.min(r.w * 0.5).min(r.h * 0.5);
        let mut pts: Vec<(f32, f32)> = Vec::new();
        let (x0, y0, x1, y1) = (r.x, r.y, r.right(), r.bottom());
        let corners = [
            (x1 - radius, y0 + radius, -std::f32::consts::FRAC_PI_2, 0.0),
            (x1 - radius, y1 - radius, 0.0, std::f32::consts::FRAC_PI_2),
            (x0 + radius, y1 - radius, std::f32::consts::FRAC_PI_2, std::f32::consts::PI),
            (x0 + radius, y0 + radius, std::f32::consts::PI, std::f32::consts::PI * 1.5),
        ];
        for (cx, cy, a0, a1) in corners {
            for i in 0..=6 {
                let a = a0 + (a1 - a0) * (i as f32 / 6.0);
                pts.push((cx + radius * a.cos(), cy + radius * a.sin()));
            }
        }
        self.stroke_polyline(&pts, color, width, true);
    }
}

// Small definition of the toolbar item type used above.
enum TBarItem {
    Btn(Icon, Act, &'static str),
    Sep,
}

fn build_menus() -> Vec<MenuDef> {
    use Act::*;
    use MenuItem::Sep;
    let a = MenuItem::a;
    let c = MenuItem::c;
    vec![
        MenuDef {
            label: "文件",
            items: vec![
                a("新建", New, "Ctrl+N"),
                a("打开...", Open, "Ctrl+O"),
                a("保存", Save, "Ctrl+S"),
                a("另存为...", SaveAs, ""),
                Sep,
                a("读入 3B 程序...", Import3B, ""),
                a("导入 DXF 图形...", ImportDxf, ""),
                a("图形辅助信息...", FileInfo, ""),
                Sep,
                a("打印预览...", Preview, ""),
                Sep,
                a("退出", Exit, ""),
            ],
        },
        MenuDef {
            label: "绘图",
            items: vec![
                a("点", SetTool(Tool::Point), ""),
                a("直线", SetTool(Tool::Line), ""),
                a("多段线", SetTool(Tool::Polyline), ""),
                a("矩形", SetTool(Tool::Rect), ""),
                a("圆", SetTool(Tool::Circle), ""),
                a("圆弧", SetTool(Tool::Arc), ""),
                Sep,
                a("修剪直线", SetTool(Tool::Trim), ""),
            ],
        },
        MenuDef {
            label: "编辑",
            items: vec![
                a("撤销", Undo, "Ctrl+Z"),
                a("重做", Redo, "Ctrl+Y"),
                Sep,
                a("删除", DeleteSel, "Delete"),
                a("复制", SetTool(Tool::Copy), ""),
                a("移动（拖动）", SetTool(Tool::Move), ""),
                a("镜像", SetTool(Tool::Mirror), ""),
                a("偏移...", EditOffset, ""),
                a("阵列...", EditArray, ""),
                a("旋转", SetTool(Tool::Rotate), ""),
                a("缩放...", EditScale, ""),
                Sep,
                a("全选", SelectAll, "Ctrl+A"),
                a("清除选择", ClearSel, "Esc"),
            ],
        },
        MenuDef {
            label: "显示",
            items: vec![
                a("重画", Redraw, ""),
                a("显示全部", FitAll, ""),
                a("窗口缩放", SetTool(Tool::ZoomWindow), ""),
                a("实时平移", Pan, ""),
                Sep,
                a("放大", ZoomIn, ""),
                a("缩小", ZoomOut, ""),
            ],
        },
        MenuDef {
            label: "查询",
            items: vec![
                a("两点距离", QueryDist, ""),
                a("封闭环面积", QueryArea, ""),
                a("封闭环周长", QueryLength, ""),
            ],
        },
        MenuDef {
            label: "选项",
            items: vec![
                c("显示圆心", ToggleCenter, "", Check::Center),
                c("显示交点", ToggleNode, "", Check::Node),
                c("显示切割线", ToggleCut, "", Check::Cut),
                Sep,
                a("系统设置...", Settings, ""),
            ],
        },
        MenuDef {
            label: "编程",
            items: vec![
                a("切割编程...", CutProgram, ""),
                a("泡沫切割 (Mach3)...", FoamProgram, "F5"),
                a("计算切割面积...", CalcArea, ""),
                Sep,
                a("代码校验", CodeCheck, ""),
                a("代码显示", CodeShow, ""),
                a("代码存盘...", CodeSave, ""),
                a("代码传送...", CodeTransfer, ""),
                Sep,
                a("删除切割路径", DeleteCut, ""),
            ],
        },
        MenuDef {
            label: "帮助",
            items: vec![a("每日一贴", Tips, ""), a("关于本软件", About, "")],
        },
    ]
}

// End of menu definitions.



