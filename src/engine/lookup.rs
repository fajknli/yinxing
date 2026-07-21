use rustc_hash::FxHashMap;
use std::fs;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub struct Entry {
    pub text: Rc<str>,
    pub code: Rc<str>,
    pub weight: u32,
}

pub struct LookupTable {
    chars_sorted: Vec<(String, Vec<Entry>)>,
    words_sorted: Vec<(String, Vec<Entry>)>,
    custom_words: FxHashMap<String, Vec<Entry>>,
    char_to_code: FxHashMap<String, String>, // 新增：汉字 -> 3键全码
}

impl LookupTable {
    pub fn load(chars_path: &str, words_path: &str) -> Self {
        let chars_map = load_dict(chars_path);
        let words_map = load_dict(words_path);

        let mut chars_sorted: Vec<_> = chars_map.into_iter().collect();
        chars_sorted.sort_by(|a, b| a.0.cmp(&b.0));

        let mut words_sorted: Vec<_> = words_map.into_iter().collect();
        words_sorted.sort_by(|a, b| a.0.cmp(&b.0));

        // 新增：构建汉字反查字典
        let mut char_to_code = FxHashMap::default();
        for (_, entries) in &chars_sorted {
            for e in entries {
                if e.code.len() == 3 {
                    char_to_code.entry(e.text.to_string()).or_insert_with(|| e.code.to_string());
                }
            }
        }

        eprintln!("yinxing: loaded {} char codes, {} word codes",
            chars_sorted.iter().map(|(_, v)| v.len()).sum::<usize>(),
            words_sorted.iter().map(|(_, v)| v.len()).sum::<usize>(),
        );

        Self { chars_sorted, words_sorted, custom_words: FxHashMap::default(), char_to_code }
    }

    pub fn add_custom_phrase(&mut self, text: &str, code: &str) -> bool {
        let code_str = code.to_string();
        let is_new = !self.custom_words.get(&code_str).map_or(false, |v| v.iter().any(|e| e.text.as_ref() == text));
        if is_new {
            let entry = Entry {
                text: Rc::from(text),
                code: Rc::from(code),
                weight: 999999,
            };
            self.custom_words.entry(code_str).or_default().push(entry);
        }
        is_new
    }

    // 新增：根据整句文本反查所有字的 3 键全码，用于混合造词
    pub fn get_codes_for_text(&self, text: &str) -> Option<Vec<String>> {
        let mut codes = Vec::new();
        for ch in text.chars() {
            let ch_str = ch.to_string();
            if let Some(code) = self.char_to_code.get(&ch_str) {
                codes.push(code.clone());
            } else {
                return None;
            }
        }
        Some(codes)
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

    fn exact_words(&self, code: &str) -> Vec<Entry> {
        // 优化：单次 binary_search，并精确预分配容量
        let main_entries = self.words_sorted
            .binary_search_by(|(k, _)| k.as_str().cmp(code))
            .ok()
            .map(|idx| self.words_sorted[idx].1.as_slice())
            .unwrap_or(&[]);

        let cust_entries = self.custom_words.get(code).map(|v| v.as_slice()).unwrap_or(&[]);

        let mut res = Vec::with_capacity(main_entries.len() + cust_entries.len());
        res.extend_from_slice(main_entries);
        res.extend_from_slice(cust_entries);
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
