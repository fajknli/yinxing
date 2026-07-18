use rustc_hash::FxHashSet;
use crate::config;
use crate::engine::action::Action;
use crate::engine::learner::Learner;
use crate::engine::lookup::LookupTable;
use crate::keys;
use crate::state::ImeState;
use crate::candi;

#[derive(Clone)]
struct CandidateItem {
    text: String,
    consumed: usize,
    score: f64,
}

pub struct RuntimeState {
    input: String,
    mode: ImeState,
    table: LookupTable,
    page: usize,
    candidates: Vec<CandidateItem>,
    learner: Learner,
}

impl RuntimeState {
    pub fn new(
        table: LookupTable,
        user_data_path: String,
    ) -> Self {
        let learner = Learner::new(user_data_path);
        Self {
            input: String::with_capacity(32),
            mode: ImeState::English,
            table,
            page: 0,
            candidates: Vec::new(),
            learner,
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
            if !self.input.is_empty() {
                let raw = self.input.clone();
                a.push(Action::ClearPreedit);
                a.push(Action::CommitText(raw));
            }
            self.clear();
            self.mode = ImeState::English;
            return a;
        }

        if (mods & keys::MOD_SHIFT) != 0 {
            if keys::evdev_to_char(key).is_some() {
                let mut a = vec![];
                if !self.input.is_empty() {
                    let raw = self.input.clone();
                    a.push(Action::ClearPreedit);
                    a.push(Action::CommitText(raw));
                }
                self.clear();
                self.mode = ImeState::English;
                a.push(Action::ForwardKey { key, mods });
                return a;
            }
        }

        if (mods & !keys::MOD_SHIFT) != 0 {
            let mut a = vec![];
            if !self.input.is_empty() {
                self.clear();
                a.push(Action::ClearPreedit);
            }
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
            self.input.push(ch);
            self.mode = ImeState::Composing;
            self.page = 0;
            self.build_preedit()
        } else if key == keys::KEY_SPACE {
            vec![Action::CommitText(" ".into())]
        } else if let Some(p) = chinese_punct(key, (mods & keys::MOD_SHIFT) != 0) {
            vec![Action::CommitText(p.into())]
        } else {
            vec![Action::ForwardKey { key, mods }]
        }
    }

