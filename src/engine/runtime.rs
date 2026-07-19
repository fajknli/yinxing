use std::rc::Rc;
use rustc_hash::FxHashSet;
use crate::config;
use crate::engine::action::Action;
use crate::engine::learner::Learner;
use crate::engine::lookup::LookupTable;
use crate::keys;
use crate::state::ImeState;
use crate::candi;

#[derive(Clone)]
struct DpNode {
    score: f64,
    word: Rc<str>,          // 只存引用，零分配
    prev: Option<(usize, usize)>, // 指向上一个节点的 (位置, 索引)
}

#[derive(Clone)]
struct CandidateItem {
    text: String,
    consumed: usize,
    score: f64,
    code: String, // 记录该候选词对应的完整原始编码
    is_dp_split: bool,
}

pub struct RuntimeState {
    input: String,
    cursor: usize,
    mode: ImeState,
    table: LookupTable,
    page: usize,
    candidates: Vec<CandidateItem>,
    learner: Learner,
    custom_phrases_path: String,
}

impl RuntimeState {
    pub fn new(
        table: LookupTable,
        user_data_path: String,
        custom_phrases_path: String,
    ) -> Self {
        let learner = Learner::new(user_data_path);
        let mut runtime = Self {
            input: String::with_capacity(32),
            cursor: 0,
            mode: ImeState::English,
            table,
            page: 0,
            candidates: Vec::new(),
            learner,
            custom_phrases_path,
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
            self.input.insert(self.cursor, ch); // 在光标处插入
            self.cursor += ch.len_utf8();        // 光标后移
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
                if self.cursor > 0 {
                    // 找到前一个字符的边界并删除
                    let prev_char_boundary = self.input[..self.cursor].char_indices().last().map(|(i, _)| i).unwrap_or(0);
                    self.input.remove(prev_char_boundary);
                    self.cursor = prev_char_boundary;
                }

                if self.input.is_empty() {
                    self.candidates.clear();
                    self.page = 0;
                    self.mode = ImeState::Chinese;
                    return vec![Action::ClearPreedit];
                }
                self.page = 0;
                self.build_preedit()
            }

            keys::KEY_DELETE => { // 可选：支持 Delete 键向后删除
                if self.cursor < self.input.len() {
                    let next_char_len = self.input[self.cursor..].chars().next().map(|c| c.len_utf8()).unwrap_or(0);
                    let end = self.cursor + next_char_len;
                    self.input.replace_range(self.cursor..end, "");
                }
                self.page = 0;
                self.build_preedit()
            }

            keys::KEY_LEFT => {
                if self.cursor > 0 {
                    // 向左移动一个字符
                    self.cursor = self.input[..self.cursor].char_indices().last().map(|(i, _)| i).unwrap_or(0);
                }
                self.page = 0;
                self.build_preedit()
            }

            keys::KEY_RIGHT => {
                if self.cursor < self.input.len() {
                    // 向右移动一个字符
                    let next_char_len = self.input[self.cursor..].chars().next().map(|c| c.len_utf8()).unwrap_or(0);
                    self.cursor += next_char_len;
                }
                self.page = 0;
                self.build_preedit()
            }

            keys::KEY_HOME => {
                self.cursor = 0;
                self.page = 0;
                self.build_preedit()
            }

            keys::KEY_END => {
                self.cursor = self.input.len();
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
                    // 在光标处插入字符，而不是 push 到末尾
                    self.input.insert(self.cursor, ch);
                    self.cursor += ch.len_utf8(); // 光标后移
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

    fn build_preedit(&mut self) -> Vec<Action> {
        let code = self.input.clone();
        let n = code.len();

        self.candidates.clear();

        if n == 0 {
            return vec![];
        }

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
                code: e.code.to_string(),
                is_dp_split: false,
            }).collect();

            let max_page = self.candidates.len().saturating_sub(1) / config::CANDIDATE_PAGE_SIZE;
            if self.page > max_page { self.page = max_page; }

            let c = self.cursor as i32;
            return vec![Action::SetPreedit { text: code, begin: c, end: c }];
        }

        // ==========================================
        // 1. Beam Search DP 动态规划 (零堆分配优化版)
        // ==========================================
        let mut dp: Vec<Vec<DpNode>> = vec![vec![]; n + 1];
        dp[0].push(DpNode { score: 0.0, word: Rc::from(""), prev: None });

        for i in 3..=n {
            for len in [3, 4, 5, 6, 7, 8, 9, 10, 12, 15, 18, 21, 24].iter() {
                if *len > i { continue; }
                let start = i - len;
                let segment = &code[start..i];

                if dp[start].is_empty() { continue; }

                let mut entries = self.table.get_all_for_code(segment);

                // 智能转化
                if *len >= 6 && *len % 3 == 0 {
                    let num_chars = *len / 3;
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

                        // 零分配：只克隆 Rc 指针和基本类型
                        dp[i].push(DpNode {
                            score: new_score,
                            word: Rc::clone(&entry.text),
                            prev: Some((start, p_idx)),
                        });
                    }
                }
            }

