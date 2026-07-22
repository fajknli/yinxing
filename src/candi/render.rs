// yinxing - 音形码输入法
// Copyright (C) 2026  fajknli
// Licensed under the GNU AGPL v3. See LICENSE for details.

use rustc_hash::FxHashMap;

use cosmic_text::{
    Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache, SwashContent, Wrap,
};

pub struct TextRenderer {
    font_system: FontSystem,
    swash_cache: SwashCache,
    font_name: String,
    cached_line_h: f32,
    cached_font_size: f32,
    // 改为嵌套 HashMap，外层是 font_size，内层可以用 &str 查找
    width_cache: FxHashMap<u32, FxHashMap<String, i32>>,
}

impl TextRenderer {
    pub fn new(font: &str) -> Self {
        let mut db = cosmic_text::fontdb::Database::new();

        if !font.is_empty() {
            if let Some(path) = fc_match(font) {
                let _ = db.load_font_file(&path);
            }
        }

        for pattern in &["sans:lang=zh", "sans-serif", "sans"] {
            if let Some(path) = fc_match(pattern) {
                if db.load_font_file(&path).is_ok() {
                    break;
                }
            }
        }

        if db.faces().count() == 0 {
            eprintln!("dust: error: no fonts found. Install a CJK font package (e.g. noto-cjk-fonts).");
            std::process::exit(1);
        }

        let font_system = FontSystem::new_with_locale_and_db("en".to_string(), db);

        Self {
            font_system,
            swash_cache: SwashCache::new(),
            font_name: font.to_string(),
            cached_line_h: 0.0,
            cached_font_size: 0.0,
            width_cache: FxHashMap::default(),
        }
    }

    pub fn line_height(&mut self, font_size: f32) -> f32 {
        if self.cached_line_h > 0.0 && (self.cached_font_size - font_size).abs() < 0.01 {
            return self.cached_line_h;
        }

        let metrics = Metrics::new(font_size, font_size * 1.5);
        let mut buf = Buffer::new(&mut self.font_system, metrics);
        buf.set_size(&mut self.font_system, Some(10000.0), Some(10000.0));

        let font_name = self.font_name.clone();
        let mut attrs = Attrs::new().family(Family::SansSerif);
        if !font_name.is_empty() {
            attrs = attrs.family(Family::Name(&font_name));
        }

        buf.set_text(
            &mut self.font_system,
            "Ayg中文\nAyg中文",
            attrs,
            Shaping::Advanced,
        );
        buf.shape_until_scroll(&mut self.font_system, false);

        let mut runs = buf.layout_runs();
        let lh = match (runs.next(), runs.next()) {
            (Some(a), Some(b)) => b.line_top - a.line_top,
            _ => font_size * 1.5,
        };

        self.cached_line_h = lh;
        self.cached_font_size = font_size;
        lh
    }

    pub fn measure_width(&mut self, text: &str, font_size: f32) -> i32 {
        let font_bits = font_size.to_bits();

        // 优化：缓存命中时零分配
        if let Some(inner) = self.width_cache.get(&font_bits) {
            if let Some(&cached) = inner.get(text) {
                return cached;
            }
        }

        let metrics = Metrics::new(font_size, font_size * 1.5);
        let mut buf = Buffer::new(&mut self.font_system, metrics);
        buf.set_size(&mut self.font_system, Some(10000.0), Some(10000.0));
        buf.set_wrap(&mut self.font_system, Wrap::None);

        // 优化：直接借用 self.font_name，避免每次 clone
        let mut attrs = Attrs::new().family(Family::SansSerif);
        if !self.font_name.is_empty() {
            attrs = attrs.family(Family::Name(&self.font_name));
        }

        buf.set_text(&mut self.font_system, text, attrs, Shaping::Advanced);
        buf.shape_until_scroll(&mut self.font_system, false);

        let mut right = 0.0f32;
        for run in buf.layout_runs() {
            for g in run.glyphs.iter() {
                let p = g.physical((0.0, run.line_y), 1.0);
                let r = p.x as f32 + g.w;
                if r > right {
                    right = r;
                }
            }
        }
        let width = right.ceil() as i32;

        // 仅在缓存未命中时分配 String
        self.width_cache
            .entry(font_bits)
            .or_default()
            .insert(text.to_string(), width);
        width
    }

