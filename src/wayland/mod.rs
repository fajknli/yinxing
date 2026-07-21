use std::os::fd::AsRawFd;

use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_client::globals::GlobalListContents;
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::protocol::wl_pointer::{self, WlPointer};
use wayland_client::protocol::wl_shm::WlShm;
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::protocol::wl_buffer::{self, WlBuffer};
use wayland_client::protocol::wl_surface::WlSurface;

use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::ZwlrLayerShellV1;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    ZwlrLayerSurfaceV1, self as ls_types,
};

use crate::candi::{self, Overlay, Config as CandiConfig, Event as CandiEvent, SlotId};
use crate::engine::action::Action;
use crate::engine::RuntimeState;
use crate::proto::{
    apply_actions, ZwpInputMethodManagerV2, ZwpInputMethodV2,
    ZwpInputMethodKeyboardGrabV2,
    ZwpVirtualKeyboardManagerV1, ZwpVirtualKeyboardV1,
};

pub struct WaylandState {
    pub seat: Option<WlSeat>,
    pub im_manager: Option<ZwpInputMethodManagerV2>,
    pub im: Option<ZwpInputMethodV2>,
    pub kb_grab: Option<ZwpInputMethodKeyboardGrabV2>,
    pub vk_manager: Option<ZwpVirtualKeyboardManagerV1>,
    pub vk: Option<ZwpVirtualKeyboardV1>,

    pub done_count: u32,
    pub pending_activate: bool,
    pub current_mods: u32,
    pub last_key_time: u32,

    pub surrounding_text: String,
    pub surrounding_cursor: usize,
    pub surrounding_anchor: usize,
    pub text_change_cause: u32,
    pub content_hint: u32,
    pub content_purpose: u32,

    pub runtime: RuntimeState,
    pub overlay: Option<Overlay>,
    pub overlay_dirty: bool,
    pub shm: Option<WlShm>,
    pub shift_held: bool,
    pub shift_dirty: bool,
    pub output_scale: i32,

    // ── key repeat 状态（对标 fcitx5 的 repeatKey_/repeatTime_/timeEvent_）──
    pub repeat_key: Option<u32>,
    pub repeat_delay_ms: u32,   // 首次延迟，默认 600ms
    pub repeat_rate_ms: u32,    // 重复间隔，默认 40ms (25次/秒)
    pub repeat_next: Option<std::time::Instant>,
    pub notification: Option<(String, std::time::Instant)>,
    pub last_mode_chinese: bool,
    pub has_cursor_info: bool,
    pub config_path: String,
}

impl WaylandState {
    pub fn flush_actions(&mut self, actions: &[Action]) {
        for a in actions {
            if let Action::ForwardKey { key, .. } = a {
                self.forward_key(*key);
            }
        }
        if let Some(im) = &self.im {
            apply_actions(im, actions, self.done_count);
        }
    }

    fn forward_key(&self, key: u32) {
        if let Some(vk) = &self.vk {
            vk.key(self.last_key_time, key, 1);
            vk.key(self.last_key_time, key, 0);
        }
    }
}

// ─── All Dispatch impls ───

