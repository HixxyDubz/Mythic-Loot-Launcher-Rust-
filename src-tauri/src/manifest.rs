use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::AppHandle;

use crate::{models::GameProfile, safe_path, storage};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct FileEntry {
    pub path: String,
    pub size: i64,
    pub hash: String,
    pub download_url: String,
    pub required: bool,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct UpdatePart {
    pub url: String,
    pub sha256: String,
    pub size: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Manifest {
    pub manifest_version: String,
    pub profile_id: String,
    pub game: String,
    pub display_name: String,
    pub server_name: String,
    pub server_ip: String,
    pub server_port: u16,
    pub required_game_version: String,
    pub modpack_version: String,
    pub update_url: String,
    pub update_sha256: String,
    pub update_parts: Vec<UpdatePart>,
    pub release_date: String,
    pub discord_invite: String,
    pub announcement: String,
    pub news_banner_path: String,
    pub news_banner_url: String,
    pub minecraft_base_mod_loader: serde_json::Value,
    pub minecraft_instance_name: String,
    pub rules_guide: serde_json::Value,
    pub changelog: Vec<serde_json::Value>,
    pub files: Vec<FileEntry>,
    pub obsolete_files: Vec<String>,
    pub optional_files: Vec<FileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestSummary {
    pub profile_id: String,
    pub valid: bool,
    pub manifest_version: String,
    pub modpack_version: String,
    pub release_date: String,
    pub required_file_count: usize,
    pub optional_file_count: usize,
    pub obsolete_file_count: usize,
    pub update_size: Option<u64>,
    pub source: String,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileVerification {
    pub profile_id: String,
    pub checked: usize,
    pub current: usize,
    pub missing: Vec<String>,
    pub changed: Vec<String>,
    pub unsafe_entries: Vec<String>,
}

pub struct LoadedManifest {
    pub manifest: Manifest,
    pub summary: ManifestSummary,
}

pub fn load_for_profile(app: &AppHandle, profile: &GameProfile) -> LoadedManifest {
    match load_source(app, profile) {
        Ok((content, source)) => match serde_json::from_str::<Manifest>(&content) {
            Ok(manifest) => {
                let errors = validate(&manifest, Some(profile));
                let summary = summarize(&manifest, source, errors);
                LoadedManifest { manifest, summary }
            }
            Err(error) => invalid_loaded(
                profile,
                format!("configured manifest could not be parsed: {error}"),
            ),
        },
        Err(error) => invalid_loaded(profile, error),
    }
}

fn invalid_loaded(profile: &GameProfile, error: String) -> LoadedManifest {
    LoadedManifest {
        manifest: Manifest {
            profile_id: profile.id.clone(),
            ..Manifest::default()
        },
        summary: ManifestSummary {
            profile_id: profile.id.clone(),
            valid: false,
            manifest_version: String::new(),
            modpack_version: String::new(),
            release_date: String::new(),
            required_file_count: 0,
            optional_file_count: 0,
            obsolete_file_count: 0,
            update_size: None,
            source: "unavailable".into(),
            errors: vec![error],
        },
    }
}

fn load_source(app: &AppHandle, profile: &GameProfile) -> Result<(String, String), String> {
    let data_path = safe_path::safe_join(&storage::data_dir(app)?, &profile.manifest_path)?;
    if data_path.is_file() {
        let content = std::fs::read_to_string(&data_path)
            .map_err(|error| format!("failed to read {}: {error}", data_path.display()))?;
        return Ok((content, data_path.display().to_string()));
    }

    let embedded = match profile.id.as_str() {
        "minecraft_main" => include_str!("../resources/manifests/minecraft_main.json"),
        "seven_days_main" => include_str!("../resources/manifests/seven_days_main.json"),
        _ => {
            return Err(format!(
                "No local manifest exists for {} at {}",
                profile.display_name,
                data_path.display()
            ));
        }
    };
    Ok((embedded.into(), "bundled launcher manifest".into()))
}

fn summarize(manifest: &Manifest, source: String, errors: Vec<String>) -> ManifestSummary {
    let update_size = manifest
        .update_parts
        .iter()
        .try_fold(0_u64, |total, part| {
            u64::try_from(part.size)
                .ok()
                .and_then(|size| total.checked_add(size))
        })
        .filter(|size| *size > 0);
    ManifestSummary {
        profile_id: manifest.profile_id.clone(),
        valid: errors.is_empty(),
        manifest_version: manifest.manifest_version.clone(),
        modpack_version: manifest.modpack_version.clone(),
        release_date: manifest.release_date.clone(),
        required_file_count: manifest.files.len(),
        optional_file_count: manifest.optional_files.len(),
        obsolete_file_count: manifest.obsolete_files.len(),
        update_size,
        source,
        errors,
    }
}

pub fn validate(manifest: &Manifest, expected: Option<&GameProfile>) -> Vec<String> {
    let mut errors = Vec::new();
    required(&manifest.manifest_version, "manifestVersion", &mut errors);
    required(&manifest.profile_id, "profileId", &mut errors);
    required(&manifest.game, "game", &mut errors);
    required(&manifest.modpack_version, "modpackVersion", &mut errors);
    if manifest.manifest_version.split('.').next() != Some("1") {
        errors.push(format!(
            "Unsupported manifest version {}; this launcher supports major version 1",
            manifest.manifest_version
        ));
    }
    if let Some(profile) = expected {
        if manifest.profile_id != profile.id {
            errors.push(format!(
                "Manifest profileId {} does not match {}",
                manifest.profile_id, profile.id
            ));
        }
        if manifest.game != profile.game {
            errors.push(format!(
                "Manifest game {} does not match {}",
                manifest.game, profile.game
            ));
        }
    }
    if !manifest.server_ip.trim().is_empty() && manifest.server_port == 0 {
        errors.push("serverPort must be between 1 and 65535 when serverIp is set".into());
    }
    validate_url(&manifest.update_url, "updateUrl", false, &mut errors);
    validate_url(
        &manifest.news_banner_url,
        "newsBannerUrl",
        true,
        &mut errors,
    );
    validate_sha(&manifest.update_sha256, "updateSha256", true, &mut errors);
    if !manifest.news_banner_path.is_empty()
        && let Err(error) = safe_path::normalize_relative(&manifest.news_banner_path)
    {
        errors.push(format!("unsafe newsBannerPath: {error}"));
    }

    let mut claimed: HashMap<String, &'static str> = HashMap::new();
    validate_entries(&manifest.files, "files", &mut claimed, &mut errors);
    validate_entries(
        &manifest.optional_files,
        "optionalFiles",
        &mut claimed,
        &mut errors,
    );
    let mut obsolete = HashSet::new();
    for path in &manifest.obsolete_files {
        match safe_path::normalize_relative(path) {
            Ok(normalized) => {
                let key = normalized.to_ascii_lowercase();
                if !obsolete.insert(key.clone()) {
                    errors.push(format!("duplicate obsolete path: {path}"));
                }
                if let Some(kind) = claimed.get(&key) {
                    errors.push(format!("{path} is both obsolete and present in {kind}"));
                }
            }
            Err(error) => errors.push(format!("unsafe obsolete path {path}: {error}")),
        }
    }
    for (index, part) in manifest.update_parts.iter().enumerate() {
        validate_url(
            &part.url,
            &format!("updateParts[{index}].url"),
            false,
            &mut errors,
        );
        validate_sha(
            &part.sha256,
            &format!("updateParts[{index}].sha256"),
            false,
            &mut errors,
        );
        if part.size < 0 {
            errors.push(format!("updateParts[{index}].size cannot be negative"));
        }
    }
    errors
}

fn validate_entries(
    entries: &[FileEntry],
    kind: &'static str,
    claimed: &mut HashMap<String, &'static str>,
    errors: &mut Vec<String>,
) {
    for entry in entries {
        match safe_path::validate_archive_member(&entry.path, false) {
            Ok(normalized) => {
                let key = normalized.to_ascii_lowercase();
                if let Some(previous) = claimed.insert(key, kind) {
                    errors.push(format!(
                        "duplicate path {} appears in {previous} and {kind}",
                        entry.path
                    ));
                }
            }
            Err(error) => errors.push(format!("unsafe {kind} path {}: {error}", entry.path)),
        }
        if entry.size < 0 {
            errors.push(format!("{} has a negative size", entry.path));
        }
        validate_sha(
            &entry.hash,
            &format!("hash for {}", entry.path),
            false,
            errors,
        );
        validate_url(
            &entry.download_url,
            &format!("downloadUrl for {}", entry.path),
            false,
            errors,
        );
    }
}

fn required(value: &str, label: &str, errors: &mut Vec<String>) {
    if value.trim().is_empty() {
        errors.push(format!("{label} is required"));
    }
}

fn validate_sha(value: &str, label: &str, allow_empty: bool, errors: &mut Vec<String>) {
    if value.is_empty() && allow_empty {
        return;
    }
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        errors.push(format!("{label} must be a 64-character SHA-256"));
    }
}

fn validate_url(value: &str, label: &str, https_only: bool, errors: &mut Vec<String>) {
    if value.trim().is_empty() {
        return;
    }
    let lower = value.to_ascii_lowercase();
    let accepted = if https_only {
        value.starts_with("https://")
    } else {
        value.starts_with("https://")
            || value.starts_with("http://")
            || value.starts_with("file://")
            || safe_path::normalize_relative(value).is_ok()
    };
    if !accepted {
        errors.push(format!("{label} uses an unsupported URL scheme"));
    }
    if (label == "updateUrl" || label.starts_with("updateParts[")) && is_discord_invite(&lower) {
        errors.push(format!("{label} must not use a Discord invitation"));
    }
}

pub fn is_discord_invite(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("discord.gg/") || lower.contains("discord.com/invite/")
}

pub fn verify_required_files(
    profile: &GameProfile,
    manifest: &Manifest,
) -> Result<FileVerification, String> {
    let root = PathBuf::from(profile.install_dir.trim());
    if !root.is_dir() {
        return Err("Choose an existing modpack folder before verifying files".into());
    }
    let mut result = FileVerification {
        profile_id: profile.id.clone(),
        checked: 0,
        current: 0,
        missing: Vec::new(),
        changed: Vec::new(),
        unsafe_entries: Vec::new(),
    };
    for entry in &manifest.files {
        result.checked += 1;
        let path = match safe_path::safe_join(&root, &entry.path) {
            Ok(path) => path,
            Err(_) => {
                result.unsafe_entries.push(entry.path.clone());
                continue;
            }
        };
        if !path.is_file() {
            result.missing.push(entry.path.clone());
            continue;
        }
        let size_matches =
            path.metadata().ok().map(|meta| meta.len()) == u64::try_from(entry.size).ok();
        if !size_matches || sha256(&path)? != entry.hash.to_ascii_lowercase() {
            result.changed.push(entry.path.clone());
            continue;
        }
        result.current += 1;
    }
    Ok(result)
}

fn sha256(path: &Path) -> Result<String, String> {
    let file =
        File::open(path).map_err(|error| format!("failed to open {}: {error}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|error| format!("failed to hash {}: {error}", path.display()))?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_manifest() -> Manifest {
        Manifest {
            manifest_version: "1.0".into(),
            profile_id: "fixture".into(),
            game: "minecraft".into(),
            modpack_version: "2.0.0".into(),
            files: vec![FileEntry {
                path: "mods/example.jar".into(),
                size: 7,
                hash: "239f59ed55e737c77147cf55ad0c1b030b6d7ee748a7426952f9b852d5a935e5".into(),
                required: true,
                ..FileEntry::default()
            }],
            ..Manifest::default()
        }
    }

    #[test]
    fn accepts_a_valid_v1_manifest() {
        assert!(validate(&valid_manifest(), None).is_empty());
        assert!(!is_discord_invite("https://github.com/release.zip"));
        assert!(is_discord_invite("HTTPS://DISCORD.GG/not-an-update"));
    }

    #[test]
    fn bundled_release_manifests_parse_and_pass_the_safety_contract() {
        let minecraft: Manifest =
            serde_json::from_str(include_str!("../resources/manifests/minecraft_main.json"))
                .unwrap();
        let seven_days: Manifest =
            serde_json::from_str(include_str!("../resources/manifests/seven_days_main.json"))
                .unwrap();
        assert_eq!(minecraft.files.len(), 2_067);
        assert_eq!(minecraft.obsolete_files.len(), 2);
        assert!(validate(&minecraft, None).is_empty());
        assert!(validate(&seven_days, None).is_empty());
    }

    #[test]
    fn rejects_unsupported_versions_bad_hashes_collisions_and_overlap() {
        let mut manifest = valid_manifest();
        manifest.manifest_version = "2.0".into();
        manifest.files[0].hash = "bad".into();
        manifest.optional_files.push(FileEntry {
            path: "MODS/EXAMPLE.JAR".into(),
            hash: "a".repeat(64),
            ..FileEntry::default()
        });
        manifest.obsolete_files.push("mods/example.jar".into());
        manifest.update_url = "https://discord.gg/not-an-update".into();
        let errors = validate(&manifest, None).join("\n");
        assert!(errors.contains("Unsupported manifest version"));
        assert!(errors.contains("must be a 64-character SHA-256"));
        assert!(errors.contains("duplicate path"));
        assert!(errors.contains("both obsolete"));
        assert!(errors.contains("must not use a Discord invitation"));
    }

    #[test]
    fn verifies_size_and_sha256_without_trusting_version_text() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("mods")).unwrap();
        std::fs::write(root.path().join("mods/example.jar"), b"payload").unwrap();
        let mut profile = crate::models::LauncherConfig::default().profiles.remove(0);
        profile.id = "fixture".into();
        profile.install_dir = root.path().display().to_string();
        let result = verify_required_files(&profile, &valid_manifest()).unwrap();
        assert!(result.missing.is_empty());
        assert!(result.changed.is_empty());
        assert!(result.unsafe_entries.is_empty());

        std::fs::write(root.path().join("mods/example.jar"), b"changed").unwrap();
        let result = verify_required_files(&profile, &valid_manifest()).unwrap();
        assert_eq!(result.changed, vec!["mods/example.jar"]);
    }
}
