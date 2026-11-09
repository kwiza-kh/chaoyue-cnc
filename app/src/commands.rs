//! Declarative command table for the status-bar command line.
//!
//! Adding a user-visible command is one row here plus the action it points at.
//! Keeping names, aliases and labels in one place (instead of scattering
//! shortcut handling across key and menu code) is the small extension point
//! borrowed from OpenCADStudio's command registry: the UI and the command line
//! both read from a table, so new features do not need their own dispatch code.

use crate::app::{Act, Tool};

pub struct CommandSpec {
    /// Canonical name, lower-case ASCII.
    pub name: &'static str,
    /// Extra names, including short forms and Chinese labels.
    pub aliases: &'static [&'static str],
    pub label: &'static str,
    pub act: Act,
}

macro_rules! cmd {
    ($name:expr, $aliases:expr, $label:expr, $act:expr) => {
        CommandSpec {
            name: $name,
            aliases: $aliases,
            label: $label,
            act: $act,
        }
    };
}

pub const COMMANDS: &[CommandSpec] = &[
    cmd!("new", &["新建"], "新建图形", Act::New),
    cmd!("open", &["打开"], "打开图形", Act::Open),
    cmd!("save", &["存盘", "保存"], "图形存盘", Act::Save),
    cmd!("saveas", &["另存为"], "图形另存为", Act::SaveAs),
    cmd!("dxf", &["导入dxf", "import"], "导入 DXF", Act::ImportDxf),
    cmd!("undo", &["u", "撤销"], "撤销", Act::Undo),
    cmd!("redo", &["重做"], "重做", Act::Redo),
    cmd!("select", &["s", "选择"], "选择工具", Act::SetTool(Tool::Select)),
    cmd!("point", &["po", "点"], "绘制点", Act::SetTool(Tool::Point)),
    cmd!("line", &["l", "直线"], "绘制直线", Act::SetTool(Tool::Line)),
    cmd!("polyline", &["pl", "多段线"], "绘制多段线", Act::SetTool(Tool::Polyline)),
    cmd!("rect", &["rec", "矩形"], "绘制矩形", Act::SetTool(Tool::Rect)),
    cmd!("circle", &["c", "圆"], "绘制圆", Act::SetTool(Tool::Circle)),
    cmd!("arc", &["a", "圆弧"], "绘制圆弧", Act::SetTool(Tool::Arc)),
    cmd!("move", &["m", "移动"], "移动图元", Act::SetTool(Tool::Move)),
    cmd!("copy", &["co", "复制"], "复制图元", Act::SetTool(Tool::Copy)),
    cmd!("mirror", &["mi", "镜像"], "镜像图元", Act::SetTool(Tool::Mirror)),
    cmd!("rotate", &["ro", "旋转"], "旋转图元", Act::SetTool(Tool::Rotate)),
    cmd!("trim", &["修剪"], "修剪直线", Act::SetTool(Tool::Trim)),
    cmd!("measure", &["di", "测量"], "距离测量", Act::SetTool(Tool::Measure)),
    cmd!("offset", &["o", "偏移"], "偏移图元", Act::EditOffset),
    cmd!("array", &["阵列"], "矩形阵列", Act::EditArray),
    cmd!("scale", &["sc", "缩放"], "缩放图元", Act::EditScale),
    cmd!("delete", &["del", "删除"], "删除选中图元", Act::DeleteSel),
    cmd!("selectall", &["all", "全选"], "全选图元", Act::SelectAll),
    cmd!("fit", &["z", "显示全部"], "显示全部", Act::FitAll),
    cmd!("zoom", &["zoomwindow", "窗口缩放"], "窗口缩放", Act::SetTool(Tool::ZoomWindow)),
    cmd!("pan", &["p", "平移"], "实时平移", Act::Pan),
    cmd!("zoomin", &["放大"], "放大", Act::ZoomIn),
    cmd!("zoomout", &["缩小"], "缩小", Act::ZoomOut),
    cmd!("cut", &["切割", "编程"], "切割编程", Act::CutProgram),
    cmd!("foam", &["泡沫", "mach3"], "泡沫切割 (Mach3)", Act::FoamProgram),
    cmd!("code", &["代码"], "显示代码", Act::CodeShow),
    cmd!("check", &["校验"], "代码校验", Act::CodeCheck),
    cmd!("transfer", &["传送"], "代码传送（演示）", Act::CodeTransfer),
    cmd!("calc", &["计算"], "切割面积与金额", Act::CalcArea),
    cmd!("area", &["面积"], "面积查询", Act::QueryArea),
    cmd!("length", &["周长"], "周长查询", Act::QueryLength),
    cmd!("dist", &["距离"], "距离查询", Act::QueryDist),
    cmd!("info", &["图形信息"], "图形辅助信息", Act::FileInfo),
    cmd!("preview", &["预览", "print"], "打印预览", Act::Preview),
    cmd!("settings", &["opt", "设置", "选项"], "系统设置", Act::Settings),
    cmd!("ortho", &["正交"], "切换正交", Act::ToggleOrtho),
    cmd!("polar", &["极轴"], "切换极轴", Act::TogglePolar),
    cmd!("osnap", &["捕捉"], "切换对象捕捉", Act::ToggleOsnap),
    cmd!("grid", &["网格"], "切换网格", Act::ToggleGrid),
    cmd!("snapall", &["全部捕捉"], "打开全部捕捉", Act::SnapAll),
    cmd!("snapnone", &["关闭捕捉"], "关闭捕捉", Act::SnapNone),
    cmd!("tips", &["提示"], "操作提示", Act::Tips),
    cmd!("about", &["关于"], "关于", Act::About),
];

/// Resolve a typed name (case-insensitive for ASCII, exact for Chinese).
pub fn find(input: &str) -> Option<&'static CommandSpec> {
    let text = input.trim().trim_start_matches(':');
    if text.is_empty() {
        return None;
    }
    let lower = text.to_ascii_lowercase();
    COMMANDS.iter().find(|c| {
        c.name.eq_ignore_ascii_case(&lower)
            || c.aliases
                .iter()
                .any(|a| a.eq_ignore_ascii_case(&lower) || *a == text)
    })
}

/// Short help for the message dialog: one command per line, grouped roughly.
pub fn help_text() -> String {
    let mut out = String::from(
        "在下方命令栏输入命令名或别名，按回车执行；直接输入坐标（如 20,15）仍然可以定位。\n\n",
    );
    for c in COMMANDS {
        out.push_str(&format!("{:<10} {}\n", c.name, c.label));
    }
    out.push_str("\n输入 ? 可随时再次打开此列表。");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_aliases_resolve() {
        assert_eq!(find("LINE").map(|c| c.name), Some("line"));
        assert_eq!(find("l").map(|c| c.name), Some("line"));
        assert_eq!(find("直线").map(|c| c.name), Some("line"));
        assert_eq!(find(" foam ").map(|c| c.name), Some("foam"));
        assert!(find("no-such-command").is_none());
    }

    #[test]
    fn every_command_has_a_unique_name() {
        let mut names: Vec<&str> = COMMANDS.iter().map(|c| c.name).collect();
        names.sort_unstable();
        let len = names.len();
        names.dedup();
        assert_eq!(len, names.len());
    }
}
