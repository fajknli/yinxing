pub const CANDIDATE_PAGE_SIZE: usize = 9;

use crate::candi::{Config as CandiConfig};

pub struct AppConfig {
    pub candi: CandiConfig,
    pub file_chars: String,
    pub file_words: String,
    pub file_user: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            candi: CandiConfig::default(),
            file_chars: "dicts/chars.txt".to_string(),
            file_words: "dicts/words.txt".to_string(),
            file_user: "dicts/user.txt".to_string(),
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

        if let Some(ap) = toml.get("appearance") {
            if let Some(v) = ap.get("font").and_then(|v| v.as_str()) {
                cfg.candi.font = v.to_string();
            }
            if let Some(v) = ap.get("font_size").and_then(|v| v.as_float()) {
                cfg.candi.font_size = v as f32;
            }
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
            if let Some(v) = ap.get("max_items").and_then(|v| v.as_integer()) {
                cfg.candi.max_items = v as usize;
            }
            if let Some(v) = ap.get("anchor").and_then(|v| v.as_str()) {
                cfg.candi.anchor = parse_anchor(v);
            }
        }

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
        }

        let base = std::path::Path::new(&path).parent()
            .unwrap_or(std::path::Path::new("."))
            .to_string_lossy().to_string();

        cfg.file_chars = resolve_relative(&cfg.file_chars, &base);
        cfg.file_words = resolve_relative(&cfg.file_words, &base);
        cfg.file_user = resolve_relative(&cfg.file_user, &base);

        cfg
    }
}

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
