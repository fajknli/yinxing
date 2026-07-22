// yinxing - 音形码输入法
// Copyright (C) 2026  fajknli
// Licensed under the GNU AGPL v3. See LICENSE for details.

#[derive(Debug, Clone)]
pub enum Action {
    SetPreedit { text: String, begin: i32, end: i32 },
    CommitText(String),
    ClearPreedit,
    ForwardKey { key: u32, mods: u32 },
}
