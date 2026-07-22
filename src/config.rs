// yinxing - 音形码输入法
// Copyright (C) 2026  fajknli
// Licensed under the GNU AGPL v3. See LICENSE for details.

pub const CANDIDATE_PAGE_SIZE: usize = 9;

use crate::candi::{Config as CandiConfig};

// ━━━━━━━━━━━━━━━━ Behavior Config ━━━━━━━━━━━━━━━━

#[derive(Debug, Clone)]
pub struct BehaviorConfig {
    pub page_size: usize,
    pub max_candidates: usize,
    pub min_phrase_chars: usize,
    pub max_phrase_chars: usize,
    pub auto_commit_unselected: bool,
    pub punctuation: String,
    pub default_mode_on_activate: String,
    pub repeat_delay_ms: u32,
    pub repeat_rate_ms: u32,
    pub toggle_key: String,
    pub enable_learning: bool,
    pub enable_phrase_creation: bool,
    pub enable_shift_toggle: bool,
    pub dp_beam_width: usize,
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            page_size: 9,
            max_candidates: 50,
            min_phrase_chars: 2,
            max_phrase_chars: 15,
            auto_commit_unselected: true,
            punctuation: "chinese".to_string(),
            default_mode_on_activate: "english".to_string(),
            repeat_delay_ms: 600,
            repeat_rate_ms: 40,
            toggle_key: "ctrl+space".to_string(),
            enable_learning: true,
            enable_phrase_creation: true,
            enable_shift_toggle: true,
            dp_beam_width: 50,
        }
    }
}

// ━━━━━━━━━━━━━━━━ App Config ━━━━━━━━━━━━━━━━

pub struct AppConfig {
    pub candi: CandiConfig,
    pub behavior: BehaviorConfig,
    pub file_chars: String,
    pub file_words: String,
    pub file_user: String,
    pub custom_phrases: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            candi: CandiConfig::default(),
            behavior: BehaviorConfig::default(),
            file_chars: "dicts/chars.txt".to_string(),
            file_words: "dicts/words.txt".to_string(),
            file_user: "dicts/user.txt".to_string(),
            custom_phrases: String::new(),
        }
    }
}

impl AppConfig {
    pub fn load() -> Self {
        let path = config_path();
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => {
                eprintln!("yinxing: no config at {}, creating with defaults", path);
                Self::write_default(&path);
                return Self::default();
            }
        };

        let toml: toml::Value = match content.parse() {
            Ok(v) => v,
            Err(e) => {
                eprintln!("yinxing: config parse error: {}, using defaults", e);
                return Self::default();
            }
        };

        let mut cfg = Self::default();

        // ── [appearance] ──
        if let Some(ap) = toml.get("appearance") {
            if let Some(v) = ap.get("font").and_then(|v| v.as_str()) {
                cfg.candi.font = v.to_string();
            }
            if let Some(v) = get_f32(ap, "font_size") { cfg.candi.font_size = v; }

            if let Some(v) = ap.get("bg").and_then(|v| v.as_integer()) {
                cfg.candi.bg = v as u32;
            }
            if let Some(v) = ap.get("fg").and_then(|v| v.as_integer()) {
                cfg.candi.fg = v as u32;
            }
            if let Some(v) = ap.get("selected_bg").and_then(|v| v.as_integer()) {
                cfg.candi.selected_bg = v as u32;
            }
            if let Some(v) = ap.get("selected_fg").and_then(|v| v.as_integer()) {
                cfg.candi.selected_fg = v as u32;
            }
            if let Some(v) = ap.get("preedit_fg").and_then(|v| v.as_integer()) {
                cfg.candi.preedit_fg = v as u32;
            }
            if let Some(v) = ap.get("preedit_bg").and_then(|v| v.as_integer()) {
                cfg.candi.preedit_bg = v as u32;
            }
            if let Some(v) = ap.get("label_fg").and_then(|v| v.as_integer()) {
                cfg.candi.label_fg = v as u32;
            }
            if let Some(v) = ap.get("divider_color").and_then(|v| v.as_integer()) {
                cfg.candi.divider_color = v as u32;
            }

            if let Some(v) = get_f32(ap, "hpad") { cfg.candi.hpad = v; }
            if let Some(v) = get_f32(ap, "preedit_hpad") { cfg.candi.preedit_hpad = v; }
            if let Some(v) = get_f32(ap, "vpad") { cfg.candi.vpad = v; }
            if let Some(v) = get_f32(ap, "gap") { cfg.candi.gap = v; }
            if let Some(v) = get_f32(ap, "preedit_gap") { cfg.candi.preedit_gap = v; }

            if let Some(v) = ap.get("anchor").and_then(|v| v.as_str()) {
                cfg.candi.anchor = parse_anchor(v);
            }
            if let Some(v) = get_f32(ap, "corner_radius") { cfg.candi.corner_radius = v; }
            if let Some(v) = ap.get("show_label").and_then(|v| v.as_bool()) {
                cfg.candi.show_label = v;
            }
        }

