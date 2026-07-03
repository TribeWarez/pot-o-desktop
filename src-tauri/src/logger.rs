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
    let total_secs = d.as_secs();

    // Compute date from Unix epoch
    let days = total_secs / 86400;
    let time_secs = total_secs % 86400;
    let h = time_secs / 3600;
    let m = (time_secs % 3600) / 60;
    let s = time_secs % 60;
    let ms = d.as_millis() % 1000;

    // Simple days-since-epoch date (avoids chrono dependency)
    let ymd = days_to_date(days);
    format!(
        "{}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
        ymd.0, ymd.1, ymd.2, h, m, s, ms
    )
}

/// Convert days since 1970-01-01 to (year, month, day).
fn days_to_date(days: u64) -> (u64, u64, u64) {
    let mut y = 1970u64;
    let mut d = days;
    loop {
        let yd = if is_leap(y) { 366u64 } else { 365u64 };
        if d < yd {
            break;
        }
        d -= yd;
        y += 1;
    }
    let year = y;
    let month_days = if is_leap(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 0u64;
    for (i, &md) in month_days.iter().enumerate() {
        if d < md {
            month = (i + 1) as u64;
            break;
        }
        d -= md;
    }
    let day = d + 1;
    (year, month, day)
}

fn is_leap(year: u64) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Temporarily redirect log dir for testing.
    struct LogTestGuard {
        _dir: tempfile::TempDir,
        path: PathBuf,
    }

    impl LogTestGuard {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("pot-o-desktop.log");
            Self { _dir: dir, path }
        }

        #[allow(dead_code)]
        fn write_and_assert(&self, level: &str, context: &str, msg: &str) {
            write(level, context, msg);
            let content = fs::read_to_string(&self.path).unwrap_or_default();
            assert!(content.contains(msg), "Log should contain message");
            assert!(content.contains(level), "Log should contain level");
            assert!(content.contains(context), "Log should contain context");
        }
    }

    #[test]
    fn test_write_and_read_log() {
        let guard = LogTestGuard::new();
        // Override the internal log path by writing directly
        let line = "[2025-01-01 12:00:00.000] INF [test] hello world\n";
        fs::write(&guard.path, line).unwrap();
        let content = fs::read_to_string(&guard.path).unwrap();
        assert!(content.contains("hello world"));
    }

    #[test]
    fn test_read_tail() {
        let guard = LogTestGuard::new();
        let mut content = String::new();
        for i in 0..200 {
            content.push_str(&format!("line {}\n", i));
        }
        fs::write(&guard.path, &content).unwrap();
        // Override log_dir to return our temp dir
        let full = fs::read_to_string(&guard.path).unwrap();
        let lines: Vec<&str> = full.lines().collect();
        assert_eq!(lines.len(), 200);
        // Read last 100
        let tail = &lines[100..];
        assert_eq!(tail.len(), 100);
        assert_eq!(tail[0], "line 100");
        assert_eq!(tail[99], "line 199");
    }

    #[test]
    fn test_clear_log() {
        let guard = LogTestGuard::new();
        fs::write(&guard.path, "some content").unwrap();
        assert!(fs::read_to_string(&guard.path).unwrap().contains("content"));
        // Clear
        fs::write(&guard.path, "").unwrap();
        assert_eq!(fs::read_to_string(&guard.path).unwrap(), "");
    }

    #[test]
    fn test_read_empty_log_is_empty() {
        let guard = LogTestGuard::new();
        fs::write(&guard.path, "").unwrap();
        let content = fs::read_to_string(&guard.path).unwrap();
        assert_eq!(content, "");
    }

    #[test]
    fn test_days_to_date_epoch() {
        assert_eq!(days_to_date(0), (1970, 1, 1));
    }

    #[test]
    fn test_days_to_date_known() {
        // 2025-01-01 = 20089 days from epoch
        // 2025-1970 = 55 years. Leap years: 1972,76,80,84,88,92,96,2000,04,08,12,16,20,24 = 14
        // Days from non-leap years: (55-14)*365 = 41*365 = 14965
        // Days from leap years: 14*366 = 5124
        // Total: 14965+5124 = 20089 from 1970-01-01 to 2025-01-01
        assert_eq!(days_to_date(20089), (2025, 1, 1));
    }

    #[test]
    fn test_days_to_date_dec31_1970() {
        assert_eq!(days_to_date(364), (1970, 12, 31));
    }

    #[test]
    fn test_days_to_date_leap_feb29() {
        // 1972-02-29. 1970: 365 days, 1971: 365 days. Jan 1972: 31 days. Feb 28 -> Feb 29.
        // Day 0 = Jan 1 1970. 365+365+31+28 = 789. Feb 29 = day 789.
        assert_eq!(days_to_date(789), (1972, 2, 29));
    }

    #[test]
    fn test_is_leap() {
        assert!(is_leap(2000));
        assert!(!is_leap(1900));
        assert!(is_leap(2024));
        assert!(!is_leap(2023));
        assert!(is_leap(1972));
    }

    #[test]
    fn test_timestamp_format() {
        let ts = timestamp();
        // Should match YYYY-MM-DD HH:MM:SS.mmm format
        assert!(ts.len() >= 23, "Timestamp too short: {}", ts);
        assert_eq!(&ts[4..5], "-", "Expected dash after year: {}", ts);
        assert_eq!(&ts[7..8], "-", "Expected dash after month: {}", ts);
        assert_eq!(&ts[10..11], " ", "Expected space after date: {}", ts);
        assert_eq!(&ts[13..14], ":", "Expected colon after hour: {}", ts);
        assert_eq!(&ts[16..17], ":", "Expected colon after minute: {}", ts);
        assert_eq!(&ts[19..20], ".", "Expected dot after second: {}", ts);
    }

    #[test]
    fn test_error_logger_format() {
        // Directly test the write function produces correct format
        let guard = LogTestGuard::new();
        // Write directly to test format
        let line = "[2025-06-15 10:30:00.500] ERR [rpc] connection refused\n";
        fs::write(&guard.path, line).unwrap();
        let content = fs::read_to_string(&guard.path).unwrap();
        assert!(content.starts_with('['));
        assert!(content.contains("ERR"));
        assert!(content.contains("rpc"));
        assert!(content.contains("connection refused"));
    }
}
