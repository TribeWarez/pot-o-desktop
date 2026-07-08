use std::fs;
use std::path::PathBuf;

pub fn app_data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("pot-o-desktop")
}

pub fn ensure_app_dir() -> std::io::Result<PathBuf> {
    let dir = app_data_dir();
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn read_json_file<P: AsRef<std::path::Path>, T: serde::de::DeserializeOwned>(
    path: P,
) -> Option<T> {
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn write_json_file<P: AsRef<std::path::Path>, T: serde::Serialize>(
    path: P,
    value: &T,
) -> std::io::Result<()> {
    let path = path.as_ref();
    let json = serde_json::to_string_pretty(value).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("JSON error: {}", e),
        )
    })?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, &json)?;
    fs::rename(&tmp, path)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CanonicalTipJson {
    pub height: u64,
    pub block_hash: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_data_dir_is_pot_o_desktop() {
        let dir = app_data_dir();
        assert_eq!(dir.file_name().unwrap(), "pot-o-desktop");
    }

    #[test]
    fn test_write_and_read_json() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_storage_json.tmp");
        let tip = CanonicalTipJson {
            height: 42,
            block_hash: "abc".into(),
        };
        write_json_file(&path, &tip).unwrap();
        let loaded: CanonicalTipJson = read_json_file(&path).unwrap();
        assert_eq!(loaded.height, 42);
        std::fs::remove_file(path).ok();
    }
}