        // ── [behavior] ──
        if let Some(bh) = toml.get("behavior") {
            if let Some(v) = bh.get("page_size").and_then(|v| v.as_integer()) {
                cfg.behavior.page_size = v.max(1) as usize;
            }
            if let Some(v) = bh.get("max_candidates").and_then(|v| v.as_integer()) {
                cfg.behavior.max_candidates = v.max(1) as usize;
            }
            if let Some(v) = bh.get("min_phrase_chars").and_then(|v| v.as_integer()) {
                cfg.behavior.min_phrase_chars = v.max(1) as usize;
            }
            if let Some(v) = bh.get("max_phrase_chars").and_then(|v| v.as_integer()) {
                cfg.behavior.max_phrase_chars = v.max(1) as usize;
            }
            if let Some(v) = bh.get("auto_commit_unselected").and_then(|v| v.as_bool()) {
                cfg.behavior.auto_commit_unselected = v;
            }
            if let Some(v) = bh.get("punctuation").and_then(|v| v.as_str()) {
                cfg.behavior.punctuation = v.to_string();
            }
            if let Some(v) = bh.get("default_mode_on_activate").and_then(|v| v.as_str()) {
                cfg.behavior.default_mode_on_activate = v.to_string();
            }
            if let Some(v) = bh.get("repeat_delay_ms").and_then(|v| v.as_integer()) {
                cfg.behavior.repeat_delay_ms = v.max(0) as u32;
            }
            if let Some(v) = bh.get("repeat_rate_ms").and_then(|v| v.as_integer()) {
                cfg.behavior.repeat_rate_ms = v.max(0) as u32;
            }
            if let Some(v) = bh.get("toggle_key").and_then(|v| v.as_str()) {
                cfg.behavior.toggle_key = v.to_string();
            }
            if let Some(v) = bh.get("enable_learning").and_then(|v| v.as_bool()) {
                cfg.behavior.enable_learning = v;
            }
            if let Some(v) = bh.get("enable_phrase_creation").and_then(|v| v.as_bool()) {
                cfg.behavior.enable_phrase_creation = v;
            }
            if let Some(v) = bh.get("enable_shift_toggle").and_then(|v| v.as_bool()) {
                cfg.behavior.enable_shift_toggle = v;
            }
            if let Some(v) = bh.get("dp_beam_width").and_then(|v| v.as_integer()) {
                cfg.behavior.dp_beam_width = (v as usize).max(1);
            }
        }

        // ── [dicts] ──
        if let Some(d) = toml.get("dicts") {
            if let Some(v) = d.get("chars").and_then(|v| v.as_str()) {
                cfg.file_chars = v.to_string();
            }
            if let Some(v) = d.get("words").and_then(|v| v.as_str()) {
                cfg.file_words = v.to_string();
            }
            if let Some(v) = d.get("user").and_then(|v| v.as_str()) {
                cfg.file_user = v.to_string();
            }
            if let Some(v) = d.get("custom_phrases").and_then(|v| v.as_str()) {
                cfg.custom_phrases = v.to_string();
            }
        }

        // ── Resolve paths ──
        let base = std::path::Path::new(&path).parent()
            .unwrap_or(std::path::Path::new("."))
            .to_string_lossy().to_string();

        cfg.file_chars = resolve_relative(&cfg.file_chars, &base);
        cfg.file_words = resolve_relative(&cfg.file_words, &base);
        cfg.file_user = resolve_relative(&cfg.file_user, &base);

        if cfg.custom_phrases.is_empty() {
            let parent = std::path::Path::new(&cfg.file_user).parent()
                .unwrap_or(std::path::Path::new("."));
            cfg.custom_phrases = parent.join("user_phrases.txt")
                .to_string_lossy().to_string();
        } else {
            cfg.custom_phrases = resolve_relative(&cfg.custom_phrases, &base);
        }