impl Dispatch<WlRegistry, GlobalListContents> for WaylandState {
    fn event(_: &mut Self, _: &WlRegistry, _: <WlRegistry as wayland_client::Proxy>::Event,
        _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<WlSeat, ()> for WaylandState {
    fn event(_: &mut Self, _: &WlSeat,
        _: <WlSeat as wayland_client::Proxy>::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<wayland_client::protocol::wl_compositor::WlCompositor, ()> for WaylandState {
    fn event(_: &mut Self, _: &wayland_client::protocol::wl_compositor::WlCompositor,
        _: <wayland_client::protocol::wl_compositor::WlCompositor as wayland_client::Proxy>::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<ZwlrLayerShellV1, ()> for WaylandState {
    fn event(_: &mut Self, _: &ZwlrLayerShellV1,
        _: <ZwlrLayerShellV1 as wayland_client::Proxy>::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<ZwlrLayerSurfaceV1, ()> for WaylandState {
    fn event(state: &mut Self, proxy: &ZwlrLayerSurfaceV1,
        event: <ZwlrLayerSurfaceV1 as wayland_client::Proxy>::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>)
    {
        match event {
            ls_types::Event::Configure { serial, width, height, .. } => {
                proxy.ack_configure(serial);
                if let Some(overlay) = &mut state.overlay {
                    overlay.on_configure(width as i32, height as i32);
                }
            }
            ls_types::Event::Closed => {
                if let Some(overlay) = &mut state.overlay {
                    let _ = overlay.on_closed();
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<WlPointer, ()> for WaylandState {
    fn event(state: &mut Self, _: &WlPointer,
        event: <WlPointer as wayland_client::Proxy>::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>)
    {
        match event {
            wl_pointer::Event::Enter { surface_x, .. }
            | wl_pointer::Event::Motion { surface_x, .. } => {
                if let Some(overlay) = &mut state.overlay {
                    // 仅更新 overlay 内部的 selected 索引，不需要设置 overlay_dirty
                    // 因为渲染循环每帧都会重绘，selected 变了高亮自然会动
                    overlay.on_pointer_motion(surface_x);
                }
            }
            wl_pointer::Event::Button { button, state: btn, .. } => {
                if button == 0x110
                    && matches!(btn, wayland_client::WEnum::Value(wl_pointer::ButtonState::Pressed))
                {
                    if let Some(overlay) = &mut state.overlay {
                        match overlay.on_pointer_click() {
                            Some(CandiEvent::Clicked { index }) => {
                                let items = state.runtime.candidate_items();
                                if let Some(item) = items.get(index) {
                                    if item.prefix.is_some() {
                                        let cand_index = items.iter().take(index).filter(|i| i.prefix.is_some()).count();
                                        let actions = state.runtime.commit_nth(cand_index);
                                        state.flush_actions(&actions);
                                    }
                                }
                                state.overlay_dirty = true;
                            }
                            // 删掉 Dismissed 的强制清空逻辑，因为内部点击不应该触发取消
                            // Some(CandiEvent::Dismissed) => { ... }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<WlShm, ()> for WaylandState {
    fn event(_: &mut Self, _: &WlShm,
        _: <WlShm as wayland_client::Proxy>::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<wayland_client::protocol::wl_output::WlOutput, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _: &wayland_client::protocol::wl_output::WlOutput,
        event: <wayland_client::protocol::wl_output::WlOutput as wayland_client::Proxy>::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>,
    ) {
        use wayland_client::protocol::wl_output::Event;
        if let Event::Scale { factor } = event {
            if factor > 0 && factor != state.output_scale {
                eprintln!("dust: output scale {} → {}", state.output_scale, factor);
                state.output_scale = factor;
                if let Some(overlay) = &mut state.overlay {
                    overlay.set_scale(factor);
                }
            }
        }
    }
}

impl Dispatch<WlShmPool, ()> for WaylandState {
    fn event(_: &mut Self, _: &WlShmPool,
        _: <WlShmPool as wayland_client::Proxy>::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<WlBuffer, SlotId> for WaylandState {
    fn event(state: &mut Self, _: &WlBuffer,
        event: <WlBuffer as wayland_client::Proxy>::Event,
        udata: &SlotId, _: &Connection, _: &QueueHandle<Self>)
    {
        if let wl_buffer::Event::Release = event {
            if let Some(overlay) = &mut state.overlay {
                overlay.on_buffer_release(udata.0);
            }
        }
    }
}

impl Dispatch<WlSurface, ()> for WaylandState {
    fn event(state: &mut Self, _: &WlSurface,
        event: <WlSurface as wayland_client::Proxy>::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>)
    {
        if let wayland_client::protocol::wl_surface::Event::PreferredBufferScale { factor } = event {
            if let Some(overlay) = &mut state.overlay {
                overlay.set_scale(factor);
            }
        }
    }
}

// ─── Main loop ───

pub fn run(runtime: RuntimeState, candi_config: CandiConfig, repeat_delay_ms: u32, repeat_rate_ms: u32) -> Result<(), Box<dyn std::error::Error>> {

    // ── 阻塞 SIGHUP 信号，防止默认行为终止进程 ──
    let mut mask: libc::sigset_t = unsafe { std::mem::zeroed() };
    unsafe {
        libc::sigemptyset(&mut mask);
        libc::sigaddset(&mut mask, libc::SIGHUP);
        libc::sigprocmask(libc::SIG_BLOCK, &mask, std::ptr::null_mut());
    }

    // ── 创建 signalfd ──
    let sfd = unsafe { libc::signalfd(-1, &mask, 0) };
    if sfd == -1 {
        eprintln!("dust: warning: failed to create signalfd, --reload disabled");
    }

    let conn = Connection::connect_to_env()?;
    let (globals, mut eq) = wayland_client::globals::registry_queue_init::<WaylandState>(&conn)?;
    let qh = eq.handle();
    let shm: WlShm = globals.bind(&qh, 1..=1, ()).expect("wl_shm");

    let seat: WlSeat = globals.bind(&qh, 1..=9, ()).expect("wl_seat");
    let im_manager: ZwpInputMethodManagerV2 = globals.bind(&qh, 1..=1, ()).expect("im_manager");
    let vk_manager: ZwpVirtualKeyboardManagerV1 = globals.bind(&qh, 1..=1, ()).expect("vk_manager");

    let mut state = WaylandState {
        seat: Some(seat),
        im_manager: Some(im_manager),
        vk_manager: Some(vk_manager),
        im: None, kb_grab: None, vk: None,
        done_count: 0, pending_activate: false, current_mods: 0,
        last_key_time: 0,
        surrounding_text: String::new(), surrounding_cursor: 0,
        surrounding_anchor: 0, text_change_cause: 0,
        content_hint: 0, content_purpose: 0,
        runtime,
        overlay: None,
        overlay_dirty: false,
        shm: Some(shm),
        shift_held: false,
        shift_dirty: false,
        output_scale: 1,
        repeat_key: None,
        repeat_delay_ms,  // from config, default 600
        repeat_rate_ms,   // from config, default 40
        repeat_next: None,
        notification: None,
        last_mode_chinese: false,
        has_cursor_info: false,
        config_path: crate::config::config_path(),
    };

    let seat = state.seat.as_ref().ok_or("no wl_seat")?;
    let mgr = state.im_manager.as_ref().ok_or("no zwp_input_method_manager_v2")?;
    state.im = Some(mgr.get_input_method(seat, &qh, ()));
    eq.roundtrip(&mut state)?;

    // 绑定 wl_output 获取 scale（必须在 overlay 创建之前）
    {
        use wayland_client::protocol::wl_output::WlOutput;
        let _output: WlOutput = globals.bind(&qh, 2..=4, ()).expect("wl_output");
    }
    eq.roundtrip(&mut state)?;
    eprintln!("dust: detected output scale: {}", state.output_scale);

    let overlay = candi::init(&globals, &qh, candi_config, state.output_scale);
    state.overlay = Some(overlay);
    eq.roundtrip(&mut state)?;

    let seat = state.seat.as_ref().unwrap();
    seat.get_pointer(&qh, ());
    eprintln!("dust: ready");

    // ── 主事件循环 ──
    loop {
        // ── 0. 检测模式变化 → 设置 notification ──
        let now_chinese = state.runtime.is_chinese();
        if now_chinese != state.last_mode_chinese {
            state.last_mode_chinese = now_chinese;
            let label = if now_chinese { "中" } else { "En" };
            state.notification = Some((label.to_string(), std::time::Instant::now()));
            state.overlay_dirty = true;
        }

        // ── 1. Notification 过期清理 ──
        if let Some((_, t)) = &state.notification {
            if t.elapsed().as_millis() >= 800 {
                state.notification = None;
                if let Some(overlay) = &mut state.overlay {
                    overlay.hide();
                }
                state.overlay_dirty = true;
            }
        }

        // ── 2. 快路径 ──
        if state.overlay_dirty {
            let wayland_fd = conn.backend().poll_fd().as_raw_fd();
            let mut peek = [
                libc::pollfd { fd: wayland_fd, events: libc::POLLIN, revents: 0 },
            ];
            unsafe { libc::poll(peek.as_mut_ptr(), 1, 0); }

            if peek[0].revents & libc::POLLIN != 0 {
                // 还有更多事件 → 跳过渲染
            } else {
                state.overlay_dirty = false;
                if state.notification.is_some() {
                    let text = state.notification.as_ref().unwrap().0.clone();
                    if let Some(overlay) = &mut state.overlay {
                        overlay.show(vec![candi::Item::text(text)], 0);
                    }
                } else if state.runtime.is_chinese() && state.runtime.has_candidates() {
                    let items = state.runtime.candidate_items();
                    if let Some(overlay) = &mut state.overlay {
                        let sel = state.runtime.get_highlight_idx();
                        overlay.show(items, sel);
                    }
                } else {
                    if let Some(overlay) = &mut state.overlay {
                        overlay.hide();
                    }
                }
            }
        }
        // ── 3. 渲染 ──
        if let Some(overlay) = &mut state.overlay {
            if overlay.needs_redraw() {
                if let Some(shm) = &state.shm {
                    if overlay.is_clearing() {
                        overlay.draw_clear(shm, &qh);
                    }
                    if overlay.needs_redraw() {
                        overlay.draw(shm, &qh);
                    }
                }
            }
        }

        let _ = conn.flush();

        // ── 4. 计算 poll 超时 ──
        let timeout_ms: i32 = {
            let mut t: i64 = -1;

            if let Some(next) = state.repeat_next {
                let remaining = next.saturating_duration_since(std::time::Instant::now());
                let ms = remaining.as_millis() as i64;
                if t == -1 || ms < t { t = ms; }
            }

            if let Some((_, instant)) = &state.notification {
                let elapsed = instant.elapsed().as_millis();
                if elapsed < 800 {
                    let remaining = 800 - elapsed;
                    let ms = remaining.max(1) as i64;
                    if t == -1 || ms < t { t = ms; }
                }
            }

            if t == -1 { -1 } else { t.min(i32::MAX as i64) as i32 }
        };

        let wayland_fd = conn.backend().poll_fd().as_raw_fd();
        let mut fds = [
            libc::pollfd { fd: wayland_fd, events: libc::POLLIN, revents: 0 },
            libc::pollfd { fd: sfd, events: libc::POLLIN, revents: 0 }, // signalfd
        ];

        unsafe { libc::poll(fds.as_mut_ptr(), 2, timeout_ms); }

        if fds[0].revents & libc::POLLIN != 0 {
            if eq.blocking_dispatch(&mut state).is_err() {
                eprintln!("dust: connection error");
                break;
            }
        } else {
            let _ = eq.dispatch_pending(&mut state);
        }

        // ── 处理 SIGHUP 信号 (配置热加载) ──
        if sfd != -1 && fds[1].revents & libc::POLLIN != 0 {
            // 必须读取信号信息，否则 poll 会一直被唤醒
            let mut buf: libc::signalfd_siginfo = unsafe { std::mem::zeroed() };
            unsafe {
                libc::read(sfd, &mut buf as *mut _ as *mut libc::c_void, std::mem::size_of::<libc::signalfd_siginfo>());
            }

            eprintln!("dust: SIGHUP received, reloading config...");
            let new_cfg = crate::config::AppConfig::load();

            // 1. 更新 Overlay 配置
            if let Some(overlay) = &mut state.overlay {
                if overlay.config.font != new_cfg.candi.font {
                    eprintln!("dust: warning: font change requires restart.");
                }
                overlay.config = new_cfg.candi;

                // 优化：如果当前候选框可见，立刻重新布局并应用
                if overlay.is_visible() {
                    let items = state.runtime.candidate_items();
                    let sel = state.runtime.get_highlight_idx();
                    overlay.show(items, sel); // show 内部会调用 layout_items
                } else {
                    overlay.rendered = false;
                }
                state.overlay_dirty = true;
            }

            // 2. 更新引擎行为配置
            state.runtime.cfg = new_cfg.behavior;
        }

        // ── 5. Key repeat ──
        if let Some(next) = state.repeat_next {
            if std::time::Instant::now() >= next {
                if let Some(key) = state.repeat_key {
                    let mods = state.current_mods;
                    let actions = state.runtime.process_key(key, mods);
                    state.flush_actions(&actions);
                    // 连击时也打断
                    if state.notification.is_some() {
                        state.notification = None;
                    }
                    state.overlay_dirty = true;
                    state.repeat_next = Some(
                        std::time::Instant::now()
                            + std::time::Duration::from_millis(
                                state.repeat_rate_ms.max(1) as u64
                            )
                    );
                } else {
                    state.repeat_next = None;
                }
            }
        }
        state.runtime.flush_user_buffer();
    }
    // 进程退出前把剩余的写完
    state.runtime.flush_user_buffer();

    Ok(())
}
