use rustc_hash::FxHashMap;
use std::fs;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub struct Entry {
    pub text: Rc<str>,
    pub code: Rc<str>,
    pub weight: u32,
}

#[derive(Clone, Debug)]
pub struct SplitResult {
    pub words: Vec<Entry>,
    pub consumed: usize,
    pub score: f64,
}

pub struct LookupTable {
    chars_sorted: Vec<(String, Vec<Entry>)>,
    words_sorted: Vec<(String, Vec<Entry>)>,
    custom_words: FxHashMap<String, Vec<Entry>>, // 新增：动态自造词字典
}

const MAX_CANDIDATES: usize = 20;

impl LookupTable {
    pub fn load(chars_path: &str, words_path: &str) -> Self {
        let chars_map = load_dict(chars_path);
        let words_map = load_dict(words_path);

        let mut chars_sorted: Vec<_> = chars_map.into_iter().collect();
        chars_sorted.sort_by(|a, b| a.0.cmp(&b.0));

        let mut words_sorted: Vec<_> = words_map.into_iter().collect();
        words_sorted.sort_by(|a, b| a.0.cmp(&b.0));

        eprintln!("yinxing: loaded {} char codes, {} word codes",
            chars_sorted.iter().map(|(_, v)| v.len()).sum::<usize>(),
            words_sorted.iter().map(|(_, v)| v.len()).sum::<usize>(),
        );

        Self { chars_sorted, words_sorted, custom_words: FxHashMap::default() }
    }

    /// 新增：注入一个自造词到内存字典中
    pub fn add_custom_phrase(&mut self, text: &str, code: &str) -> bool {
        let code_str = code.to_string();
        // 检查是否已存在
        let is_new = !self.custom_words.get(&code_str).map_or(false, |v| v.iter().any(|e| e.text.as_ref() == text));
        if is_new {
            let entry = Entry {
                text: Rc::from(text),
                code: Rc::from(code),
                weight: 999999, // 自造词赋予极高权重，保证排在第一位
            };
            self.custom_words.entry(code_str).or_default().push(entry);
        }
        is_new
    }

    /// Viterbi DP 解码
    pub fn decode<B>(&self, code: &str, boost_fn: &B) -> Vec<SplitResult>
    where
        B: Fn(&str) -> u32,
    {
        let n = code.len();
        if n < 3 {
            return vec![];
        }

        const LAMBDA: f64 = 2.0;
        const CHAR_BIAS: f64 = 3.8;

        type State = (f64, Vec<Entry>);
        let mut dp: Vec<Option<State>> = vec![None; n + 1];
        dp[0] = Some((0.0, vec![]));

        for i in 3..=n {
            for len in [3usize, 4, 5, 6] {
                if len > i { continue; }
                let start = i - len;
                let segment = &code[start..i];

                let entries: Vec<Entry> = if len == 3 {
                    self.exact_chars(segment).map(|e| e.to_vec()).unwrap_or_default()
                } else {
                    self.exact_words(segment) // exact_words 现在返回 Vec<Entry>
                };

                if entries.is_empty() { continue; }

                let prev = match &dp[start] {
                    Some(s) => s,
                    None => continue,
                };

                let best = entries.iter().max_by_key(|e| e.weight).unwrap();
                let user_boost = boost_fn(best.text.as_ref()) as f64;
                let base_weight = best.weight as f64 + user_boost;

                let mut score = base_weight.log2();
                if len == 3 { score += CHAR_BIAS; }
                score -= LAMBDA;

                let total_score = prev.0 + score;

                let is_better = match &dp[i] {
                    Some((old, _)) => total_score > *old,
                    None => true,
                };

                if is_better {
                    let mut words = prev.1.clone();
                    words.push(best.clone());
                    dp[i] = Some((total_score, words));
                }
            }
        }

        let mut results: Vec<SplitResult> = (3..=n)
            .filter_map(|i| {
                dp[i].as_ref().map(|(score, words)| SplitResult {
                    words: words.clone(),
                    consumed: i,
                    score: *score,
                })
            })
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        results.truncate(MAX_CANDIDATES);
        results
    }

    pub fn prefix_chars(&self, prefix: &str, limit: usize) -> Vec<Entry> {
        let mut results = Vec::new();
        let start = self.chars_sorted.partition_point(|(k, _)| k.as_str() < prefix);
        for i in start..self.chars_sorted.len() {
            if self.chars_sorted[i].0.starts_with(prefix) {
                for e in &self.chars_sorted[i].1 {
                    results.push(e.clone());
                    if results.len() >= limit { return results; }
                }
            } else {
                break;
            }
        }
        results
    }

    pub fn exact_chars(&self, code: &str) -> Option<&[Entry]> {
        self.chars_sorted
            .binary_search_by(|(k, _)| k.as_str().cmp(code))
            .ok()
            .map(|idx| &self.chars_sorted[idx].1[..])
    }

    // 修改：查词语时，同时查静态字典和动态字典
    fn exact_words(&self, code: &str) -> Vec<Entry> {
        let mut res = Vec::new();
        if let Some(idx) = self.words_sorted.binary_search_by(|(k, _)| k.as_str().cmp(code)).ok() {
            res.extend_from_slice(&self.words_sorted[idx].1);
        }
        if let Some(cust) = self.custom_words.get(code) {
            res.extend_from_slice(cust);
        }
        res
    }

    pub fn get_all_for_code(&self, code: &str) -> Vec<Entry> {
        if code.len() == 3 {
            self.exact_chars(code).map(|e| e.to_vec()).unwrap_or_default()
        } else if code.len() >= 4 {
            self.exact_words(code)
        } else {
            vec![]
        }
    }
    /// 检查原生词库中是否存在指定的词组和编码
    pub fn word_exists(&self, code: &str, text: &str) -> bool {
        match self.words_sorted.binary_search_by(|(k, _)| k.as_str().cmp(code)) {
            Ok(idx) => self.words_sorted[idx].1.iter().any(|e| e.text.as_ref() == text),
            Err(_) => false,
        }
    }
}

fn load_dict(path: &str) -> FxHashMap<String, Vec<Entry>> {
    let mut map: FxHashMap<String, Vec<Entry>> = FxHashMap::default();
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => { eprintln!("yinxing: dict not found: {}", path); return map; }
    };
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 2 { continue; }
        let text: Rc<str> = Rc::from(parts[0]);
        let code_key = parts[1].to_string();
        let code: Rc<str> = Rc::from(parts[1]);
        let weight: u32 = parts.get(2)
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(100);
        map.entry(code_key).or_default().push(Entry { text, code, weight });
    }
    map
}
