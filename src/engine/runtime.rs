use std::rc::Rc;
use rustc_hash::FxHashSet;
use crate::engine::action::Action;
use crate::engine::learner::Learner;
use crate::engine::lookup::LookupTable;
use crate::keys;
use crate::state::ImeState;
use crate::candi;

#[derive(Clone)]
struct DpNode {
    score: f64,
    word: Rc<str>,
    prev: Option<(usize, usize)>,
}

#[derive(Clone, Debug)]
struct CandidateItem {
    text: String,
    consumed: usize,
    score: f64,
    code: String,
    is_dp_split: bool,
}

#[derive(Clone, Debug)]
struct Block {
    code: String,
    text: String,
    confirmed: bool,
    saved_text: String,
}

pub struct RuntimeState {
    blocks: Vec<Block>,
    active_idx: usize,
    cursor: usize,
    current_selection: String,
    current_consumed: usize,
    last_pending_code: String,
    mode: ImeState,
    table: LookupTable,
    page: usize,
    candidates: Vec<CandidateItem>,
    learner: Learner,
    custom_phrases_path: String,
    cfg: crate::config::BehaviorConfig,
    highlight_idx: usize,
}

impl RuntimeState {
    pub fn new(
        table: LookupTable,
        user_data_path: String,
        custom_phrases_path: String,
        cfg: crate::config::BehaviorConfig,
    ) -> Self {
        let learner = Learner::new(user_data_path);
        let mut runtime = Self {
            blocks: Vec::new(),
            active_idx: usize::MAX,
            cursor: 0,
            current_selection: String::new(),
            current_consumed: 0,
            last_pending_code: String::new(),
            mode: ImeState::English,
            table,
            page: 0,
            candidates: Vec::new(),
            learner,
            custom_phrases_path,
            cfg,
            highlight_idx: 0,
        };
        runtime.load_custom_phrases();
        runtime
    }

    fn load_custom_phrases(&mut self) {
        if let Ok(content) = std::fs::read_to_string(&self.custom_phrases_path) {
            for line in content.lines() {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() == 2 {
                    self.table.add_custom_phrase(parts[0], parts[1]);
                }
            }
        }
    }

