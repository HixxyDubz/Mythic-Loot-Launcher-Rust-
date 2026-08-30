use std::{
    fs::{self, File},
    io::{Read, Write},
    path::Path,
    time::Duration,
};

const REMOTE_TIMEOUT: Duration = Duration::from_secs(20);

pub fn fetch_https(url: &str, maximum_bytes: usize) -> Result<Vec<u8>, String> {
    if !url.starts_with("https://") || url.chars().any(char::is_whitespace) {
        return Err("Remote launcher metadata must use a valid HTTPS URL".into());
    }
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(REMOTE_TIMEOUT))
        .build();
    let agent: ureq::Agent = config.into();
    let mut response = agent
        .get(url)
        .header("User-Agent", "Mythic-Loot-Launcher/0.1")
        .call()
        .map_err(|error| format!("HTTPS request failed: {error}"))?;
    if let Some(length) = response
        .headers()
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok())
        && length > maximum_bytes
    {
        return Err(format!(
            "Remote metadata is larger than the {maximum_bytes}-byte safety limit"
        ));
    }
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(
            u64::try_from(maximum_bytes)
                .unwrap_or(u64::MAX)
                .saturating_add(1),
        )
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Could not read remote metadata: {error}"))?;
    if bytes.len() > maximum_bytes {
        return Err(format!(
            "Remote metadata is larger than the {maximum_bytes}-byte safety limit"
        ));
    }
    Ok(bytes)
}

pub fn write_atomic(destination: &Path, bytes: &[u8]) -> Result<bool, String> {
    if fs::read(destination).ok().as_deref() == Some(bytes) {
        return Ok(false);
    }
    let parent = destination
        .parent()
        .ok_or_else(|| "Remote cache destination has no parent folder".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create remote cache folder: {error}"))?;
    let file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "Remote cache destination has an invalid filename".to_string())?;
    let temporary = parent.join(format!("{file_name}.download"));
    let backup = parent.join(format!("{file_name}.previous"));
    let mut file = File::create(&temporary)
        .map_err(|error| format!("Could not stage remote metadata: {error}"))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("Could not flush remote metadata: {error}"))?;

    if backup.exists() {
        fs::remove_file(&backup)
            .map_err(|error| format!("Could not rotate remote metadata backup: {error}"))?;
    }
    if destination.exists() {
        fs::rename(destination, &backup)
            .map_err(|error| format!("Could not preserve previous remote metadata: {error}"))?;
    }
    if let Err(error) = fs::rename(&temporary, destination) {
        if backup.exists() {
            let _ = fs::rename(&backup, destination);
        }
        return Err(format!(
            "Could not activate verified remote metadata: {error}"
        ));
    }
    if backup.exists() {
        fs::remove_file(backup).ok();
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_cache_write_preserves_the_last_complete_value() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("metadata.json");
        assert!(write_atomic(&path, b"first").unwrap());
        assert!(!write_atomic(&path, b"first").unwrap());
        assert!(write_atomic(&path, b"second").unwrap());
        assert_eq!(fs::read(path).unwrap(), b"second");
    }
}
