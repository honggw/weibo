//! Simple file logger for Weibo PC client.
//! Writes timestamped log entries to `weibo_app.log` and also prints to stdout/stderr.

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

static LOGGER: std::sync::LazyLock<Mutex<std::fs::File>> = std::sync::LazyLock::new(|| {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("weibo_app.log")
        .expect("无法创建日志文件 weibo_app.log");
    Mutex::new(file)
});

fn timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let hours = (secs / 3600) % 24;
    let minutes = (secs / 60) % 60;
    let seconds = secs % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

fn write_line(prefix: &str, msg: &str, console: fn(&str)) {
    let line = format!("[{} {}] {}\n", timestamp(), prefix, msg);
    console(&line);
    if let Ok(mut f) = LOGGER.lock() {
        let _ = f.write_all(line.as_bytes());
        let _ = f.flush();
    }
}

/// Raw logging functions
pub fn info(msg: &str) {
    write_line("INFO", msg, |s| print!("{}", s));
}

pub fn error(msg: &str) {
    write_line("ERR ", msg, |s| eprint!("{}", s));
}

pub fn success(msg: &str) {
    write_line("OK  ", msg, |s| println!("{}", s));
}

// Convenience macros that use format! and call the above functions
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {{
        $crate::logger::info(&format!($($arg)*));
    }};
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {{
        $crate::logger::error(&format!($($arg)*));
    }};
}

#[macro_export]
macro_rules! log_success {
    ($($arg:tt)*) => {{
        $crate::logger::success(&format!($($arg)*));
    }};
}
