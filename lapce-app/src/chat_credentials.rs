//! App-local endpoint-bound keys. Never serialized into general settings or Debug.
use std::{collections::BTreeMap, path::Path, io::Write};
use lapce_core::directory::Directory;

fn identity(endpoint: &str) -> Result<String, String> {
    crate::chat::completion_url(endpoint)
}
fn location() -> Result<std::path::PathBuf, String> {
    Directory::config_directory().map(|dir| dir.join("llm-credentials.json"))
        .ok_or_else(|| "No editor config directory".into())
}
fn read(path: &Path) -> Result<BTreeMap<String, String>, String> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| "Credential file is invalid; not overwritten".into()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(_) => Err("Cannot read credential file".into()),
    }
}
pub fn saved_key(endpoint: &str) -> Option<String> {
    read(&location().ok()?).ok()?.remove(&identity(endpoint).ok()?)
}
pub fn save(endpoint: &str, key: &str) -> Result<(), String> {
    save_at(&location()?, endpoint, key)
}
fn save_at(path: &Path, endpoint: &str, key: &str) -> Result<(), String> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = LOCK.lock().map_err(|_| "Credential store is busy")?;
    let id = identity(endpoint)?;
    let mut values = read(path)?;
    if key.trim().is_empty() { values.remove(&id); }
    else { values.insert(id, key.trim().into()); }
    let bytes = serde_json::to_vec(&values).map_err(|_| "Cannot encode credentials")?;
    let temp = path.with_extension(format!("tmp-{}", std::process::id()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)] {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    // Acquire ownership before arranging cleanup. A stale file or symlink
    // must never be removed just because create_new refused to open it.
    let mut file = options.open(&temp).map_err(|_| "Cannot create private credential file")?;
    let result: Result<(), String> = (|| {
        file.write_all(&bytes).map_err(|_| "Cannot write credentials")?;
        file.sync_all().map_err(|_| "Cannot flush credentials")?;
        // Release the handle before rename (required on some platforms).
        drop(file);
        std::fs::rename(&temp, path).map_err(|_| "Cannot replace credentials")?;
        Ok(())
    })();
    if result.is_err() { let _ = std::fs::remove_file(&temp); }
    result
}

/// Never echo input. Key entry is append/backspace/paste/clear only.
pub fn append_key(value: &mut String, text: &str) {
    if !text.chars().any(char::is_control) && value.len() + text.len() <= 4096 {
        value.push_str(text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn endpoint_bound_roundtrip_clear_and_permissions() {
        let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("chat-key-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("credentials.json");
        save_at(&path, "https://example.test", "fixture-key").unwrap();
        assert_eq!(read(&path).unwrap().get(&identity("https://example.test/v1").unwrap()).map(String::as_str), Some("fixture-key"));
        assert!(!read(&path).unwrap().contains_key(&identity("https://other.test").unwrap()));
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        }
        save_at(&path, "https://example.test", "").unwrap();
        assert!(read(&path).unwrap().is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn existing_temp_file_and_credentials_are_not_deleted_on_create_failure() {
        let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("chat-key-stale-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("credentials.json");
        save_at(&path, "https://example.test", "old-fixture").unwrap();
        let original = std::fs::read(&path).unwrap();
        let temp = path.with_extension(format!("tmp-{}", std::process::id()));
        std::fs::write(&temp, b"existing temporary data").unwrap();
        assert!(save_at(&path, "https://example.test", "new-fixture").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert_eq!(std::fs::read(&temp).unwrap(), b"existing temporary data");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn malformed_credential_store_is_not_overwritten() {
        let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("chat-key-invalid-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("credentials.json");
        std::fs::write(&path, b"invalid metadata").unwrap();
        assert!(save_at(&path, "https://example.test", "fixture").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"invalid metadata");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn input_rejects_controls_and_overlong_keys() {
        let mut key = String::new();
        append_key(&mut key, "abc"); append_key(&mut key, "\n");
        append_key(&mut key, &"x".repeat(4096));
        assert_eq!(key, "abc");
    }
}
