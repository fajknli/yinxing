extern crate wayland_client;
extern crate wayland_backend;
use wayland_client::protocol::wl_seat;
use wayland_client::protocol::wl_surface;

// Input Method interfaces
pub(crate) mod im_interfaces {
    extern crate wayland_client;
    use wayland_client::protocol::__interfaces::*;
    wayland_scanner::generate_interfaces!("protocols/input-method-v2.xml");
}
use self::im_interfaces::*;

wayland_scanner::generate_client_code!("protocols/input-method-v2.xml");

pub use self::zwp_input_method_manager_v2::ZwpInputMethodManagerV2;
pub use self::zwp_input_method_v2::ZwpInputMethodV2;
pub use self::zwp_input_method_v2::Event as ZwpInputMethodV2Event;
pub use self::zwp_input_popup_surface_v2::ZwpInputPopupSurfaceV2;
pub use self::zwp_input_method_keyboard_grab_v2::ZwpInputMethodKeyboardGrabV2;

// Virtual Keyboard interfaces
pub(crate) mod vk_interfaces {
    extern crate wayland_client;
    use wayland_client::protocol::__interfaces::*;
    wayland_scanner::generate_interfaces!("protocols/virtual-keyboard-v1.xml");
}
use self::vk_interfaces::*;

wayland_scanner::generate_client_code!("protocols/virtual-keyboard-v1.xml");

pub use self::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;
pub use self::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1;

use wayland_client::{Connection, Dispatch, QueueHandle};
use crate::keys;
use crate::wayland::WaylandState;

