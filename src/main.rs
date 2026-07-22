// yinxing - 音形码输入法
// Copyright (C) 2026  fajknli
// Licensed under the GNU AGPL v3. See LICENSE for details.

use yinxing::engine::lookup::LookupTable;
use yinxing::engine::RuntimeState;
use std::env;

fn main() {
    // 优先使用 XDG_RUNTIME_DIR，回退到 /run/user/uid
    let runtime_dir = env::var("XDG_RUNTIME_DIR")
        .unwrap_or_else(|_| format!("/run/user/{}", unsafe { libc::getuid() }));
    let pid_path = format!("{}/yinxing.pid", runtime_dir);

    let args: Vec<String> = env::args().collect();

    // 如果带 --reload 参数，给正在运行的实例发信号然后退出
    if args.iter().any(|a| a == "--reload") {
        match std::fs::read_to_string(&pid_path) {
            Ok(pid_str) => {
                if let Ok(pid) = pid_str.trim().parse::<i32>() {
                    unsafe { libc::kill(pid, libc::SIGHUP); }
                    eprintln!("yinxing: reload signal sent to pid {}", pid);
                } else {
                    eprintln!("yinxing: invalid pid in {}", pid_path);
                }
            }
            Err(_) => eprintln!("yinxing: no running instance found."),
        }
        return;
    }

    eprintln!("yinxing: loading...");
    let cfg = yinxing::config::AppConfig::load();
    let table = LookupTable::load(&cfg.file_chars, &cfg.file_words);

    let runtime = RuntimeState::new(
        table,
        cfg.file_user.clone(),
        cfg.custom_phrases.clone(),
        cfg.behavior.clone(),
    );

    // 写入 PID 文件，供 --reload 使用
    let _ = std::fs::write(&pid_path, std::process::id().to_string());

    if let Err(e) = yinxing::wayland::run(runtime, cfg.candi, cfg.behavior.repeat_delay_ms, cfg.behavior.repeat_rate_ms) {
        eprintln!("yinxing: fatal: {}", e);
        std::process::exit(1);
    }

    // 退出时清理 PID 文件
    let _ = std::fs::remove_file(&pid_path);
}
