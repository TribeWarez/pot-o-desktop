use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::time::SystemTime;

fn log_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("pot-o-desktop")
}

fn timestamp() -> String {
    let d = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = d.as_secs();
    let ms = d.as_millis() % 1000;
    format!("{:02}:{:02}:{:02}.{:03}", secs / 3600 % 24, secs / 60 % 60, secs % 60, ms)
}

pub fn write(level: &str, context: &str, message: &str) {
    let dir = log_dir();
    std::fs::create_dir_all(&dir).ok();
    let path = dir.join("pot-o-desktop.log");
    let line = format!("[{}] {} [{}] {}\n", timestamp(), level, context, message);
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = f.write_all(line.as_bytes());
    }
}

pub fn error(context: &str, message: &str) {
    write("ERR", context, message);
}

#[allow(dead_code)]
pub fn warn(context: &str, message: &str) {
    write("WRN", context, message);
}

#[allow(dead_code)]
pub fn info(context: &str, message: &str) {
    write("INF", context, message);
}

pub fn read_log(max_lines: usize) -> Result<String, String> {
    let path = log_dir().join("pot-o-desktop.log");
    let content = std::fs::read_to_string(&path).map_err(|e| format!("Cannot read log: {}", e))?;
    let lines: Vec<&str> = content.lines().collect();
    let tail = if lines.len() > max_lines {
        &lines[lines.len() - max_lines..]
    } else {
        &lines[..]
    };
    Ok(tail.join("\n"))
}

pub fn clear_log() -> Result<(), String> {
    let path = log_dir().join("pot-o-desktop.log");
    std::fs::write(&path, "").map_err(|e| format!("Cannot clear log: {}", e))
}
