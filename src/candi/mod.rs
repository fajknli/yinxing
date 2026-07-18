// candi — Overlay rendering engine (shared connection)
// All Dispatch impls live in src/wayland/mod.rs.

mod render;

use wayland_client::{
    protocol::{
        wl_compositor::WlCompositor,
        wl_shm::{self, WlShm},
        wl_surface::WlSurface,
    },
    QueueHandle, globals::GlobalList,
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{Layer, ZwlrLayerShellV1},
    zwlr_layer_surface_v1::{Anchor as WlAnchor, KeyboardInteractivity, ZwlrLayerSurfaceV1},
};

use render::TextRenderer;
use crate::wayland::WaylandState;

// ━━━━━━━━━━━━━━━━ Public types ━━━━━━━━━━━━━━━━

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    Top, Bottom, TopLeft, TopRight, TopCenter,
    BottomLeft, BottomRight, BottomCenter, Left, Right,
}

impl Default for Anchor {
    fn default() -> Self { Anchor::BottomCenter }
}

impl Anchor {
    pub(crate) fn to_wl(self) -> WlAnchor {
        match self {
            Anchor::Top          => WlAnchor::Top,
            Anchor::Bottom       => WlAnchor::Bottom,
            Anchor::TopLeft      => WlAnchor::Top | WlAnchor::Left,
            Anchor::TopRight     => WlAnchor::Top | WlAnchor::Right,
            Anchor::TopCenter    => WlAnchor::Top,
            Anchor::BottomLeft   => WlAnchor::Bottom | WlAnchor::Left,
            Anchor::BottomRight  => WlAnchor::Bottom | WlAnchor::Right,
            Anchor::BottomCenter => WlAnchor::Bottom,
            Anchor::Left         => WlAnchor::Left,
            Anchor::Right        => WlAnchor::Right,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub anchor: Anchor,
    pub bg: u32,
    pub fg: u32,
    pub selected_bg: u32,
    pub selected_fg: u32,
    pub font: String,
    pub font_size: f32,
    pub hpad: f32,
    pub vpad: f32,
    pub gap: f32,
    pub max_items: usize,
    pub clickable: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            anchor: Anchor::BottomCenter,
            bg: 0xE61a1a1a,
            fg: 0xFFe0e0e0,
            selected_bg: 0xFF3a6ea5,
            selected_fg: 0xFFffffff,
            font: String::new(),
            font_size: 16.0,
            hpad: 12.0,
            vpad: 4.0,
            gap: 6.0,
            max_items: 9,
            clickable: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Item {
    pub text: String,
    pub prefix: Option<String>,
}

impl Item {
    pub fn text<S: Into<String>>(s: S) -> Self {
        Self { text: s.into(), prefix: None }
    }
    pub fn with_prefix<S: Into<String>>(prefix: S, text: S) -> Self {
        Self { text: text.into(), prefix: Some(prefix.into()) }
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    Clicked { index: usize },
    Dismissed,
    Closed,
}

/// User data attached to each WlBuffer to identify its slot index.
/// Used by Dispatch<WlBuffer, SlotId> in wayland/mod.rs.
#[derive(Debug)]
pub(crate) struct SlotId(pub usize);

// ━━━━━━━━━━━━━━━━ Internal ━━━━━━━━━━━━━━━━

struct BufferSlot {
    buffer: wayland_client::protocol::wl_buffer::WlBuffer,
    _fd: std::os::fd::OwnedFd,
    ptr: *mut u8,
    size: usize,
    w: i32,
    h: i32,
    busy: bool,
}

impl Drop for BufferSlot {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { libc::munmap(self.ptr as *mut libc::c_void, self.size); }
        }
    }
}

struct LayoutItem {
    display: String,
    x: i32,
    w: i32,
}

// ━━━━━━━━━━━━━━━━ Overlay ━━━━━━━━━━━━━━━━

pub struct Overlay {
    pub(crate) surface: WlSurface,
    pub(crate) layer_surface: ZwlrLayerSurfaceV1,
    pub(crate) config: Config,
    renderer: TextRenderer,
    buffers: [Option<BufferSlot>; 2],
    items: Vec<Item>,
    selected: usize,
    visible: bool,
    width: i32,
    height: i32,
    want_w: i32,
    want_h: i32,
    req_w: i32,
    req_h: i32,
    layout: Vec<LayoutItem>,
    pointer_x: f64,
    needs_configure: bool,
    wants_clear: bool,
    cursor_x: i32,
    cursor_y: i32,
    cursor_w: i32,
    cursor_h: i32,
    scale: i32,
    popup_buffers: [Option<BufferSlot>; 2],
    use_popup: bool,
    // ── 新增：独立的 popup surface 用于接收光标坐标 ──
    pub(crate) popup_wl_surface: Option<WlSurface>,
    pub(crate) popup_surface: Option<crate::proto::ZwpInputPopupSurfaceV2>,
}

// ━━━━━━━━━━━━━━━━ Init ━━━━━━━━━━━━━━━━

pub(crate) fn init(
    globals: &GlobalList,
    qh: &QueueHandle<WaylandState>,
    config: Config,
    initial_scale: i32,
) -> Overlay {
    let compositor: WlCompositor =
        globals.bind(qh, 1..=4, ()).expect("wl_compositor");
    let layer_shell: ZwlrLayerShellV1 =
        globals.bind(qh, 1..=4, ()).expect("layer-shell");

    let renderer = TextRenderer::new(&config.font);
    let surface = compositor.create_surface(qh, ());
    // 独立的 wl_surface，专门给 popup 接收光标坐标用
    let popup_wl_surface = compositor.create_surface(qh, ());

    let ls = layer_shell.get_layer_surface(
        &surface, None, Layer::Overlay, "dust-candidates".to_string(), qh, (),
    );
    ls.set_anchor(config.anchor.to_wl());
    ls.set_exclusive_zone(-1);
    ls.set_keyboard_interactivity(KeyboardInteractivity::None);
    ls.set_size(1, 1);
    surface.commit();

    Overlay {
        surface,
        layer_surface: ls,
        config,
        renderer,
        buffers: [None, None],
        items: Vec::new(),
        selected: 0,
        visible: false,
        width: 0,
        height: 0,
        want_w: 0,
        want_h: 0,
        req_w: 0,
        req_h: 0,
        layout: Vec::new(),
        pointer_x: 0.0,
        needs_configure: true,
        wants_clear: false,
        cursor_x: 0,
        cursor_y: 0,
        cursor_w: 0,
        cursor_h: 0,
        scale: initial_scale.max(1),
        popup_buffers: [None, None],
        use_popup: false,
        popup_wl_surface: Some(popup_wl_surface),
        popup_surface: None,
    }
}

// ━━━━━━━━━━━━━━━━ Public methods ━━━━━━━━━━━━━━━━

impl Overlay {

    pub fn show(&mut self, items: Vec<Item>, selected: usize) {
        if items.is_empty() { self.hide(); return; }
        let max = self.config.max_items;
        let count = items.len().min(max);
        self.selected = selected.min(count.saturating_sub(1));
        self.items = items;
        self.visible = true;
        self.layout_items();

        self.use_popup = self.popup_surface.is_some()
            && self.popup_wl_surface.is_some()
            && (self.cursor_w > 0 || self.cursor_h > 0);

        if self.use_popup {
            // 清理 layer surface：如果它之前有内容，渲染透明
            if self.width > 1 {
                self.wants_clear = true;
                self.width = 1;
                self.height = 1;
            }
            self.req_w = 1;
            self.req_h = 1;
            self.needs_configure = false;
        } else {
            // 清理 popup surface
            if let Some(ws) = &self.popup_wl_surface {
                ws.attach(None, 0, 0);
                ws.commit();
            }
            self.wants_clear = false;
            self.update_position();
            if self.want_w != self.req_w || self.want_h != self.req_h {
                self.req_w = self.want_w;
                self.req_h = self.want_h;
                self.layer_surface.set_size(self.want_w as u32, self.want_h as u32);
                self.surface.commit();
                self.needs_configure = true;
            }
            self.surface.commit();
        }
    }

    pub fn hide(&mut self) {
        if !self.visible { return; }
        self.visible = false;
        self.items.clear();
        self.layout.clear();

        // popup: attach(None) 直接卸载
        if let Some(ws) = &self.popup_wl_surface {
            ws.attach(None, 0, 0);
            ws.commit();
        }

        // layer: 标记需要透明渲染清理
        if self.width > 1 {
            self.wants_clear = true;
            self.width = 1;
            self.height = 1;
        }

        self.req_w = 1;
        self.req_h = 1;
        self.use_popup = false;
    }

    pub fn set_selected(&mut self, idx: usize) {
        let max = self.items.len().min(self.config.max_items);
        if max == 0 { return; }
        let clamped = idx.min(max - 1);
        if clamped != self.selected {
            self.selected = clamped;
        }
    }

    pub fn needs_redraw(&self) -> bool {
        self.wants_clear || (self.visible && !self.needs_configure)
    }
    pub fn is_visible(&self) -> bool { self.visible }
    pub fn is_clearing(&self) -> bool { self.wants_clear }

    // ─── Called by Dispatch impls in wayland/mod.rs ───

    pub(crate) fn on_configure(&mut self, width: i32, height: i32) {
        if !self.use_popup {
            if width > 0 { self.width = width; }
            if height > 0 { self.height = height; }
        }
        self.needs_configure = false;
    }
    pub(crate) fn set_cursor_rect(&mut self, x: i32, y: i32, w: i32, h: i32) {
        self.cursor_x = x;
        self.cursor_y = y;
        self.cursor_w = w;
        self.cursor_h = h;
    }

    pub(crate) fn on_pointer_motion(&mut self, x: f64) -> bool {
        self.pointer_x = x;
        if !self.config.clickable || !self.visible || self.layout.is_empty() {
            return false;
        }
        let px = x as i32;
        for (i, item) in self.layout.iter().enumerate() {
            if px >= item.x && px < item.x + item.w {
                if i != self.selected {
                    self.selected = i;
                    return true;
                }
                return false;
            }
        }
        false
    }

    pub(crate) fn on_pointer_click(&mut self) -> Option<Event> {
        if !self.config.clickable || !self.visible { return None; }
        let px = self.pointer_x as i32;
        for (i, item) in self.layout.iter().enumerate() {
            if px >= item.x && px < item.x + item.w {
                return Some(Event::Clicked { index: i });
            }
        }
        Some(Event::Dismissed)
    }

    pub(crate) fn on_closed(&mut self) -> Event {
        self.visible = false;
        Event::Closed
    }

    /// Called when the compositor releases a buffer (wl_buffer::Release).
    pub(crate) fn on_buffer_release(&mut self, slot: usize) {
        if let Some(Some(entry)) = self.buffers.get_mut(slot) {
            entry.busy = false;
        }
        if let Some(Some(entry)) = self.popup_buffers.get_mut(slot) {
            entry.busy = false;
        }
    }

    pub(crate) fn set_scale(&mut self, scale: i32) {
        if scale > 0 && scale != self.scale {
            eprintln!("dust: buffer scale {} → {}", self.scale, scale);
            self.scale = scale;
        }
    }

    fn update_position(&mut self) {
        if self.cursor_w == 0 && self.cursor_h == 0 {
            // 没有光标信息 → 用配置的默认锚点
            self.layer_surface.set_anchor(self.config.anchor.to_wl());
            self.layer_surface.set_margin(0, 0, 0, 0);
            self.surface.commit();
            return;
        }

        // 有光标信息 → 跟随光标
        self.layer_surface.set_anchor(WlAnchor::Top | WlAnchor::Left);
        let top_margin = self.cursor_y + self.cursor_h + 4;
        let left_margin = self.cursor_x;
        self.layer_surface.set_margin(top_margin, 0, 0, left_margin);
        self.surface.commit();
    }

    // ─── Rendering ───

    pub fn draw(&mut self, shm: &WlShm, qh: &QueueHandle<WaylandState>) {
        if !self.visible || self.width <= 0 || self.height <= 0 { return; }
        if self.needs_configure { return; }

        if self.use_popup {
            self.draw_popup(shm, qh);
        } else {
            let idx = match self.acquire_slot(shm, qh) {
                Some(i) => i,
                None => return,
            };
            self.render_slot(idx);
            self.commit_layer(idx);
        }
    }

    pub fn draw_clear(&mut self, shm: &WlShm, qh: &QueueHandle<WaylandState>) {
        if self.width <= 0 || self.height <= 0 {
            self.wants_clear = false;
            return;
        }
        let idx = match self.acquire_slot(shm, qh) {
            Some(i) => i,
            None => return,
        };
        if let Some(entry) = &mut self.buffers[idx] {
            let pixels = unsafe { std::slice::from_raw_parts_mut(entry.ptr, entry.size) };
            for chunk in pixels.chunks_exact_mut(4) {
                chunk.copy_from_slice(&[0, 0, 0, 0]);
            }
        }
        self.commit_layer(idx);
        self.wants_clear = false;
    }

    // ─── Internals ───

    fn commit_layer(&mut self, idx: usize) {
        if let Some(entry) = &mut self.buffers[idx] {
            entry.busy = true;
        }
        if let Some(entry) = &self.buffers[idx] {
            let scale = self.scale;
            self.layer_surface.set_size(self.width as u32, self.height as u32);
            if scale > 1 {
                self.surface.set_buffer_scale(scale);
            }
            self.surface.attach(Some(&entry.buffer), 0, 0);
            self.surface.damage_buffer(0, 0, self.width * scale, self.height * scale);
            self.surface.commit();
        }
    }

    fn draw_popup(&mut self, shm: &WlShm, qh: &QueueHandle<WaylandState>) {
        let scale = self.scale;
        let pw = self.want_w * scale;  // <--- 改用 want_w
        let ph = self.want_h * scale;  // <--- 改用 want_h

        // 获取或创建 popup buffer
        let idx = match self.acquire_popup_slot(shm, qh, pw, ph) {
            Some(i) => i,
            None => return,
        };

        // 渲染到 popup buffer
        self.render_popup_slot(idx, pw, ph);

        // 提交到 popup surface
        if let Some(entry) = &mut self.popup_buffers[idx] {
            entry.busy = true;
        }
        if let (Some(entry), Some(popup_ws)) = (&self.popup_buffers[idx], &self.popup_wl_surface) {
            if scale > 1 {
                popup_ws.set_buffer_scale(scale);
            }
            popup_ws.attach(Some(&entry.buffer), 0, 0);
            popup_ws.damage_buffer(0, 0, pw, ph);
            popup_ws.commit();
        }
    }

    fn acquire_popup_slot(&mut self, shm: &WlShm, qh: &QueueHandle<WaylandState>, pw: i32, ph: i32) -> Option<usize> {
        for i in 0..2 {
            if let Some(s) = &self.popup_buffers[i] {
                if !s.busy && s.w == pw && s.h == ph {
                    return Some(i);
                }
            }
        }
        for i in 0..2 {
            let can_destroy = match &self.popup_buffers[i] {
                Some(s) => !s.busy,
                None => true,
            };
            if can_destroy {
                self.popup_buffers[i] = None;
                if self.create_popup_buffer(shm, qh, i, pw, ph) {
                    return Some(i);
                }
            }
        }
        None
    }

    fn create_popup_buffer(&mut self, shm: &WlShm, qh: &QueueHandle<WaylandState>, idx: usize, w: i32, h: i32) -> bool {
        use std::ffi::CString;
        use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};

        let stride = w * 4;
        let size = (stride * h) as usize;
        if size == 0 { return false; }

        let name = CString::new("dust-popup").unwrap();
        let raw = unsafe { libc::syscall(libc::SYS_memfd_create, name.as_ptr(), 0) };
        if raw == -1 { return false; }
        let fd = unsafe { OwnedFd::from_raw_fd(raw as i32) };
        if unsafe { libc::ftruncate(fd.as_raw_fd(), size as i64) } == -1 { return false; }

        let ptr = unsafe {
            libc::mmap(std::ptr::null_mut(), size, libc::PROT_READ | libc::PROT_WRITE, libc::MAP_SHARED, fd.as_raw_fd(), 0) as *mut u8
        };
        if ptr == libc::MAP_FAILED as *mut u8 { return false; }

        let pool = shm.create_pool(fd.as_fd(), size as i32, qh, ());
        let buffer = pool.create_buffer(0, w, h, stride, wl_shm::Format::Argb8888, qh, SlotId(idx));
        pool.destroy();

        self.popup_buffers[idx] = Some(BufferSlot { buffer, _fd: fd, ptr, size, w, h, busy: false });
        true
    }

    fn render_popup_slot(&mut self, idx: usize, w: i32, h: i32) {
        let entry = match self.popup_buffers[idx].as_mut() {
            Some(e) => e,
            None => return,
        };
        let scale = self.scale;
        let stride = w * 4;
        let pixels = unsafe { std::slice::from_raw_parts_mut(entry.ptr, entry.size) };

        let bg = argb_to_bgra(self.config.bg);
        for chunk in pixels.chunks_exact_mut(4) {
            chunk.copy_from_slice(&bg);
        }

        let font_size = self.config.font_size * scale as f32;
        let base_lh = self.renderer.line_height(font_size);
        let vpad = self.config.vpad.round() as i32 * scale;
        let hpad = self.config.hpad.round() as i32 * scale;
        let text_y = ((base_lh * 1.15 - base_lh) / 2.0).round() as i32 + vpad;

        for (i, item) in self.layout.iter().enumerate() {
            let sel = i == self.selected;
            if sel {
                let sel_bg = argb_to_bgra(self.config.selected_bg);
                fill_rect(pixels, stride, w, h, item.x, 0, item.w, h, &sel_bg);
            }
            let fg = if sel { self.config.selected_fg } else { self.config.fg };
            self.renderer.render_text(
                pixels, w, h, stride, &item.display,
                item.x + hpad, text_y, font_size, fg,
            );
        }
    }

    fn layout_items(&mut self) {
        self.layout.clear();
        let scale = self.scale;
        let hpad = self.config.hpad.round() as i32 * scale;
        let gap = self.config.gap.round() as i32 * scale;
        let font_size = self.config.font_size * scale as f32;
        let count = self.items.len().min(self.config.max_items);
        let mut x = 0;  // ← 从 0 开始，不留前置空白

        for i in 0..count {
            let item = &self.items[i];
            let display = match &item.prefix {
                Some(p) => format!("{}{}", p, item.text),
                None => item.text.clone(),
            };
            let tw = self.renderer.measure_width(&display, font_size);
            let iw = tw + hpad * 2;
            self.layout.push(LayoutItem { display, x, w: iw });
            x += iw + gap;
        }

        // 去掉首尾 hpad
        let pw = if self.layout.is_empty() { 0 } else { x - gap };
        self.want_w = (pw + scale - 1) / scale;
        let base_lh = self.renderer.line_height(font_size);
        let vpad = self.config.vpad.round() as i32 * scale;
        self.want_h = ((base_lh * 1.15).ceil() as i32 + vpad * 2 + scale - 1) / scale;
    }

    fn create_buffer(&mut self, shm: &WlShm, qh: &QueueHandle<WaylandState>, idx: usize) -> bool {
        use std::ffi::CString;
        use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};

        let w = self.width * self.scale;
        let h = self.height * self.scale;
        let stride = w * 4;
        let size = (stride * h) as usize;
        if size == 0 { return false; }

        let name = CString::new("dust-shm").unwrap();
        let raw = unsafe { libc::syscall(libc::SYS_memfd_create, name.as_ptr(), 0) };
        if raw == -1 { return false; }
        let fd = unsafe { OwnedFd::from_raw_fd(raw as i32) };

        if unsafe { libc::ftruncate(fd.as_raw_fd(), size as i64) } == -1 { return false; }

        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(), size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED, fd.as_raw_fd(), 0,
            ) as *mut u8
        };
        if ptr == libc::MAP_FAILED as *mut u8 { return false; }

        let pool = shm.create_pool(fd.as_fd(), size as i32, qh, ());
        let buffer = pool.create_buffer(
            0, w, h, stride, wl_shm::Format::Argb8888, qh,
            SlotId(idx),
        );
        pool.destroy();

        self.buffers[idx] = Some(BufferSlot {
            buffer, _fd: fd, ptr, size, w, h,
            busy: false,
        });
        true
    }

    fn acquire_slot(&mut self, shm: &WlShm, qh: &QueueHandle<WaylandState>) -> Option<usize> {
        // 1. Reuse a non-busy buffer with matching dimensions
        let pw = self.width * self.scale;
        let ph = self.height * self.scale;
        for i in 0..2 {
            if let Some(s) = &self.buffers[i] {
                if !s.busy && s.w == pw && s.h == ph {
                    return Some(i);
                }
            }
        }

        // 2. Destroy a non-busy buffer and recreate with new dimensions
        for i in 0..2 {
            let can_destroy = match &self.buffers[i] {
                Some(s) => !s.busy,
                None => true,
            };
            if can_destroy {
                self.buffers[i] = None;
                if self.create_buffer(shm, qh, i) {
                    return Some(i);
                }
            }
        }
        None
    }

    fn render_slot(&mut self, idx: usize) {
        let entry = match self.buffers[idx].as_mut() {
            Some(e) => e,
            None => return,
        };
        let scale = self.scale;
        let w = self.width * scale;
        let h = self.height * scale;
        let stride = w * 4;
        let pixels = unsafe { std::slice::from_raw_parts_mut(entry.ptr, entry.size) };

        let bg = argb_to_bgra(self.config.bg);
        for chunk in pixels.chunks_exact_mut(4) {
            chunk.copy_from_slice(&bg);
        }

        let font_size = self.config.font_size * scale as f32;
        let base_lh = self.renderer.line_height(font_size);
        let vpad = self.config.vpad.round() as i32 * scale;
        let hpad = self.config.hpad.round() as i32 * scale;
        let text_y = ((base_lh * 1.15 - base_lh) / 2.0).round() as i32 + vpad;

        for (i, item) in self.layout.iter().enumerate() {
            let sel = i == self.selected;
            if sel {
                let sel_bg = argb_to_bgra(self.config.selected_bg);
                fill_rect(pixels, stride, w, h, item.x, 0, item.w, h, &sel_bg);
            }
            let fg = if sel { self.config.selected_fg } else { self.config.fg };
            self.renderer.render_text(
                pixels, w, h, stride,
                &item.display,
                item.x + hpad,
                text_y, font_size, fg,
            );
        }
    }
}

// ━━━━━━━━━━━━━━━━ Helpers ━━━━━━━━━━━━━━━━

fn argb_to_bgra(c: u32) -> [u8; 4] {
    [
        (c & 0xFF) as u8,
        ((c >> 8) & 0xFF) as u8,
        ((c >> 16) & 0xFF) as u8,
        ((c >> 24) & 0xFF) as u8,
    ]
}

fn fill_rect(pixels: &mut [u8], stride: i32, bw: i32, bh: i32,
    x: i32, y: i32, w: i32, h: i32, c: &[u8; 4])
{
    let x0 = x.max(0);
    let x1 = (x + w).min(bw);
    let y0 = y.max(0);
    let y1 = (y + h).min(bh);
    if x0 >= x1 || y0 >= y1 { return; }
    let row_bytes = (x1 - x0) as usize * 4;
    for py in y0..y1 {
        let start = (py * stride + x0 * 4) as usize;
        for chunk in pixels[start..start + row_bytes].chunks_exact_mut(4) {
            chunk.copy_from_slice(c);
        }
    }
}
