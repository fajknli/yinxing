use yinxing::engine::lookup::LookupTable;
use yinxing::engine::RuntimeState;

fn main() {
    eprintln!("yinxing: loading...");

    let cfg = yinxing::config::AppConfig::load();
    let table = LookupTable::load(&cfg.file_chars, &cfg.file_words);

    let runtime = RuntimeState::new(
        table,
        cfg.file_user.clone(),
        cfg.custom_phrases.clone(),
        cfg.behavior.clone(),
    );

    if let Err(e) = yinxing::wayland::run(runtime, cfg.candi, 600, 40) {
        eprintln!("yinxing: fatal: {}", e);
        std::process::exit(1);
    }
}
