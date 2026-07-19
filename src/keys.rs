pub const KEY_ESC: u32 = 1;
pub const KEY_1: u32 = 2;
pub const KEY_2: u32 = 3;
pub const KEY_3: u32 = 4;
pub const KEY_4: u32 = 5;
pub const KEY_5: u32 = 6;
pub const KEY_6: u32 = 7;
pub const KEY_7: u32 = 8;
pub const KEY_8: u32 = 9;
pub const KEY_9: u32 = 10;
pub const KEY_BACKSPACE: u32 = 14;
pub const KEY_BACKSLASH: u32 = 43;
pub const KEY_TAB: u32 = 15;
pub const KEY_Q: u32 = 16;
pub const KEY_W: u32 = 17;
pub const KEY_E: u32 = 18;
pub const KEY_R: u32 = 19;
pub const KEY_T: u32 = 20;
pub const KEY_Y: u32 = 21;
pub const KEY_U: u32 = 22;
pub const KEY_I: u32 = 23;
pub const KEY_O: u32 = 24;
pub const KEY_P: u32 = 25;
pub const KEY_ENTER: u32 = 28;
pub const KEY_A: u32 = 30;
pub const KEY_S: u32 = 31;
pub const KEY_D: u32 = 32;
pub const KEY_F: u32 = 33;
pub const KEY_G: u32 = 34;
pub const KEY_H: u32 = 35;
pub const KEY_J: u32 = 36;
pub const KEY_K: u32 = 37;
pub const KEY_L: u32 = 38;
pub const KEY_Z: u32 = 44;
pub const KEY_X: u32 = 45;
pub const KEY_C: u32 = 46;
pub const KEY_V: u32 = 47;
pub const KEY_B: u32 = 48;
pub const KEY_N: u32 = 49;
pub const KEY_M: u32 = 50;
pub const KEY_SPACE: u32 = 57;
pub const KEY_MINUS: u32 = 12;
pub const KEY_EQUALS: u32 = 13;

pub const MOD_CTRL: u32 = 4;
pub const MOD_SHIFT: u32 = 1;

pub const KEY_SEMICOLON: u32 = 39;
pub const KEY_APOSTROPHE: u32 = 40;
pub const KEY_COMMA: u32 = 51;
pub const KEY_DOT: u32 = 52;
pub const KEY_SLASH: u32 = 53;
pub const KEY_LEFTBRACE: u32 = 26;
pub const KEY_LEFTSHIFT: u32 = 42;
pub const KEY_RIGHTBRACE: u32 = 27;
pub const KEY_RIGHTSHIFT: u32 = 54;

pub const KEY_LEFT: u32 = 105;
pub const KEY_RIGHT: u32 = 106;
pub const KEY_HOME: u32 = 102;
pub const KEY_END: u32 = 107;
pub const KEY_DELETE: u32 = 111; // 可选：支持 Delete 键向后删除


pub fn evdev_to_char(key: u32) -> Option<char> {
    match key {
        KEY_Q => Some('q'), KEY_W => Some('w'), KEY_E => Some('e'),
        KEY_R => Some('r'), KEY_T => Some('t'), KEY_Y => Some('y'),
        KEY_U => Some('u'), KEY_I => Some('i'), KEY_O => Some('o'),
        KEY_P => Some('p'),
        KEY_A => Some('a'), KEY_S => Some('s'), KEY_D => Some('d'),
        KEY_F => Some('f'), KEY_G => Some('g'), KEY_H => Some('h'),
        KEY_J => Some('j'), KEY_K => Some('k'), KEY_L => Some('l'),
        KEY_Z => Some('z'), KEY_X => Some('x'), KEY_C => Some('c'),
        KEY_V => Some('v'), KEY_B => Some('b'), KEY_N => Some('n'),
        KEY_M => Some('m'),
        _ => None,
    }
}
