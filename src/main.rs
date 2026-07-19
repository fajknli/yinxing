use yinxing::engine::lookup::LookupTable;
use yinxing::engine::RuntimeState;

fn main() {
    eprintln!("yinxing: loading...");

    let cfg = yinxing::config::AppConfig::load();
    let table = LookupTable::load(&cfg.file_chars, &cfg.file_words);

    // 获取 user_phrases.txt 的路径
    let user_phrases_path = {
        let parent = std::path::Path::new(&cfg.file_user).parent()
            .unwrap_or(std::path::Path::new("."));
        parent.join("user_phrases.txt").to_string_lossy().to_string()
    };

    let runtime = RuntimeState::new(table, cfg.file_user.clone(), user_phrases_path);

    if let Err(e) = yinxing::wayland::run(runtime, cfg.candi, 600, 40) {
        eprintln!("yinxing: fatal: {}", e);
        std::process::exit(1);
    }
}