    pub fn render_text(
        &mut self,
        pixels: &mut [u8],
        buf_w: i32,
        buf_h: i32,
        stride: i32,
        text: &str,
        x_off: i32,
        y_off: i32,
        font_size: f32,
        color: u32,
    ) {
        let metrics = Metrics::new(font_size, font_size * 1.5);
        let mut buf = Buffer::new(&mut self.font_system, metrics);
        buf.set_size(&mut self.font_system, Some(10000.0), Some(10000.0));
        buf.set_wrap(&mut self.font_system, Wrap::None);

        // 优化：直接借用，避免 clone
        let mut attrs = Attrs::new().family(Family::SansSerif);
        if !self.font_name.is_empty() {
            attrs = attrs.family(Family::Name(&self.font_name));
        }

        buf.set_text(&mut self.font_system, text, attrs, Shaping::Advanced);
        buf.shape_until_scroll(&mut self.font_system, false);

        let (cr, cg, cb) = extract_rgb(color);

        for run in buf.layout_runs() {
            for glyph in run.glyphs.iter() {
                let phys = glyph.physical((0.0, run.line_y), 1.0);
                let image = match self
                    .swash_cache
                    .get_image(&mut self.font_system, phys.cache_key)
                {
                    Some(img) => img,
                    None => continue,
                };
                let p = image.placement;

                match image.content {
                    SwashContent::Mask => {
                        let start_x = phys.x + p.left + x_off;
                        let start_y = phys.y - p.top + y_off;

                        // 预先计算裁剪边界，彻底消除内层循环的 if 判断
                        let x_min = if start_x < 0 { -start_x } else { 0 };
                        let y_min = if start_y < 0 { -start_y } else { 0 };
                        let x_max = (buf_w - start_x).min(p.width as i32);
                        let y_max = (buf_h - start_y).min(p.height as i32);

                        if x_min >= x_max || y_min >= y_max { continue; }

                        for yy in y_min..y_max {
                            let ty = start_y + yy;
                            let row_start_di = (ty * stride + start_x * 4) as usize;
                            let row_start_si = (yy as usize * p.width as usize) + x_min as usize;

                            for xx in x_min..x_max {
                                let di = row_start_di + (xx as usize) * 4;
                                let si = row_start_si + (xx - x_min) as usize;
                                let alpha = image.data[si];
                                if alpha > 0 {
                                    blend(pixels, di, cr, cg, cb, alpha);
                                }
                            }
                        }
                    }
                    SwashContent::Color => {
                        let start_x = phys.x + p.left + x_off;
                        let start_y = phys.y - p.top + y_off;

                        let x_min = if start_x < 0 { -start_x } else { 0 };
                        let y_min = if start_y < 0 { -start_y } else { 0 };
                        let x_max = (buf_w - start_x).min(p.width as i32);
                        let y_max = (buf_h - start_y).min(p.height as i32);

                        if x_min >= x_max || y_min >= y_max { continue; }

                        for yy in y_min..y_max {
                            let ty = start_y + yy;
                            let row_start_di = (ty * stride + start_x * 4) as usize;
                            let row_start_si = ((yy as usize * p.width as usize) + x_min as usize) * 4;

                            for xx in x_min..x_max {
                                let di = row_start_di + (xx as usize) * 4;
                                let si = row_start_si + (xx - x_min) as usize * 4;
                                if si + 3 < image.data.len() && image.data[si + 3] > 0 {
                                    blend(
                                        pixels, di,
                                        image.data[si], image.data[si + 1],
                                        image.data[si + 2], image.data[si + 3],
                                    );
                                }
                            }
                        }
                    }
                    SwashContent::SubpixelMask => {
                        let start_x = phys.x + p.left + x_off;
                        let start_y = phys.y - p.top + y_off;

                        let x_min = if start_x < 0 { -start_x } else { 0 };
                        let y_min = if start_y < 0 { -start_y } else { 0 };
                        let x_max = (buf_w - start_x).min(p.width as i32);
                        let y_max = (buf_h - start_y).min(p.height as i32);

                        if x_min >= x_max || y_min >= y_max { continue; }

                        for yy in y_min..y_max {
                            let ty = start_y + yy;
                            let row_start_di = (ty * stride + start_x * 4) as usize;
                            let row_start_si = ((yy as usize * p.width as usize) + x_min as usize) * 3;

                            for xx in x_min..x_max {
                                let di = row_start_di + (xx as usize) * 4;
                                let si = row_start_si + (xx - x_min) as usize * 3;
                                if si + 2 < image.data.len() {
                                    let a = image.data[si]
                                        .max(image.data[si + 1])
                                        .max(image.data[si + 2]);
                                    if a > 0 {
                                        blend(pixels, di, cr, cg, cb, a);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn extract_rgb(c: u32) -> (u8, u8, u8) {
    (
        ((c >> 16) & 0xFF) as u8,
        ((c >> 8) & 0xFF) as u8,
        (c & 0xFF) as u8,
    )
}

// 强制内联，消除函数调用开销
#[inline(always)]
fn blend(pixels: &mut [u8], i: usize, r: u8, g: u8, b: u8, a: u8) {
    let sa = a as u32;
    let inv = 255 - sa;
    let bg0 = pixels[i] as u32;
    let bg1 = pixels[i + 1] as u32;
    let bg2 = pixels[i + 2] as u32;
    let bg3 = pixels[i + 3] as u32;
    let oa = sa + (bg3 * inv) / 255;
    if oa > 0 {
        pixels[i] = ((bg0 * inv + b as u32 * sa) / oa) as u8;
        pixels[i + 1] = ((bg1 * inv + g as u32 * sa) / oa) as u8;
        pixels[i + 2] = ((bg2 * inv + r as u32 * sa) / oa) as u8;
        pixels[i + 3] = oa as u8;
    }
}

fn fc_match(pattern: &str) -> Option<String> {
    std::process::Command::new("fc-match")
        .args(["--format", "%{file}", pattern])
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() { None } else { Some(s) }
        })
}