        cfg
    }

    fn write_default(path: &str) {
        if let Some(parent) = std::path::Path::new(path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let content = r#"# yinxing - 音形码输入法配置文件
# 修改后发送 SIGHUP 信号即可热加载: killall -SIGHUP yinxing

[appearance]
font = ""
font_size = 16.0

# 颜色配置 (格式: 0xAARRGGBB)
bg = 0xE6141522           # 极暗冷黑 (带透明度)
fg = 0xFFa9b5d5           # 基准冷白
selected_bg = 0xFF565d7e  # 深冷灰 (高亮背景)
selected_fg = 0xFFd4dcf2  # 亮冷白 (高亮文字)
preedit_fg = 0xFF8fa1cc   # 中冷蓝 (预编辑区文字)
preedit_bg = 0xFF1b1d2b   # 微冷黑 (预编辑区背景)
label_fg = 0xFF6f78a0     # 暗冷灰 (序号文字)
divider_color = 0xFF1b1d2b# 微冷黑 (分割线)

# 布局参数
preedit_hpad = 6          # 预编辑区左右内边距
preedit_gap = 0           # 预编辑区与候选词之间的间距
hpad = 6                  # 候选词左右内边距
gap = 3                   # 候选词之间的间距
vpad = 2                  # 垂直内边距

# UI 设置
anchor = "bottom_center"  # 候选框锚点位置 (top / bottom_center / top_left / bottom_right 等)
corner_radius = 4.0       # 圆角半径, 0 = 直角
show_label = true         # 是否显示候选词前面的 "1." "2." 序号


[behavior]
# 候选与词库
page_size = 9             # 每页显示的候选词数量
max_candidates = 50       # 候选词总数上限 (长句连打建议调大)
min_phrase_chars = 2      # 自动造词的最小字数
max_phrase_chars = 15     # 自动造词的最大字数 (影响长句切分和造词上限)
dp_beam_width = 50        # DP寻路保留路径数 (越大长句越全, 但越耗CPU, 建议 30-100)

# 输入行为
auto_commit_unselected = true           # 按下非候选键时, 自动上屏当前预编辑内容
punctuation = "chinese"                 # 标点方案: "chinese" (中文全角) 或 "english" (英文半角)
default_mode_on_activate = "english"    # 切换窗口时的默认模式: "english" 或 "chinese"

# 按键重复 (单位: 毫秒)
repeat_delay_ms = 30      # 按住多久开始连发 (数值越小触发越快)
repeat_rate_ms = 30       # 连发间隔 (数值越小速度越快)

# 快捷键与开关
toggle_key = "ctrl+space"         # 中英切换组合键 (支持 ctrl+space / super+space / alt+space / shift+space)
enable_shift_toggle = true        # 是否允许单按 Shift 键切换中英文
enable_learning = true            # 词频学习开关 (调整已有词的排序)
enable_phrase_creation = true     # 自动造词开关 (把你选过的长句自动存为新词)


[dicts]
# 字典路径 (相对于本配置文件所在目录, 或绝对路径)
chars = "dicts/chars.txt"         # 单字码表
words = "dicts/words.txt"         # 词语码表
user = "dicts/user.txt"           # 用户学习词库 (自动生成)
custom_phrases = ""               # 自定义短语文件 (留空则使用 dicts/user_phrases.txt)
"#;
        match std::fs::write(path, content) {
            Ok(_) => eprintln!("yinxing: wrote default config to {}", path),
            Err(e) => eprintln!("yinxing: failed to write config: {}", e),
        }
    }

}

// ━━━━━━━━━━━━━━━━ Helpers ━━━━━━━━━━━━━━━━

pub fn config_path() -> String {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return format!("{}/yinxing/config.toml", xdg);
    }
    if let Ok(home) = std::env::var("HOME") {
        return format!("{}/.config/yinxing/config.toml", home);
    }
    "config.toml".to_string()
}

fn parse_anchor(s: &str) -> crate::candi::Anchor {
    match s.to_ascii_lowercase().as_str() {
        "top" => crate::candi::Anchor::Top,
        "bottom" | "bottom_center" => crate::candi::Anchor::BottomCenter,
        "top_left" => crate::candi::Anchor::TopLeft,
        "top_right" => crate::candi::Anchor::TopRight,
        "top_center" => crate::candi::Anchor::TopCenter,
        "bottom_left" => crate::candi::Anchor::BottomLeft,
        "bottom_right" => crate::candi::Anchor::BottomRight,
        "left" => crate::candi::Anchor::Left,
        "right" => crate::candi::Anchor::Right,
        _ => crate::candi::Anchor::BottomCenter,
    }
}

fn resolve_relative(path: &str, base: &str) -> String {
    let p = std::path::Path::new(path);
    if p.is_absolute() {
        path.to_string()
    } else {
        std::path::Path::new(base).join(p).to_string_lossy().to_string()
    }
}

fn get_f32(ap: &toml::Value, key: &str) -> Option<f32> {
    ap.get(key).and_then(|v| {
        if let Some(f) = v.as_float() {
            Some(f as f32)
        } else {
            v.as_integer().map(|i| i as f32)
        }
    })
}
