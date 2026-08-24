use std::path::{Component, Path, PathBuf};

const RESERVED_WINDOWS_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

pub fn normalize_relative(value: &str) -> Result<String, String> {
    let candidate = value.replace('\\', "/");
    if candidate.trim().is_empty() || candidate.contains('\0') {
        return Err("the relative path is empty or contains a NUL byte".into());
    }
    if candidate.starts_with('/')
        || candidate.starts_with("//")
        || candidate.as_bytes().get(1) == Some(&b':')
    {
        return Err(format!("absolute paths are not allowed: {value}"));
    }

    let mut clean = Vec::new();
    for part in candidate.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err(format!("parent traversal is not allowed: {value}"));
        }
        if part.contains(':') {
            return Err(format!(
                "drive and alternate stream syntax is not allowed: {value}"
            ));
        }
        if part.ends_with('.') || part.ends_with(' ') {
            return Err(format!(
                "Windows-trimmed path segments are not allowed: {value}"
            ));
        }
        let stem = part.split('.').next().unwrap_or(part).to_ascii_uppercase();
        if RESERVED_WINDOWS_NAMES.contains(&stem.as_str()) {
            return Err(format!(
                "reserved Windows path segment is not allowed: {value}"
            ));
        }
        clean.push(part);
    }

    if clean.is_empty() {
        return Err("the relative path does not name a file or folder".into());
    }
    Ok(clean.join("/"))
}

pub fn safe_join(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let normalized = normalize_relative(relative)?;
    let joined = normalized
        .split('/')
        .fold(root.to_path_buf(), |path, part| path.join(part));
    if joined.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) && !root.is_absolute()
    {
        return Err(format!("unsafe destination path: {relative}"));
    }
    Ok(joined)
}

pub fn validate_archive_member(name: &str, is_directory: bool) -> Result<String, String> {
    let trimmed = if is_directory {
        name.trim_end_matches(['/', '\\'])
    } else {
        name
    };
    normalize_relative(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_and_normalizes_safe_relative_paths() {
        assert_eq!(
            normalize_relative("mods\\example.jar").unwrap(),
            "mods/example.jar"
        );
        assert_eq!(
            validate_archive_member("config/nested/", true).unwrap(),
            "config/nested"
        );
    }

    #[test]
    fn rejects_traversal_absolute_ads_reserved_and_ambiguous_paths() {
        for unsafe_path in [
            "../outside",
            "mods/../../outside",
            "C:\\outside",
            "\\\\server\\share",
            "/rooted",
            "mods/file.jar:stream",
            "CON",
            "mods/NUL.txt",
            "mods/trailing. ",
            "mods/trailing.",
            "mods/evil\0.jar",
            ".",
            "",
        ] {
            assert!(
                normalize_relative(unsafe_path).is_err(),
                "accepted unsafe path {unsafe_path:?}"
            );
        }
    }

    #[test]
    fn safe_join_never_escapes_through_relative_segments() {
        let root = Path::new("C:/launcher/staging");
        assert_eq!(
            safe_join(root, "mods/example.jar").unwrap(),
            root.join("mods").join("example.jar")
        );
        assert!(safe_join(root, "../outside").is_err());
    }
}
