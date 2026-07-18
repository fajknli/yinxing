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

        Self { chars_sorted, words_sorted }
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

        // 词数惩罚：越大越偏向单字拆分
        const LAMBDA: f64 = 2.0;
        // 单字基线偏移：拉平单字与词语的权重量级差
        // chars wpv ~10-1000, words wpv ~100-100000
        // Without this bias, DP will always prefer long words.
        const CHAR_BIAS: f64 = 3.8;

        type State = (f64, Vec<Entry>);
        let mut dp: Vec<Option<State>> = vec![None; n + 1];
        dp[0] = Some((0.0, vec![]));

        for i in 3..=n {
            for len in [3usize, 4, 5, 6] {
                if len > i { continue; }
                let start = i - len;
                let segment = &code[start..i];

                // 查字典，返回引用避免 clone 整个 Vec
                let entries: &[Entry] = if len == 3 {
                    self.exact_chars(segment).unwrap_or(&[])
                } else {
                    self.exact_words(segment).unwrap_or(&[])
                };

                if entries.is_empty() { continue; }

                let prev = match &dp[start] {
                    Some(s) => s,
                    None => continue,
                };

                // 直接取权重最高的条目，不排序
                let best = entries.iter().max_by_key(|e| e.weight).unwrap();
                let user_boost = boost_fn(&best.text) as f64;
                let base_weight = best.weight as f64 + user_boost;

                // 评分：对数权重 + 单字偏移 - 词数惩罚
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

        // 收集所有有效终止位置的结果
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

    fn exact_words(&self, code: &str) -> Option<&[Entry]> {
        self.words_sorted
            .binary_search_by(|(k, _)| k.as_str().cmp(code))
            .ok()
            .map(|idx| &self.words_sorted[idx].1[..])
    }
    /// 获取某个编码下的所有候选（不区分词/字，按长度自动判断）
    pub fn get_all_for_code(&self, code: &str) -> Vec<Entry> {
        let n = code.len();
        if n == 3 {
            self.exact_chars(code).map(|e| e.to_vec()).unwrap_or_default()
        } else if n >= 4 {
            self.exact_words(code).map(|e| e.to_vec()).unwrap_or_default()
        } else {
            vec![]
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
