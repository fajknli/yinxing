use std::collections::HashMap;
use std::time::Instant;

const FLUSH_INTERVAL_SECS: u64 = 10;
const BOOST_SCALE: u32 = 10000;
const BOOST_CAP: u32 = 500000;

pub struct Learner {
    path: String,
    user_freq: HashMap<String, u32>,
    dirty: bool,
    last_flush: Instant,
}

impl Learner {
    pub fn new(path: String) -> Self {
        let user_freq = load_file(&path);
        eprintln!("yinxing: user freq: {} entries", user_freq.len());
        Self {
            path,
            user_freq,
            dirty: false,
            last_flush: Instant::now(),
        }
    }

    pub fn record(&mut self, text: &str) {
        *self.user_freq.entry(text.to_string()).or_insert(0) += 1;
        self.dirty = true;
        if self.last_flush.elapsed().as_secs() >= FLUSH_INTERVAL_SECS {
            self.flush();
        }
    }

    pub fn get_boost(&self, text: &str) -> u32 {
        self.user_freq.get(text)
            .copied()
            .unwrap_or(0)
            .saturating_mul(BOOST_SCALE)
            .min(BOOST_CAP)
    }

    fn flush(&mut self) {
        if !self.dirty { return; }
        let temp_path = format!("{}.tmp", self.path);
        match std::fs::File::create(&temp_path) {
            Ok(mut f) => {
                use std::io::Write;
                for (word, count) in &self.user_freq {
                    let _ = writeln!(f, "{}\t{}", word, count);
                }
                drop(f);
                let _ = std::fs::rename(&temp_path, &self.path);
            }
            Err(e) => {
                eprintln!("yinxing: failed to flush learner to {}: {}", self.path, e);
            }
        }
        self.dirty = false;
        self.last_flush = Instant::now();
    }
}

impl Drop for Learner {
    fn drop(&mut self) {
        self.flush();
    }
}

fn load_file(path: &str) -> HashMap<String, u32> {
    let mut map = HashMap::new();
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return map,
    };
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 2 {
            let word = parts[0].to_string();
            let count: u32 = parts[1].parse().unwrap_or(1);
            map.insert(word, count);
        }
    }
    map
}
