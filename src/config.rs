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
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            page_size: 9,
            max_candidates: 20,
            min_phrase_chars: 2,
            max_phrase_chars: 8,
            auto_commit_unselected: true,
            punctuation: "chinese".to_string(),
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
            custom_phrases: String::new(), // 空 = 自动从 file_user 推导
        }
    }
}

impl AppConfig {
    pub fn load() -> Self {
        let path = config_path();
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => {
                eprintln!("yinxing: no config at {}, using defaults", path);
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
            // 使用 get_f32 兼容整数和浮点数
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
            // 新增颜色
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

            // 布局参数 (全部使用 get_f32 兼容写 0 或 0.0)
            if let Some(v) = get_f32(ap, "hpad") { cfg.candi.hpad = v; }
            if let Some(v) = get_f32(ap, "preedit_hpad") { cfg.candi.preedit_hpad = v; }
            if let Some(v) = get_f32(ap, "vpad") { cfg.candi.vpad = v; }
            if let Some(v) = get_f32(ap, "gap") { cfg.candi.gap = v; }
            if let Some(v) = get_f32(ap, "preedit_gap") { cfg.candi.preedit_gap = v; }

            if let Some(v) = ap.get("anchor").and_then(|v| v.as_str()) {
                cfg.candi.anchor = parse_anchor(v);
            }
            if let Some(v) = get_f32(ap, "corner_radius") { cfg.candi.corner_radius = v; }
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

        // custom_phrases: 空 = 从 file_user 同目录推导
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
}

// ━━━━━━━━━━━━━━━━ Helpers ━━━━━━━━━━━━━━━━

fn config_path() -> String {
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

// 新增：智能解析 f32，兼容 TOML 整数和浮点数
fn get_f32(ap: &toml::Value, key: &str) -> Option<f32> {
    ap.get(key).and_then(|v| {
        if let Some(f) = v.as_float() {
            Some(f as f32)
        } else {
            v.as_integer().map(|i| i as f32)
        }
    })
}