impl Dispatch<ZwpInputMethodManagerV2, ()> for WaylandState {
    fn event(_: &mut Self, _: &ZwpInputMethodManagerV2,
        _: <ZwpInputMethodManagerV2 as wayland_client::Proxy>::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<ZwpInputMethodV2, ()> for WaylandState {
    fn event(
        state: &mut Self, _: &ZwpInputMethodV2,
        event: <ZwpInputMethodV2 as wayland_client::Proxy>::Event,
        _: &(), _: &Connection, qhandle: &QueueHandle<Self>,
    ) {
        match event {
            ZwpInputMethodV2Event::Activate => {
                eprintln!("dust: activate");
                state.runtime.activate();
                state.pending_activate = true;
            }
            ZwpInputMethodV2Event::Deactivate => {
                eprintln!("dust: deactivate");
                state.runtime.deactivate();
                state.kb_grab = None;
                state.has_cursor_info = false;
                state.repeat_key = None;
                state.repeat_next = None;
                if let Some(overlay) = &mut state.overlay {
                    // 不销毁 popup_surface，只隐藏 UI
                    // popup_surface 只在第一次 Activate 时创建，永久复用
                    overlay.hide();
                }
            }
            ZwpInputMethodV2Event::Done => {
                state.done_count += 1;
                if state.pending_activate {
                    state.pending_activate = false;
                    if let Some(im) = &state.im {
                        state.kb_grab = Some(im.grab_keyboard(qhandle, ()));
                        eprintln!("dust: keyboard grabbed");
                        if let Some(overlay) = &mut state.overlay {
                            if overlay.popup_surface.is_none() {
                                if let Some(popup_ws) = &overlay.popup_wl_surface {
                                    overlay.popup_surface = Some(
                                        im.get_input_popup_surface(popup_ws, qhandle, ())
                                    );
                                    eprintln!("dust: popup surface created");
                                }
                            }
                        }
                    }
                }
            }
            ZwpInputMethodV2Event::SurroundingText { text, cursor, anchor } => {
                state.surrounding_text = text;
                state.surrounding_cursor = cursor as usize;
                state.surrounding_anchor = anchor as usize;
            }
            ZwpInputMethodV2Event::TextChangeCause { cause } => {
                state.text_change_cause = cause;
            }
            ZwpInputMethodV2Event::ContentType { hint, purpose } => {
                state.content_hint = hint;
                state.content_purpose = purpose;
            }
            ZwpInputMethodV2Event::Unavailable => {
                eprintln!("dust: unavailable");
                state.runtime.deactivate();
            }
        }
    }
}

impl Dispatch<ZwpInputMethodKeyboardGrabV2, ()> for WaylandState {
    fn event(
        state: &mut Self, _: &ZwpInputMethodKeyboardGrabV2,
        event: <ZwpInputMethodKeyboardGrabV2 as wayland_client::Proxy>::Event,
        _: &(), _: &Connection, qhandle: &QueueHandle<Self>,
    ) {
        use self::zwp_input_method_keyboard_grab_v2::Event;
        match event {
            Event::Keymap { format, fd, size } => {
                eprintln!("dust: keymap format={} size={}", format, size);
                if let (Some(vkm), Some(seat)) = (&state.vk_manager, &state.seat) {
                    let vk = vkm.create_virtual_keyboard(seat, qhandle, ());
                    use std::os::unix::io::AsFd;
                    vk.keymap(format, fd.as_fd(), size);
                    state.vk = Some(vk);
                    eprintln!("dust: virtual keyboard ready");
                }
            }
            Event::Key { time, key, state: key_state, .. } => {
                // ── 关键修正：0=released, 1=pressed ──
                // 只处理 pressed 事件，跳过 released
                if key_state == 0 {
                    // 松开 → 如果是当前正在 repeat 的键，停止 repeat
                    if state.repeat_key == Some(key) {
                        state.repeat_key = None;
                        state.repeat_next = None;
                    }
                    return;
                }

                // 按下
                if key == keys::KEY_LEFTSHIFT || key == keys::KEY_RIGHTSHIFT {
                    state.shift_held = true;
                    state.shift_dirty = false;
                    return;
                }

                if is_modifier_only_key(key) {
                    return;
                }

                if state.shift_held {
                    state.shift_dirty = true;
                }
                //if key == keys::KEY_ESC { std::process::exit(0); }

                state.last_key_time = time;
                let mods = state.current_mods;
                let actions = state.runtime.process_key(key, mods);
                state.flush_actions(&actions);
                state.overlay_dirty = true;

                // ── 设置 key repeat（fcitx5 同款逻辑）──
                // 只对可重复的键设置（字母、数字、符号、backspace 等）
                if is_repeatable_key(key) && state.repeat_delay_ms > 0 {
                    state.repeat_key = Some(key);
                    state.repeat_next = Some(
                        std::time::Instant::now()
                            + std::time::Duration::from_millis(
                                state.repeat_delay_ms as u64
                            )
                    );
                }
            }

            Event::Modifiers { mods_depressed, mods_latched, mods_locked, group, .. } => {
                let shift_now = mods_depressed & keys::MOD_SHIFT != 0;
                if state.shift_held && !shift_now {
                    if !state.shift_dirty && state.has_cursor_info {
                        if state.runtime.is_chinese() {
                            eprintln!("dust: shift toggle → english");
                            let raw = state.runtime.get_raw_input();
                            state.runtime.deactivate();
                            let mut actions = Vec::new();
                            if !raw.is_empty() {
                                actions.push(Action::CommitText(raw));
                            }
                            actions.push(Action::ClearPreedit);
                            state.flush_actions(&actions);
                            if let Some(overlay) = &mut state.overlay {
                                overlay.hide();
                            }
                        } else {
                            eprintln!("dust: shift toggle → chinese");
                            state.runtime.toggle_to_chinese();
                        }
                        state.overlay_dirty = true;
                    }
                    state.shift_held = false;
                    state.shift_dirty = false;
                }
                state.current_mods = mods_depressed;
                if let Some(vk) = &state.vk {
                    vk.modifiers(mods_depressed, mods_latched, mods_locked, group);
                }
            }
            Event::RepeatInfo { rate, delay } => {
                // compositor 告知重复参数
                // delay: 首次延迟(ms), rate: 每秒重复次数
                state.repeat_delay_ms = delay.max(0) as u32;
                state.repeat_rate_ms = if rate > 0 {
                    (1000 / rate) as u32
                } else {
                    0  // rate<=0 表示禁用 repeat
                };
                eprintln!("dust: repeat_info delay={}ms rate={}/s interval={}ms",
                    delay, rate, state.repeat_rate_ms);
            }
        }
    }
}

/// 判断一个键是否应该长按重复
/// 排除 shift、ctrl、alt、capslock 等修饰键和非字符键
fn is_repeatable_key(key: u32) -> bool {
    match key {
        keys::KEY_LEFTSHIFT | keys::KEY_RIGHTSHIFT   // 42, 54
        | 29 | 97                                     // Ctrl L/R
        | 56 | 100                                    // Alt L/R
        | 58 | 69                                     // CapsLock / NumLock
        | 125 | 126                                   // Super L/R
        | keys::KEY_LEFTBRACE | keys::KEY_RIGHTBRACE  // 26, 27
        | keys::KEY_SEMICOLON | keys::KEY_APOSTROPHE  // 39, 40
        | keys::KEY_COMMA | keys::KEY_DOT             // 51, 52
        | keys::KEY_SLASH                             // 53
        => false,
        _ => true,
    }
}

fn is_modifier_only_key(key: u32) -> bool {
    matches!(key, 29 | 97 | 56 | 100 | 125 | 126)
    // LCtrl(29) RCtrl(97) LAlt(56) RAlt(100) SuperL(125) SuperR(126)
}

impl Dispatch<ZwpInputPopupSurfaceV2, ()> for WaylandState {
    fn event(
        state: &mut Self, _: &ZwpInputPopupSurfaceV2,
        event: <ZwpInputPopupSurfaceV2 as wayland_client::Proxy>::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>,
    ) {
        use self::zwp_input_popup_surface_v2::Event;
        match event {
            Event::TextInputRectangle { x, y, width, height } => {
                if let Some(overlay) = &mut state.overlay {
                    overlay.set_cursor_rect(x, y, width, height);
                }
                if !state.has_cursor_info {
                    state.has_cursor_info = true;
                    state.overlay_dirty = true;
                }
            }
        }
    }
}

impl Dispatch<ZwpVirtualKeyboardManagerV1, ()> for WaylandState {
    fn event(_: &mut Self, _: &ZwpVirtualKeyboardManagerV1,
        _: <ZwpVirtualKeyboardManagerV1 as wayland_client::Proxy>::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<ZwpVirtualKeyboardV1, ()> for WaylandState {
    fn event(_: &mut Self, _: &ZwpVirtualKeyboardV1,
        _: <ZwpVirtualKeyboardV1 as wayland_client::Proxy>::Event,
        _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

use crate::engine::action::Action;

pub fn apply_actions(im: &ZwpInputMethodV2, actions: &[Action], done_count: u32) {
    let mut dirty = false;
    for a in actions {
        match a {
            Action::SetPreedit { text, begin, end } => {
                im.set_preedit_string(text.clone(), *begin, *end);
                dirty = true;
            }
            Action::CommitText(text) => {
                im.commit_string(text.clone());
                // 删除这行：im.set_preedit_string(String::new(), 0, 0);
                // 让随后的 ClearPreedit action 或者下一个循环去处理预编辑框清理
                dirty = true;
            }
            Action::ClearPreedit => {
                im.set_preedit_string(String::new(), 0, 0);
                dirty = true;
            }
            _ => {}
        }
    }
    if dirty { im.commit(done_count); }
}
