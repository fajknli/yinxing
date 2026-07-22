// yinxing - 音形码输入法
// Copyright (C) 2026  fajknli
// Licensed under the GNU AGPL v3. See LICENSE for details.

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImeState {
    English,
    Chinese,
    Composing,
}