            // 关键剪枝：每个位置只保留得分最高的 5 条路径
            dp[i].sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
            dp[i].truncate(5);
        }

        // 只收集消耗了全部输入 (dp[n]) 的整句候选，最多拼接 5 次，彻底消灭长句卡顿
        for node in &dp[n] {
            // 反向追踪拼接字符串
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
                    consumed: n,
                    score: node.score,
                    code: code.clone(),
                    is_dp_split: true,
                });
            }
        }

        // 2. 暴力前缀查表 (绝对不丢词，限制最大长度为 10，避免无意义查表)
        for len in (3..=n.min(10)).rev() {
            let segment = &code[..len];
            let exact = self.table.get_all_for_code(segment);
            for entry in &exact {
                let text = entry.text.to_string();
                if !self.candidates.iter().any(|c| c.text == text) {
                    self.candidates.push(CandidateItem {
                        text: text.clone(),
                        consumed: len,
                        score: entry.weight as f64 + self.learner.get_boost(entry.text.as_ref()) as f64,
                        code: entry.code.to_string(),
                        is_dp_split: false,
                    });
                }
            }
        }

        // 排序：消耗码长降序 → 得分降序
        self.candidates.sort_by(|a, b| {
            b.consumed.cmp(&a.consumed)
                .then_with(|| b.score.partial_cmp(&a.score).unwrap())
        });

        let max_page = self.candidates.len().saturating_sub(1) / config::CANDIDATE_PAGE_SIZE;
        if self.page > max_page { self.page = max_page; }

        let c = self.cursor as i32;
        vec![Action::SetPreedit { text: code, begin: c, end: c }]
    }

    pub fn commit_nth(&mut self, n: usize) -> Vec<Action> {
        let idx = self.page * config::CANDIDATE_PAGE_SIZE + n;
        if let Some(cand) = self.candidates.get(idx).cloned() {
            self.learner.record(&cand.text);

            // --- BUG FIX: 截取实际消耗的编码用于造词 ---
            let mut phrase_cand = cand.clone();
            if cand.consumed > 0 && cand.consumed <= self.input.len() {
                phrase_cand.code = self.input[..cand.consumed].to_string();
            }
            self.try_create_phrase(&phrase_cand);
            // ---------------------------------------------

            let remaining = if cand.consumed > 0 && cand.consumed < self.input.len() {
                self.input[cand.consumed..].to_string()
            } else {
                String::new()
            };

            self.input = remaining;
            self.cursor = self.input.len(); // 光标移动到残余输入的末尾
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

            // --- BUG FIX: 截取实际消耗的编码用于造词 ---
            let mut phrase_cand = cand.clone();
            if cand.consumed > 0 && cand.consumed <= self.input.len() {
                phrase_cand.code = self.input[..cand.consumed].to_string();
            }
            self.try_create_phrase(&phrase_cand);
            // ---------------------------------------------

            let remaining = if cand.consumed > 0 && cand.consumed < self.input.len() {
                self.input[cand.consumed..].to_string()
            } else {
                String::new()
            };

            self.input = remaining;
            self.cursor = self.input.len(); // 光标移动到残余输入的末尾
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

    // 核心造词逻辑：只处理 DP 切分出来的单字全码组合
    fn try_create_phrase(&mut self, cand: &CandidateItem) {
        // 1. 必须是 DP 切分出来的结果，原生词组绝不处理
        if !cand.is_dp_split {
            return;
        }

        let code_len = cand.code.len();
        // 2. 只有当原始编码长度 >= 6 且能被 3 整除时，才说明是单字全码连打
        if code_len >= 6 && code_len % 3 == 0 {
            let num_chars = code_len / 3;
            if num_chars >= 2 && num_chars <= 8 {  // <--- 改成 8，支持长词造词
                let mut sub_codes = Vec::new();
                for i in 0..num_chars {
                    sub_codes.push(cand.code[i*3..(i+1)*3].to_string());
                }
                // 尝试用 ISIR 规则生成标准词组编码
                if let Some(new_code) = encode_word_from_codes(&sub_codes) {
                    // 3. 检查原生字典里有没有这个词
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
        self.input.clear();
        self.cursor = 0;
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

fn encode_word_from_codes(codes: &[String]) -> Option<String> {
    let n = codes.len();
    if n < 2 || n > 8 { return None; }
    let c: Vec<&[u8]> = codes.iter().map(|s| s.as_bytes()).collect();
    if c.iter().any(|s| s.len() < 3) { return None; }

    // 核心：首字 IS，第二字 IR，后续字全 I
    let mut res = format!("{}{}{}{}", c[0][0] as char, c[0][1] as char, c[1][0] as char, c[1][2] as char);
    for i in 2..n {
        res.push(c[i][0] as char);
    }
    Some(res)
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