    fn save_custom_phrase(&self, text: &str, code: &str) {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.custom_phrases_path)
        {
            let _ = writeln!(f, "{}\t{}", text, code);
        }
    }

    pub fn process_key(&mut self, key: u32, mods: u32) -> Vec<Action> {
        if self.mode == ImeState::English {
            if is_toggle(key, mods) {
                self.mode = ImeState::Chinese;
                return vec![];
            }
            return vec![Action::ForwardKey { key, mods }];
        }

        if is_toggle(key, mods) {
            let mut a = vec![];
            if !self.blocks.is_empty() {
                a.extend(self.commit_staging_buffer());
            }
            self.clear();
            self.mode = ImeState::English;
            return a;
        }

        if (mods & keys::MOD_SHIFT) != 0 {
            if keys::evdev_to_char(key).is_some() {
                let mut a = vec![];
                if !self.blocks.is_empty() {
                    a.extend(self.commit_staging_buffer());
                }
                self.clear();
                self.mode = ImeState::English;
                a.push(Action::ForwardKey { key, mods });
                return a;
            }
        }

        if (mods & !keys::MOD_SHIFT) != 0 {
            let mut a = vec![];
            if !self.blocks.is_empty() {
                a.extend(self.commit_staging_buffer());
            } else {
                a.push(Action::ClearPreedit);
            }
            self.clear();
            a.push(Action::ForwardKey { key, mods });
            return a;
        }

        match self.mode {
            ImeState::Chinese => self.on_chinese(key, mods),
            ImeState::Composing => self.on_composing(key, mods),
            ImeState::English => unreachable!(),
        }
    }

    fn on_chinese(&mut self, key: u32, mods: u32) -> Vec<Action> {
        if let Some(ch) = keys::evdev_to_char(key) {
            self.blocks.push(Block {
                code: ch.to_string(),
                text: String::new(),
                confirmed: false,
                saved_text: String::new(),
            });
            self.active_idx = self.blocks.len() - 1;
            self.cursor = ch.len_utf8();
            self.mode = ImeState::Composing;
            self.page = 0;
            self.build_preedit()
        } else if key == keys::KEY_SPACE {
            if self.blocks.is_empty() && self.current_selection.is_empty() {
                return vec![Action::CommitText(" ".into())];
            }
            self.commit_staging_buffer()
        } else if let Some(p) = punct_char(key, (mods & keys::MOD_SHIFT) != 0, &self.cfg.punctuation) {
            if !self.blocks.is_empty() || !self.current_selection.is_empty() {
                self.commit_with_suffix(p)
            } else {
                vec![Action::CommitText(p.into())]
            }
        } else {
            vec![Action::ForwardKey { key, mods }]
        }
    }

    fn on_composing(&mut self, key: u32, mods: u32) -> Vec<Action> {
        if self.blocks.is_empty() {
            self.mode = ImeState::Chinese;
            return self.on_chinese(key, mods);
        }

        let shifted = (mods & keys::MOD_SHIFT) != 0;

        match key {
            keys::KEY_BACKSPACE => {
                if self.blocks.is_empty() { return self.build_preedit(); }
                if self.active_idx >= self.blocks.len() { self.active_idx = self.blocks.len() - 1; }

                let start = self.get_block_start_offset(self.active_idx);
                let len = self.blocks[self.active_idx].code.len();
                let local_off = self.cursor.saturating_sub(start).min(len);
                let is_zh_mode = !self.blocks[self.active_idx].text.is_empty();

                if is_zh_mode && local_off == len {
                    self.blocks[self.active_idx].saved_text = self.blocks[self.active_idx].text.clone();
                    self.blocks[self.active_idx].confirmed = false;
                    self.blocks[self.active_idx].text.clear();
                    self.current_selection.clear();
                    self.current_consumed = 0;
                    self.last_pending_code.clear();
                    self.page = 0;
                    return self.build_preedit();
                }

                if local_off == 0 {
                    if self.active_idx == 0 { return self.build_preedit(); }
                    self.active_idx -= 1;
                    let prev_len = self.blocks[self.active_idx].code.len();

                    if !self.blocks[self.active_idx].text.is_empty() {
                        self.blocks[self.active_idx].saved_text = self.blocks[self.active_idx].text.clone();
                        self.blocks[self.active_idx].confirmed = false;
                        self.blocks[self.active_idx].text.clear();
                        self.cursor = self.get_block_start_offset(self.active_idx) + prev_len;
                        self.current_selection.clear();
                        self.current_consumed = 0;
                        self.last_pending_code.clear();
                        self.page = 0;
                        return self.build_preedit();
                    }
                    let char_len = self.blocks[self.active_idx].code[..prev_len].chars().last().unwrap().len_utf8();
                    self.blocks[self.active_idx].code.replace_range(prev_len - char_len..prev_len, "");
                    self.blocks[self.active_idx].confirmed = false;
                    self.blocks[self.active_idx].text.clear();
                    self.cursor -= char_len;
                } else {
                    let char_len = self.blocks[self.active_idx].code[..local_off].chars().last().unwrap().len_utf8();
                    self.blocks[self.active_idx].code.replace_range(local_off - char_len..local_off, "");
                    self.blocks[self.active_idx].confirmed = false;
                    self.blocks[self.active_idx].text.clear();
                    self.cursor -= char_len;
                }

                let before_len = self.blocks.len();
                self.blocks.retain(|b| !b.code.is_empty());
                if self.blocks.len() < before_len {
                    self.merge_free_blocks();
                }

                if self.blocks.is_empty() {
                    self.clear();
                    self.mode = ImeState::Chinese;
                    return vec![Action::ClearPreedit];
                }

                if self.active_idx >= self.blocks.len() {
                    self.active_idx = self.blocks.len() - 1;
                }

                self.current_selection.clear();
                self.current_consumed = 0;
                self.last_pending_code.clear();

                self.page = 0;
                self.build_preedit()
            }

            keys::KEY_DELETE => {
                if self.blocks.is_empty() { return self.build_preedit(); }
                if self.active_idx >= self.blocks.len() { self.active_idx = self.blocks.len() - 1; }

                let start = self.get_block_start_offset(self.active_idx);
                let len = self.blocks[self.active_idx].code.len();
                let local_off = self.cursor.saturating_sub(start).min(len);
                let is_zh_mode = !self.blocks[self.active_idx].text.is_empty();

                if is_zh_mode && local_off == 0 {
                    self.blocks[self.active_idx].saved_text = self.blocks[self.active_idx].text.clone();
                    self.blocks[self.active_idx].confirmed = false;
                    self.blocks[self.active_idx].text.clear();
                    self.current_selection.clear();
                    self.current_consumed = 0;
                    self.last_pending_code.clear();
                    self.page = 0;
                    return self.build_preedit();
                }

                if local_off == len {
                    if self.active_idx + 1 >= self.blocks.len() { return self.build_preedit(); }
                    self.active_idx += 1;
                    if !self.blocks[self.active_idx].text.is_empty() {
                        self.blocks[self.active_idx].saved_text = self.blocks[self.active_idx].text.clone();
                        self.blocks[self.active_idx].confirmed = false;
                        self.blocks[self.active_idx].text.clear();
                        self.cursor = self.get_block_start_offset(self.active_idx);
                        self.current_selection.clear();
                        self.current_consumed = 0;
                        self.last_pending_code.clear();
                        self.page = 0;
                        return self.build_preedit();
                    }
                    let char_len = self.blocks[self.active_idx].code.chars().next().unwrap().len_utf8();
                    self.blocks[self.active_idx].code.replace_range(0..char_len, "");
                    self.blocks[self.active_idx].confirmed = false;
                    self.blocks[self.active_idx].text.clear();
                } else {
                    let char_len = self.blocks[self.active_idx].code[local_off..].chars().next().unwrap().len_utf8();
                    self.blocks[self.active_idx].code.replace_range(local_off..local_off + char_len, "");
                    self.blocks[self.active_idx].confirmed = false;
                    self.blocks[self.active_idx].text.clear();
                }

                let before_len = self.blocks.len();
                self.blocks.retain(|b| !b.code.is_empty());
                if self.blocks.len() < before_len {
                    self.merge_free_blocks();
                }

                if self.blocks.is_empty() {
                    self.clear();
                    self.mode = ImeState::Chinese;
                    return vec![Action::ClearPreedit];
                }

                if self.active_idx >= self.blocks.len() {
                    self.active_idx = self.blocks.len() - 1;
                }

                self.current_selection.clear();
                self.current_consumed = 0;
                self.last_pending_code.clear();

                self.page = 0;
                self.build_preedit()
            }

            keys::KEY_LEFT => {
                if self.blocks.is_empty() { return self.build_preedit(); }
                if self.active_idx >= self.blocks.len() { self.active_idx = self.blocks.len() - 1; }

                let start = self.get_block_start_offset(self.active_idx);
                let len = self.blocks[self.active_idx].code.len();
                let local_off = self.cursor.saturating_sub(start).min(len);
                let is_zh_mode = !self.blocks[self.active_idx].text.is_empty();

                if is_zh_mode {
                    if self.active_idx > 0 {
                        self.blocks[self.active_idx].confirmed = true;
                        self.current_selection.clear();
                        self.current_consumed = 0;
                        self.last_pending_code.clear();

                        self.active_idx -= 1;
                        let prev_len = self.blocks[self.active_idx].code.len();
                        self.cursor = self.get_block_start_offset(self.active_idx) + prev_len;
                    } else {
                        self.cursor = start;
                    }
                } else {
                    if local_off > 0 {
                        let char_len = self.blocks[self.active_idx].code[..local_off].chars().last().unwrap().len_utf8();
                        self.cursor -= char_len;
                    } else if self.active_idx > 0 {
                        self.lock_block(self.active_idx);
                        self.active_idx -= 1;
                        let prev_len = self.blocks[self.active_idx].code.len();
                        self.cursor = self.get_block_start_offset(self.active_idx) + prev_len;
                    }
                }
                self.page = 0;
                self.build_preedit()
            }

            keys::KEY_RIGHT => {
                if self.blocks.is_empty() { return self.build_preedit(); }
                if self.active_idx >= self.blocks.len() { self.active_idx = self.blocks.len() - 1; }

                let start = self.get_block_start_offset(self.active_idx);
                let len = self.blocks[self.active_idx].code.len();
                let local_off = self.cursor.saturating_sub(start).min(len);
                let is_zh_mode = !self.blocks[self.active_idx].text.is_empty();

                if is_zh_mode {
                    if self.active_idx + 1 < self.blocks.len() {
                        self.blocks[self.active_idx].confirmed = true;
                        self.current_selection.clear();
                        self.current_consumed = 0;
                        self.last_pending_code.clear();

                        self.active_idx += 1;
                        let next_len = self.blocks[self.active_idx].code.len();
                        self.cursor = self.get_block_start_offset(self.active_idx) + next_len;
                    } else {
                        self.cursor = start + len;
                    }
                } else {
                    if local_off < len {
                        let char_len = self.blocks[self.active_idx].code[local_off..].chars().next().map(|c| c.len_utf8()).unwrap_or(0);
                        if char_len > 0 {
                            self.cursor += char_len;
                        }
                    } else if self.active_idx + 1 < self.blocks.len() {
                        self.lock_block(self.active_idx);
                        self.active_idx += 1;
                        let next_len = self.blocks[self.active_idx].code.len();
                        self.cursor = self.get_block_start_offset(self.active_idx) + next_len;
                    }
                }
                self.page = 0;
                self.build_preedit()
            }

            keys::KEY_HOME => {
                if self.blocks.is_empty() { return self.build_preedit(); }
                if self.cursor > 0 || self.active_idx != 0 {
                    self.cursor = 0;
                    if self.active_idx != 0 {
                        let is_zh_mode = !self.blocks[self.active_idx].text.is_empty();
                        if is_zh_mode {
                            self.blocks[self.active_idx].confirmed = true;
                        } else {
                            self.lock_block(self.active_idx);
                        }
                        self.active_idx = 0;
                        self.current_selection.clear();
                        self.current_consumed = 0;
                        self.last_pending_code.clear();
                    }
                }
                self.page = 0;
                self.build_preedit()
            }

            keys::KEY_END => {
                if self.blocks.is_empty() { return self.build_preedit(); }
                let last_idx = self.blocks.len() - 1;
                let total_len: usize = self.blocks.iter().map(|b| b.code.len()).sum();

                if self.cursor < total_len || self.active_idx != last_idx {
                    self.cursor = total_len;
                    if self.active_idx != last_idx {
                        let is_zh_mode = !self.blocks[self.active_idx].text.is_empty();
                        if is_zh_mode {
                            self.blocks[self.active_idx].confirmed = true;
                        } else {
                            self.lock_block(self.active_idx);
                        }
                        self.active_idx = last_idx;
                        self.current_selection.clear();
                        self.current_consumed = 0;
                        self.last_pending_code.clear();
                    }
                }
                self.page = 0;
                self.build_preedit()
            }

            keys::KEY_ESC => {
                self.clear();
                self.mode = ImeState::Chinese;
                vec![Action::ClearPreedit]
            }

            keys::KEY_ENTER => {
                let (raw, _) = self.preedit_display_info();
                self.clear();
                self.mode = ImeState::Chinese;
                if raw.is_empty() {
                    vec![Action::ClearPreedit]
                } else {
                    vec![Action::ClearPreedit, Action::CommitText(raw)]
                }
            }

            keys::KEY_SPACE => {
                if self.blocks.is_empty() && self.current_selection.is_empty() {
                    return vec![Action::CommitText(" ".into())];
                }
                self.commit_staging_buffer()
            }

            keys::KEY_1..=keys::KEY_9 if !shifted => {
                let n = (key - keys::KEY_1) as usize + self.page * self.cfg.page_size;
                self.commit_nth(n)
            }

            keys::KEY_TAB | keys::KEY_EQUALS if !shifted => {
                self.next_page();
                self.build_preedit()
            }

            keys::KEY_MINUS if !shifted => {
                self.prev_page();
                self.build_preedit()
            }

            k if punct_char(k, shifted, &self.cfg.punctuation).is_some() => {
                let punct = punct_char(k, shifted, &self.cfg.punctuation).unwrap();
                self.commit_with_suffix(punct)
            }

            _ => {
                if let Some(ch) = keys::evdev_to_char(key) {
                    if self.blocks.is_empty() {
                        self.blocks.push(Block {
                            code: String::new(),
                            text: String::new(),
                            confirmed: false,
                            saved_text: String::new(),
                        });
                        self.active_idx = 0;
                    }
                    if self.active_idx >= self.blocks.len() {
                        self.active_idx = self.blocks.len() - 1;
                    }

                    let target_idx = self.active_idx;
                    let start = self.get_block_start_offset(target_idx);
                    let len = self.blocks[target_idx].code.len();
                    let local_off = self.cursor.saturating_sub(start).min(len);

                    let is_at_end_of_block = local_off == len;
                    let is_at_start_of_block = local_off == 0;
                    let is_zh_mode = !self.blocks[target_idx].text.is_empty();

                    let mut should_create_new = false;
                    let mut insert_pos = target_idx + 1;

                    if is_at_end_of_block && is_zh_mode {
                        self.blocks[target_idx].confirmed = true;
                        should_create_new = true;
                        insert_pos = target_idx + 1;
                    }
                    else if is_at_start_of_block && is_zh_mode {
                        should_create_new = true;
                        insert_pos = target_idx;
                    }

                    if should_create_new {
                        let new_block = Block {
                            code: ch.to_string(),
                            text: String::new(),
                            confirmed: false,
                            saved_text: String::new(),
                        };
                        self.blocks.insert(insert_pos, new_block);
                        self.active_idx = insert_pos;
                        self.cursor = self.get_block_start_offset(self.active_idx) + ch.len_utf8();
                        self.current_selection.clear();
                        self.current_consumed = 0;
                    } else {
                        let block = &mut self.blocks[target_idx];
                        block.confirmed = false;
                        block.text.clear();
                        block.code.insert(local_off, ch);
                        self.cursor += ch.len_utf8();
                        self.active_idx = target_idx;
                    }
                    self.page = 0;
                    self.build_preedit()
                } else {
                    let mut actions = self.commit_staging_buffer();
                    actions.push(Action::ForwardKey { key, mods });
                    actions
                }
            }
        }
    }

    fn build_preedit(&mut self) -> Vec<Action> {
        self.candidates.clear();

        if self.blocks.is_empty() {
            return vec![Action::ClearPreedit];
        }

        if self.active_idx >= self.blocks.len() {
            self.active_idx = self.blocks.len() - 1;
        }

        let active_block = &self.blocks[self.active_idx];

        let code = active_block.code.clone();
        let n = code.len();

        if n == 0 {
            self.current_selection.clear();
            self.current_consumed = 0;
            self.last_pending_code.clear();
        } else if n > 0 && n < 3 {
            let mut entries = self.table.prefix_chars(&code, 200);
            for entry in &mut entries {
                entry.weight += self.learner.get_boost(entry.text.as_ref());
            }
            entries.sort_by(|a, b| b.weight.cmp(&a.weight));
            let mut seen = FxHashSet::default();
            entries.retain(|e| seen.insert(e.text.clone()));
            self.candidates = entries.into_iter().map(|e| CandidateItem {
                text: e.text.to_string(),
                consumed: n,
                score: 0.0,
                code: e.code.to_string(),
                is_dp_split: false,
            }).collect();
        } else if n >= 3 {
            let mut dp: Vec<Vec<DpNode>> = vec![vec![]; n + 1];
            dp[0].push(DpNode { score: 0.0, word: Rc::from(""), prev: None });

            for i in 3..=n {
                for &len in [3, 4, 5, 6, 7, 8, 9, 10, 12, 15, 18, 21, 24].iter() {
                    if len > i { continue; }
                    let start = i - len;
                    let segment = &code[start..i];

                    if dp[start].is_empty() { continue; }

                    let mut entries = self.table.get_all_for_code(segment);

                    if len >= 6 && len % 3 == 0 {
                        let num_chars = len / 3;
                        let mut sub_codes = Vec::new();
                        for j in 0..num_chars {
                            sub_codes.push(segment[j*3..(j+1)*3].to_string());
                        }
                        if let Some(isir_code) = encode_word_from_codes(&sub_codes) {
                            entries.extend(self.table.get_all_for_code(&isir_code));
                        }
                    }

                    let prev_nodes = dp[start].clone();

                    for (p_idx, prev_node) in prev_nodes.iter().enumerate() {
                        for entry in &entries {
                            let base_score = entry.weight as f64 + self.learner.get_boost(entry.text.as_ref()) as f64;
                            let new_score = prev_node.score + base_score;

                            dp[i].push(DpNode {
                                score: new_score,
                                word: Rc::clone(&entry.text),
                                prev: Some((start, p_idx)),
                            });
                        }
                    }
                }

                dp[i].sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
                dp[i].truncate(5);
            }

            for i in 3..=n {
                for node in &dp[i] {
                    let mut parts: Vec<&str> = Vec::new();
                    let mut curr = node;
                    parts.push(curr.word.as_ref());

                    while let Some((s, p)) = curr.prev {
                        curr = &dp[s][p];
                        parts.push(curr.word.as_ref());
                    }

                    parts.reverse();
                    let text = parts.join("");

                    if !text.is_empty() && !self.candidates.iter().any(|c| c.text == text) {
                        self.candidates.push(CandidateItem {
                            text,
                            consumed: i,
                            score: node.score,
                            code: code[..i].to_string(),
                            is_dp_split: true,
                        });
                    }
                }
            }
        }

        self.candidates.sort_by(|a, b| {
            b.consumed.cmp(&a.consumed)
                .then_with(|| b.score.partial_cmp(&a.score).unwrap())
        });

        if self.candidates.len() > self.cfg.max_candidates {
            self.candidates.truncate(self.cfg.max_candidates);
        }

        if code != self.last_pending_code {
            if !self.blocks[self.active_idx].text.is_empty() {
                self.current_selection = self.blocks[self.active_idx].text.clone();
                if let Some(c) = self.candidates.iter().find(|c| c.text == self.blocks[self.active_idx].text) {
                    self.current_consumed = c.consumed;
                } else {
                    self.current_consumed = 0;
                }
            } else {
                self.current_selection.clear();
                self.current_consumed = 0;
            }
            self.last_pending_code = code.clone();
        }

        let max_page = self.candidates.len().saturating_sub(1) / self.cfg.page_size;
        if self.page > max_page { self.page = max_page; }

        // ─── 新增：智能计算高亮索引 ───
        let offset = if self.blocks.is_empty() { 0 } else { 1 }; // 如果有预编辑区，候选词索引从 1 开始
        self.highlight_idx = if self.candidates.is_empty() {
            0
        } else {
            let mut idx = offset; // 默认高亮第一个候选词
            if !self.current_selection.is_empty() {
                let start = self.page * self.cfg.page_size;
                let end = (start + self.cfg.page_size).min(self.candidates.len());
                if start < end {
                    // 如果用户选过的词还在当前页，就高亮它
                    if let Some(rel_idx) = self.candidates[start..end].iter().position(|c| c.text == self.current_selection) {
                        idx = offset + rel_idx;
                    }
                }
            }
            idx
        };
        // ─────────────────────────────

        let (preedit, cursor_pos) = self.preedit_display_info();
        if preedit.is_empty() {
            return vec![Action::ClearPreedit];
        }
        let c = cursor_pos as i32;
        vec![Action::SetPreedit { text: preedit, begin: c, end: c }]
    }

    pub fn commit_nth(&mut self, n: usize) -> Vec<Action> {
        if let Some(cand) = self.candidates.get(n).cloned() {
            if self.active_idx >= self.blocks.len() { return self.build_preedit(); }

            {
                let target_idx = self.active_idx;
                let consumed = cand.consumed;
                let current_code_len = self.blocks[target_idx].code.len();

                if consumed > 0 && consumed < current_code_len {
                    let remaining = self.blocks[target_idx].code[consumed..].to_string();
                    self.blocks[target_idx].code.truncate(consumed);
                    self.blocks[target_idx].text = cand.text.clone();
                    self.blocks[target_idx].confirmed = false;
                    self.blocks[target_idx].saved_text.clear();

                    let new_block = Block {
                        code: remaining,
                        text: String::new(),
                        confirmed: false,
                        saved_text: String::new(),
                    };
                    self.blocks.insert(target_idx + 1, new_block);

                    // 【关键修复】：将焦点和光标移动到新分裂出的 Block！
                    self.active_idx = target_idx + 1;
                } else {
                    self.blocks[target_idx].text = cand.text.clone();
                    self.blocks[target_idx].confirmed = false;
                    self.blocks[target_idx].saved_text.clear();
                    // 如果没有分裂，焦点依然在当前 block
                    self.active_idx = target_idx;
                }
            }

            // 光标计算会自动使用新的 active_idx
            self.cursor = self.get_block_start_offset(self.active_idx) + self.blocks[self.active_idx].code.len();

            self.current_selection = cand.text.clone();
            self.current_consumed = cand.consumed;

            self.page = 0;
            self.build_preedit()
        } else {
            self.build_preedit()
        }
    }

    fn commit_with_suffix(&mut self, suffix: &str) -> Vec<Action> {
        let mut text = String::new();

        for (i, b) in self.blocks.iter().enumerate() {
            if i == self.active_idx {
                if !b.text.is_empty() {
                    text.push_str(&b.text);
                } else if !self.current_selection.is_empty() {
                    text.push_str(&self.current_selection);
                } else if let Some(best) = self.candidates.first() {
                    text.push_str(&best.text);
                } else {
                    text.push_str(&b.code);
                }
            } else if !b.text.is_empty() {
                text.push_str(&b.text);
            } else {
                text.push_str(&b.code);
            }
        }

        if text.is_empty() && suffix.is_empty() {
            return vec![];
        }

        if !text.is_empty() {
            let char_count = text.chars().count();
            if char_count >= self.cfg.min_phrase_chars && char_count <= self.cfg.max_phrase_chars {
                if let Some(sub_codes) = self.table.get_codes_for_text(&text) {
                    if let Some(new_code) = encode_word_from_codes(&sub_codes) {
                        let exists_in_main = self.table.word_exists(&new_code, &text);
                        if !exists_in_main {
                            let is_new = self.table.add_custom_phrase(&text, &new_code);
                            if is_new {
                                self.save_custom_phrase(&text, &new_code);
                            }
                        }
                    }
                }
            }
            self.learner.record(&text);
        }

        text.push_str(suffix);

        self.clear();
        self.mode = ImeState::Chinese;
        vec![Action::CommitText(text), Action::ClearPreedit]
    }

    pub fn commit_staging_buffer(&mut self) -> Vec<Action> {
        self.commit_with_suffix("")
    }

    fn try_create_phrase(&mut self, cand: &CandidateItem) {
        if !cand.is_dp_split {
            return;
        }

        let code_len = cand.code.len();
        if code_len >= 6 && code_len % 3 == 0 {
            let num_chars = code_len / 3;
            if num_chars >= 2 && num_chars <= 8 {
                let mut sub_codes = Vec::new();
                for i in 0..num_chars {
                    sub_codes.push(cand.code[i*3..(i+1)*3].to_string());
                }
                if let Some(new_code) = encode_word_from_codes(&sub_codes) {
                    let exists_in_main = self.table.word_exists(&new_code, &cand.text);
                    if !exists_in_main {
                        let is_new = self.table.add_custom_phrase(&cand.text, &new_code);
                        if is_new {
                            self.save_custom_phrase(&cand.text, &new_code);
                        }
                    }
                }
            }
        }
    }

    fn clear(&mut self) {
        self.blocks.clear();
        self.active_idx = usize::MAX;
        self.cursor = 0;
        self.current_selection.clear();
        self.current_consumed = 0;
        self.last_pending_code.clear();
        self.candidates.clear();
        self.page = 0;
        self.highlight_idx = 0;
    }

    fn next_page(&mut self) {
        if (self.page + 1) * self.cfg.page_size < self.candidates.len() {
            self.page += 1;
        }
    }

    fn prev_page(&mut self) {
        if self.page > 0 {
            self.page -= 1;
        }
    }

    pub fn activate(&mut self) {
        self.clear();
        self.mode = ImeState::English;
    }

    pub fn toggle_to_chinese(&mut self) {
        self.mode = ImeState::Chinese;
    }

    pub fn deactivate(&mut self) {
        self.clear();
        self.mode = ImeState::English;
    }

    pub fn get_raw_input(&self) -> String {
        self.blocks.iter().map(|b| b.code.as_str()).collect()
    }

    pub fn is_chinese(&self) -> bool { self.mode != ImeState::English }
    pub fn has_candidates(&self) -> bool { !self.candidates.is_empty() }
    pub fn sync_candidates(&self) {}
    pub fn flush_user_buffer(&mut self) {}

    pub fn candidate_items(&self) -> Vec<candi::Item> {
        let mut items = Vec::new();

        let mut preview = String::new();
        for (i, b) in self.blocks.iter().enumerate() {
            if i == self.active_idx {
                if !self.current_selection.is_empty() {
                    preview.push_str(&self.current_selection);
                } else {
                    preview.push_str(&b.code);
                }
            } else if !b.text.is_empty() {
                preview.push_str(&b.text);
            } else {
                preview.push_str(&b.code);
            }
        }

        if !preview.is_empty() {
            items.push(candi::Item::text(preview));
        }

        let start = self.page * self.cfg.page_size;
        let end = (start + self.cfg.page_size).min(self.candidates.len());
        if start < end {
            items.extend(self.candidates[start..end].iter().enumerate().map(|(i, c)| {
                let prefix = format!("{}.", i + 1);
                candi::Item::with_prefix(prefix, c.text.clone())
            }));
        }

        items
    }

    pub fn get_highlight_idx(&self) -> usize {
        self.highlight_idx
    }

    fn preedit_display_info(&self) -> (String, usize) {
        let mut s = String::new();
        let mut cursor_pos = 0;
        let mut offset = 0;
        let mut found_cursor = false;

        for (i, b) in self.blocks.iter().enumerate() {
            if i == self.active_idx {
                if !b.text.is_empty() {
                    let local_off = self.cursor.saturating_sub(offset).min(b.code.len());
                    if local_off == 0 {
                        cursor_pos = s.len();
                    }
                    s.push_str(&b.text);
                    if local_off != 0 {
                        cursor_pos = s.len();
                    }
                    found_cursor = true;
                    offset += b.code.len();
                } else {
                    let local_off = self.cursor.saturating_sub(offset).min(b.code.len());
                    let (before, after) = b.code.split_at(local_off);
                    s.push_str(before);
                    cursor_pos = s.len();
                    s.push_str(after);
                    found_cursor = true;
                    offset += b.code.len();
                }
            } else {
                if !found_cursor && self.cursor <= offset {
                    cursor_pos = s.len();
                    found_cursor = true;
                }
                if !b.text.is_empty() {
                    s.push_str(&b.text);
                } else {
                    s.push_str(&b.code);
                }
                offset += b.code.len();
                if !found_cursor && self.cursor <= offset {
                    cursor_pos = s.len();
                    found_cursor = true;
                }
            }
        }
        if !found_cursor {
            cursor_pos = s.len();
        }
        (s, cursor_pos)
    }

    fn locate_pos(&self, pos: usize) -> (usize, usize) {
        if self.blocks.is_empty() {
            return (0, 0);
        }
        let mut offset = 0;
        for (i, b) in self.blocks.iter().enumerate() {
            let len = b.code.len();
            if pos <= offset + len {
                return (i, pos - offset);
            }
            offset += len;
        }
        (self.blocks.len() - 1, self.blocks.last().unwrap().code.len())
    }

    fn get_block_start_offset(&self, idx: usize) -> usize {
        self.blocks[..idx.min(self.blocks.len())].iter().map(|b| b.code.len()).sum()
    }

    fn merge_free_blocks(&mut self) {
        let mut i = 0;
        while i + 1 < self.blocks.len() {
            if !self.blocks[i].confirmed && !self.blocks[i+1].confirmed {
                let next_code = std::mem::take(&mut self.blocks[i+1].code);
                self.blocks[i].code.push_str(&next_code);
                self.blocks[i].text.clear();
                self.blocks.remove(i+1);
                if self.active_idx > i {
                    self.active_idx -= 1;
                }
            } else {
                i += 1;
            }
        }
    }

    fn lock_block(&mut self, idx: usize) {
        if idx >= self.blocks.len() { return; }
        let block = &mut self.blocks[idx];

        if block.code.is_empty() {
            self.blocks.remove(idx);
            if self.active_idx > idx && self.active_idx != usize::MAX {
                self.active_idx -= 1;
            }
            self.current_selection.clear();
            self.current_consumed = 0;
            self.last_pending_code.clear();
            return;
        }

        if block.text.is_empty() && !block.confirmed {
            if !block.saved_text.is_empty() && self.current_selection.is_empty() {
                block.text = block.saved_text.clone();
            } else if !self.current_selection.is_empty() {
                block.text = self.current_selection.clone();
            } else if self.cfg.auto_commit_unselected {
                if let Some(best) = self.candidates.first() {
                    block.text = best.text.clone();
                } else {
                    block.text = block.code.clone();
                }
            } else {
                block.text = block.code.clone();
            }
            block.saved_text.clear();
            block.confirmed = true;
        }

        self.current_selection.clear();
        self.current_consumed = 0;
        self.last_pending_code.clear();
    }
}