    fn on_composing(&mut self, key: u32, mods: u32) -> Vec<Action> {
        let shifted = (mods & keys::MOD_SHIFT) != 0;

        match key {
            keys::KEY_BACKSPACE => {
                self.input.pop();
                if self.input.is_empty() {
                    self.candidates.clear();
                    self.page = 0;
                    self.mode = ImeState::Chinese;
                    return vec![Action::ClearPreedit];
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
                let raw = self.input.clone();
                self.clear();
                self.mode = ImeState::Chinese;
                vec![Action::ClearPreedit, Action::CommitText(raw)]
            }

            keys::KEY_SPACE => self.commit_nth(0),

            keys::KEY_1..=keys::KEY_9 if !shifted =>
                self.commit_nth((key - keys::KEY_1) as usize),

            keys::KEY_TAB | keys::KEY_EQUALS if !shifted => {
                self.next_page();
                self.build_preedit()
            }

            keys::KEY_MINUS if !shifted => {
                self.prev_page();
                self.build_preedit()
            }

            k if raw_punct_char(k, shifted).is_some() => {
                let punct = raw_punct_char(k, shifted).unwrap();
                let mut actions = self.commit_best();
                actions.push(Action::CommitText(punct.into()));
                actions
            }

            _ => {
                if let Some(ch) = keys::evdev_to_char(key) {
                    self.input.push(ch);
                    self.page = 0;
                    self.build_preedit()
                } else {
                    let mut a = self.commit_best();
                    a.push(Action::ForwardKey { key, mods });
                    a
                }
            }
        }
    }

    //笛卡尔积全展开
    //把一条路径上所有同码字的所有组合可能性全部列出来
    fn build_preedit(&mut self) -> Vec<Action> {
        let code = self.input.clone();
        let n = code.len();

        self.candidates.clear();

        if n == 0 {
            return vec![];
        }

        // 1. 不足 3 码：前缀匹配提示
        if n < 3 {
            let mut entries = self.table.prefix_chars(&code, 200);
            for entry in &mut entries {
                entry.weight += self.learner.get_boost(entry.text.as_ref());
            }
            entries.sort_by(|a, b| b.weight.cmp(&a.weight));
            let mut seen = FxHashSet::default();
            entries.retain(|e| seen.insert(e.text.clone()));
            self.candidates = entries.into_iter().map(|e| CandidateItem {
                text: e.text.to_string(),
                consumed: 0,
                score: 0.0,
            }).collect();

            let max_page = self.candidates.len().saturating_sub(1) / config::CANDIDATE_PAGE_SIZE;
            if self.page > max_page {
                self.page = max_page;
            }

            let c = n as i32;
            return vec![Action::SetPreedit { text: code, begin: c, end: c }];
        }

        // 2. DP 自动分词
        let learner = &self.learner;
        let splits = self.table.decode(&code, &|text| learner.get_boost(text));

        for split in &splits {
            let sentence: String = split.words.iter().map(|e| e.text.as_ref()).collect();
            if !self.candidates.iter().any(|c| c.text == sentence) {
                self.candidates.push(CandidateItem {
                    text: sentence,
                    consumed: split.consumed,
                    score: split.score,
                });
            }
        }

        // 3. 展开所有 DP 路径的替代项 (使用笛卡尔积，彻底修复同码字组合丢失)
        for best in &splits {
            let alt_sentences = expand_path(&best.words, &self.table, 50);
            for alt_sentence in alt_sentences {
                if alt_sentence.is_empty() { continue; }
                if !self.candidates.iter().any(|c| c.text == alt_sentence) {
                    self.candidates.push(CandidateItem {
                        text: alt_sentence,
                        consumed: best.consumed,
                        score: best.score - 0.1,
                    });
                }
            }
        }

        // 4. 强制补充所有可能的部分精确匹配 (防止前缀词丢失)
        for len in (3..=n).rev() {
            let segment = &code[..len];
            let exact = self.table.get_all_for_code(segment);
            for entry in &exact {
                let text = entry.text.to_string();
                if !self.candidates.iter().any(|c| c.text == text) {
                    self.candidates.push(CandidateItem {
                        text,
                        consumed: len,
                        score: entry.weight as f64 + self.learner.get_boost(entry.text.as_ref()) as f64,
                    });
                }
            }
        }

        // 5. 排序：消耗码长降序 → 得分降序
        self.candidates.sort_by(|a, b| {
            b.consumed.cmp(&a.consumed)
                .then_with(|| b.score.partial_cmp(&a.score).unwrap())
        });

        let max_page = self.candidates.len().saturating_sub(1) / config::CANDIDATE_PAGE_SIZE;
        if self.page > max_page {
            self.page = max_page;
        }

        let c = n as i32;
        vec![Action::SetPreedit { text: code, begin: c, end: c }]
    }

    pub fn commit_nth(&mut self, n: usize) -> Vec<Action> {
        let idx = self.page * config::CANDIDATE_PAGE_SIZE + n;
        if let Some(cand) = self.candidates.get(idx).cloned() {
            self.learner.record(&cand.text);

            let remaining = if cand.consumed > 0 && cand.consumed < self.input.len() {
                self.input[cand.consumed..].to_string()
            } else {
                String::new()
            };

            self.input = remaining;
            self.candidates.clear();
            self.page = 0;

            if self.input.is_empty() {
                self.mode = ImeState::Chinese;
                return vec![Action::CommitText(cand.text), Action::ClearPreedit];
            }

            self.mode = ImeState::Composing;
            let mut actions = vec![Action::CommitText(cand.text)];
            actions.extend(self.build_preedit());
            actions
        } else {
            let raw = self.input.clone();
            self.clear();
            self.mode = ImeState::Chinese;
            vec![Action::CommitText(raw), Action::ClearPreedit]
        }
    }

    fn commit_best(&mut self) -> Vec<Action> {
        if let Some(cand) = self.candidates.first().cloned() {
            self.learner.record(&cand.text);

            let remaining = if cand.consumed > 0 && cand.consumed < self.input.len() {
                self.input[cand.consumed..].to_string()
            } else {
                String::new()
            };

            self.input = remaining;
            self.candidates.clear();
            self.page = 0;

            if self.input.is_empty() {
                self.mode = ImeState::Chinese;
                return vec![Action::CommitText(cand.text), Action::ClearPreedit];
            }

            self.mode = ImeState::Composing;
            let mut actions = vec![Action::CommitText(cand.text)];
            actions.extend(self.build_preedit());
            actions
        } else {
            let raw = self.input.clone();
            self.clear();
            self.mode = ImeState::Chinese;
            vec![Action::CommitText(raw), Action::ClearPreedit]
        }
    }

    fn clear(&mut self) {
        self.input.clear();
        self.candidates.clear();
        self.page = 0;
    }

    fn next_page(&mut self) {
        if (self.page + 1) * config::CANDIDATE_PAGE_SIZE < self.candidates.len() {
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
        self.input.clone()
    }

    pub fn is_chinese(&self) -> bool { self.mode != ImeState::English }
    pub fn has_candidates(&self) -> bool { !self.candidates.is_empty() }
    pub fn sync_candidates(&self) {}
    pub fn flush_user_buffer(&mut self) { /* handled by Learner::drop */ }

    pub fn candidate_items(&self) -> Vec<candi::Item> {
        let start = self.page * config::CANDIDATE_PAGE_SIZE;
        let end = (start + config::CANDIDATE_PAGE_SIZE).min(self.candidates.len());
        self.candidates[start..end].iter().enumerate().map(|(i, c)| {
            let prefix = format!("{}.", i + 1);
            candi::Item::with_prefix(prefix, c.text.clone())
        }).collect()
    }
}

// 生成一条 DP 路径上所有同码字的组合（笛卡尔积）
fn expand_path(words: &[crate::engine::lookup::Entry], table: &LookupTable, limit: usize) -> Vec<String> {
    let mut combos: Vec<Vec<String>> = vec![vec![]];
    for word in words {
        let alts = table.get_all_for_code(word.code.as_ref());
        let mut next_combos = Vec::new();
        for combo in &combos {
            if alts.is_empty() {
                let mut new_combo = combo.clone();
                new_combo.push(word.text.to_string());
                next_combos.push(new_combo);
            } else {
                for alt in &alts {
                    let mut new_combo = combo.clone();
                    new_combo.push(alt.text.to_string());
                    next_combos.push(new_combo);
                }
            }
        }
        combos = next_combos;
        if combos.len() > limit { break; } // 防止组合爆炸
    }
    combos.into_iter().map(|c| c.concat()).collect()
}

fn is_toggle(key: u32, mods: u32) -> bool {
    key == keys::KEY_SPACE && (mods & keys::MOD_CTRL) != 0
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
//Exhaustive Prefix Matching (穷举前缀匹配)
// the most fundamental rule of an input method: What you see is what you keep.
// No dropped words: It searches every possible prefix length (6 -> 5 -> 4 -> 3) and pulls all matches.
// Correct prioritization: It sorts by code length consumed (longest first), so 中华 (4 keys) ranks above single characters (3 keys).
// Correct leftover handling: When you select 中华 from zyhhmz, it perfectly keeps the mz in the input box.