fn encode_word_from_codes(codes: &[String]) -> Option<String> {
    let n = codes.len();
    if n < 2 || n > 8 { return None; }
    let c: Vec<&[u8]> = codes.iter().map(|s| s.as_bytes()).collect();
    if c.iter().any(|s| s.len() < 3) { return None; }

    let mut res = format!("{}{}{}{}", c[0][0] as char, c[0][1] as char, c[1][0] as char, c[1][2] as char);
    for i in 2..n {
        res.push(c[i][0] as char);
    }
    Some(res)
}

fn is_toggle(key: u32, mods: u32) -> bool {
    key == keys::KEY_SPACE && (mods & keys::MOD_CTRL) != 0
}

fn punct_char(key: u32, shifted: bool, mode: &str) -> Option<&'static str> {
    if mode == "english" {
        raw_punct_char(key, shifted)
    } else {
        chinese_punct(key, shifted).or_else(|| raw_punct_char(key, shifted))
    }
}

fn chinese_punct(key: u32, shifted: bool) -> Option<&'static str> {
    match (key, shifted) {
        (keys::KEY_COMMA, false)      => Some("，"),
        (keys::KEY_COMMA, true)       => Some("《"),
        (keys::KEY_DOT, false)        => Some("。"),
        (keys::KEY_DOT, true)         => Some("》"),
        (keys::KEY_SLASH, true)       => Some("？"),
        (keys::KEY_SEMICOLON, false)  => Some("；"),
        (keys::KEY_SEMICOLON, true)   => Some("："),
        (keys::KEY_APOSTROPHE, false) => Some("＇"),
        (keys::KEY_APOSTROPHE, true)  => Some("\u{201c}"),
        (keys::KEY_LEFTBRACE, false)  => Some("【"),
        (keys::KEY_LEFTBRACE, true)   => Some("「"),
        (keys::KEY_RIGHTBRACE, false) => Some("】"),
        (keys::KEY_RIGHTBRACE, true)  => Some("」"),
        (keys::KEY_BACKSLASH, false)  => Some("、"),
        (keys::KEY_BACKSLASH, true)   => Some("|"),
        _ => None,
    }
}

fn raw_punct_char(key: u32, shifted: bool) -> Option<&'static str> {
    match (key, shifted) {
        (keys::KEY_COMMA, false)      => Some(","),
        (keys::KEY_COMMA, true)       => Some("<"),
        (keys::KEY_DOT, false)        => Some("."),
        (keys::KEY_DOT, true)         => Some(">"),
        (keys::KEY_SLASH, false)      => Some("/"),
        (keys::KEY_SLASH, true)       => Some("?"),
        (keys::KEY_SEMICOLON, false)  => Some(";"),
        (keys::KEY_SEMICOLON, true)   => Some(":"),
        (keys::KEY_APOSTROPHE, false) => Some("'"),
        (keys::KEY_APOSTROPHE, true)  => Some("\""),
        (keys::KEY_LEFTBRACE, false)  => Some("["),
        (keys::KEY_LEFTBRACE, true)   => Some("{"),
        (keys::KEY_RIGHTBRACE, false) => Some("]"),
        (keys::KEY_RIGHTBRACE, true)  => Some("}"),
        (keys::KEY_BACKSLASH, false)  => Some("\\"),
        (keys::KEY_BACKSLASH, true)   => Some("|"),
        _ => None,
    }
}
