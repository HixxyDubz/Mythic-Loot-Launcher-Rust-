//! Read-only, allowlisted projection of local Minecraft metadata. Raw launcher
//! JSON, account fields, arguments and download URLs never cross the IPC boundary.
use crate::{
    manifest::{self, Manifest},
    operations::WorkGuard,
    safe_path, storage,
};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Read,
    path::Path,
};
use tauri::AppHandle;

const MAX_METADATA_BYTES: u64 = 8 * 1024 * 1024;
const FILES: [&str; 3] = [
    "minecraftinstance.json",
    "manifest.json",
    "modrinth.index.json",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataEvidence {
    file_name: String,
    instance_metadata: bool,
    game_version: String,
    mod_loader: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MetadataComparison {
    Matches,
    Mismatch,
    Unknown,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftMetadataInspection {
    pub profile_id: String,
    directory: String,
    pub game_version: Option<String>,
    pub mod_loader: Option<String>,
    pub can_use: bool,
    sources: Vec<MetadataEvidence>,
    issues: Vec<String>,
    expected_game_version: Option<String>,
    expected_mod_loader: Option<String>,
    comparison: MetadataComparison,
}

#[tauri::command]
pub async fn inspect_minecraft_metadata(
    app: AppHandle,
    profile_id: String,
    directory: String,
) -> Result<MinecraftMetadataInspection, String> {
    let guard = WorkGuard::begin()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let config = storage::load_or_create(&app)?;
        let profile = config
            .profiles
            .iter()
            .find(|profile| profile.id == profile_id && profile.game == "minecraft")
            .ok_or("Choose a saved Minecraft profile before inspecting metadata")?;
        let loaded = manifest::load_for_profile(&app, profile);
        inspect_at(
            Path::new(directory.trim()),
            &profile_id,
            loaded.summary.valid.then_some(&loaded.manifest),
        )
    })
    .await
    .map_err(|_| "Minecraft metadata inspection could not finish".to_string())?
}

fn inspect_at(
    directory: &Path,
    profile_id: &str,
    expected: Option<&Manifest>,
) -> Result<MinecraftMetadataInspection, String> {
    if !directory.is_absolute() {
        return Err("Choose an existing absolute Minecraft folder".into());
    }
    safe_path::reject_link_path(directory)?;
    if !directory.is_dir() {
        return Err("Choose an existing Minecraft folder".into());
    }
    let mut result = MinecraftMetadataInspection {
        profile_id: profile_id.into(),
        directory: directory.display().to_string(),
        game_version: None,
        mod_loader: None,
        can_use: false,
        sources: vec![],
        issues: vec![],
        expected_game_version: None,
        expected_mod_loader: None,
        comparison: MetadataComparison::Unknown,
    };
    for name in FILES {
        let path = directory.join(name);
        let exists = match fs::symlink_metadata(&path) {
            Ok(_) => true,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(_) => {
                result
                    .issues
                    .push(format!("{name}: cannot inspect metadata file"));
                continue;
            }
        };
        if !exists {
            continue;
        }
        match read_evidence(&path, name) {
            Ok(evidence) => result.sources.push(evidence),
            Err(reason) => result.issues.push(format!("{name}: {reason}")),
        }
    }
    if result.sources.is_empty() && result.issues.is_empty() {
        result.issues.push("No supported JSON metadata found. Inspect a CurseForge instance or a folder containing CurseForge/Modrinth export metadata. Database-only Modrinth instances and other formats require manual checks; no database or account files were opened.".into());
    }
    let versions: BTreeSet<_> = result
        .sources
        .iter()
        .map(|source| source.game_version.clone())
        .collect();
    let loaders: BTreeSet<_> = result
        .sources
        .iter()
        .map(|source| source.mod_loader.clone())
        .collect();
    if versions.len() == 1 {
        result.game_version = versions.into_iter().next();
    } else if versions.len() > 1 {
        result.issues.push("Metadata files disagree about the Minecraft version. Review the files; no version was selected.".into());
    }
    if loaders.len() == 1 {
        result.mod_loader = loaders.into_iter().next();
    } else if loaders.len() > 1 {
        result.issues.push("Metadata files disagree about the mod loader. Review the files; no loader was selected.".into());
    }
    result.can_use =
        result.issues.is_empty() && result.game_version.is_some() && result.mod_loader.is_some();
    if let Some(expected) = expected {
        result.expected_game_version = token(&expected.required_game_version, 64).ok();
        result.expected_mod_loader = expected
            .minecraft_base_mod_loader
            .get("name")
            .and_then(Value::as_str)
            .and_then(|name| normalize_loader(name, &expected.required_game_version).ok());
        if result.can_use {
            let mismatch = differs(&result.game_version, &result.expected_game_version)
                || differs(&result.mod_loader, &result.expected_mod_loader);
            if mismatch {
                result.comparison = MetadataComparison::Mismatch;
            } else if result.expected_game_version.is_some()
                && result.expected_mod_loader.is_some()
                && result.sources.iter().any(|source| source.instance_metadata)
            {
                result.comparison = MetadataComparison::Matches;
            }
        }
    }
    Ok(result)
}

fn differs(left: &Option<String>, right: &Option<String>) -> bool {
    matches!((left, right), (Some(left), Some(right)) if left != right)
}

fn read_evidence(path: &Path, name: &str) -> Result<MetadataEvidence, String> {
    safe_path::reject_link_path(path).map_err(|_| "linked metadata paths are not inspected")?;
    let input = File::open(path).map_err(|_| "cannot read metadata file")?;
    let metadata = input
        .metadata()
        .map_err(|_| "cannot inspect metadata size")?;
    if !metadata.is_file() || metadata.len() > MAX_METADATA_BYTES {
        return Err("metadata must be a regular file no larger than 8 MiB".into());
    }
    let mut bytes = Vec::new();
    input
        .take(MAX_METADATA_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "cannot read metadata file")?;
    if bytes.len() as u64 > MAX_METADATA_BYTES {
        return Err("metadata grew beyond the 8 MiB limit".into());
    }
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
    // Do not include serde errors/field values in user-facing errors: unknown
    // fields may contain account or other private launcher state.
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| "invalid JSON metadata; raw contents were not returned")?;
    parse_evidence(name, &value)
}

fn parse_evidence(name: &str, value: &Value) -> Result<MetadataEvidence, String> {
    let (game_version, mod_loader) = match name {
        "minecraftinstance.json" => {
            let game_version = token(
                value
                    .get("gameVersion")
                    .and_then(Value::as_str)
                    .ok_or("missing Minecraft version")?,
                64,
            )?;
            let base = value
                .get("baseModLoader")
                .ok_or("missing mod loader metadata")?;
            for field in ["minecraftVersion", "MinecraftVersion"] {
                if let Some(other) = base.get(field).filter(|other| !other.is_null())
                    && other.as_str() != Some(game_version.as_str())
                {
                    return Err("instance and loader Minecraft versions disagree".into());
                }
            }
            let loader = normalize_loader(
                base.get("name")
                    .and_then(Value::as_str)
                    .ok_or("no supported mod loader declared; review manually")?,
                &game_version,
            )?;
            (game_version, loader)
        }
        "manifest.json" => {
            if value.get("manifestType").and_then(Value::as_str) != Some("minecraftModpack")
                || value.get("manifestVersion").and_then(Value::as_u64) != Some(1)
            {
                return Err("not a supported CurseForge export manifest".into());
            }
            let minecraft = value.get("minecraft").ok_or("missing Minecraft metadata")?;
            let game_version = token(
                minecraft
                    .get("version")
                    .and_then(Value::as_str)
                    .ok_or("missing Minecraft version")?,
                64,
            )?;
            let loaders = minecraft
                .get("modLoaders")
                .and_then(Value::as_array)
                .ok_or("missing mod loader list")?;
            let primary: Vec<_> = loaders
                .iter()
                .filter(|loader| loader.get("primary").and_then(Value::as_bool) == Some(true))
                .collect();
            let selected = if primary.len() == 1 {
                primary[0]
            } else if primary.is_empty() && loaders.len() == 1 {
                &loaders[0]
            } else {
                return Err("no unambiguous primary mod loader; review manually".into());
            };
            let loader = normalize_loader(
                selected
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or("missing loader identity")?,
                &game_version,
            )?;
            (game_version, loader)
        }
        "modrinth.index.json" => {
            if value.get("formatVersion").and_then(Value::as_u64) != Some(1)
                || value.get("game").and_then(Value::as_str) != Some("minecraft")
            {
                return Err("not a supported Modrinth export index".into());
            }
            let dependencies = value
                .get("dependencies")
                .and_then(Value::as_object)
                .ok_or("missing dependencies")?;
            if dependencies.keys().any(|key| {
                !matches!(
                    key.as_str(),
                    "minecraft" | "neoforge" | "forge" | "fabric-loader" | "quilt-loader"
                )
            }) {
                return Err("unsupported dependency identifiers; review manually".into());
            }
            let game_version = token(
                dependencies
                    .get("minecraft")
                    .and_then(Value::as_str)
                    .ok_or("missing Minecraft version")?,
                64,
            )?;
            let loaders: Vec<_> = dependencies
                .iter()
                .filter(|(key, _)| key.as_str() != "minecraft")
                .collect();
            if loaders.len() != 1 {
                return Err(
                    "exactly one supported mod loader must be declared; review manually".into(),
                );
            }
            let (kind, version) = loaders[0];
            let version = token(version.as_str().ok_or("invalid loader version")?, 100)?;
            let loader = normalize_loader(&format!("{kind}-{version}"), &game_version)?;
            (game_version, loader)
        }
        _ => return Err("unsupported metadata file".into()),
    };
    Ok(MetadataEvidence {
        file_name: name.into(),
        instance_metadata: name == "minecraftinstance.json",
        game_version,
        mod_loader,
    })
}

fn token(value: &str, limit: usize) -> Result<String, String> {
    if value.is_empty()
        || value.len() > limit
        || !value.bytes().any(|byte| byte.is_ascii_digit())
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._+-".contains(&byte))
    {
        return Err("invalid or unsupported version identifier".into());
    }
    Ok(value.into())
}

fn normalize_loader(name: &str, game_version: &str) -> Result<String, String> {
    // Legacy CurseForge Fabric/Quilt identities include the Minecraft suffix;
    // legacy Forge identities may include a matching Minecraft prefix.
    for (legacy, canonical) in [("fabric-", "fabric-loader"), ("quilt-", "quilt-loader")] {
        if let Some(version) = name
            .strip_prefix(legacy)
            .and_then(|tail| tail.strip_suffix(&format!("-{game_version}")))
        {
            return Ok(format!("{canonical}-{}", token(version, 100)?));
        }
    }
    let (kind, version) = crate::minecraft_setup::split_loader(name)
        .map_err(|_| "unsupported loader identity; confirm its exact name/version manually")?;
    let version = if kind == "forge" {
        version
            .strip_prefix(&format!("{game_version}-"))
            .unwrap_or(version)
    } else {
        version
    };
    Ok(format!("{kind}-{}", token(version, 100)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn curseforge(version: &str, loader: &str) -> Value {
        json!({"gameVersion":version,"baseModLoader":{"name":loader}})
    }
    fn expected() -> Manifest {
        Manifest {
            game: "minecraft".into(),
            required_game_version: "1.21.1".into(),
            minecraft_base_mod_loader: json!({"name":"neoforge-21.1.248"}),
            ..Manifest::default()
        }
    }
    fn put(root: &Path, file: &str, value: &Value) {
        fs::write(root.join(file), serde_json::to_vec(value).unwrap()).unwrap();
    }

    #[test]
    fn projects_only_version_and_loader_and_never_changes_source_bytes() {
        let root = tempfile::tempdir().unwrap();
        let mut data = curseforge("1.21.1", "neoforge-21.1.248");
        data["account"] = json!({"accessToken":"private-value","userName":"private-name"});
        data["baseModLoader"]["versionJson"] = json!("private-arguments");
        put(root.path(), FILES[0], &data);
        let before = fs::read(root.path().join(FILES[0])).unwrap();
        let result = inspect_at(root.path(), "pack", Some(&expected())).unwrap();
        assert!(result.can_use);
        assert_eq!(result.comparison, MetadataComparison::Matches);
        assert_eq!(result.game_version.as_deref(), Some("1.21.1"));
        assert_eq!(result.mod_loader.as_deref(), Some("neoforge-21.1.248"));
        assert!(!serde_json::to_string(&result).unwrap().contains("private-"));
        assert_eq!(fs::read(root.path().join(FILES[0])).unwrap(), before);
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }

    #[test]
    fn normalizes_all_supported_loader_families() {
        for (input, expected) in [
            ("forge-1.20.1-47.4.0", "forge-47.4.0"),
            ("fabric-0.16.10-1.20.1", "fabric-loader-0.16.10"),
            ("quilt-0.27.1-1.20.1", "quilt-loader-0.27.1"),
            ("neoforge-21.1.248", "neoforge-21.1.248"),
        ] {
            assert_eq!(normalize_loader(input, "1.20.1").unwrap(), expected);
        }
    }

    #[test]
    fn export_metadata_is_not_proof_of_installed_compatibility() {
        let root = tempfile::tempdir().unwrap();
        put(
            root.path(),
            FILES[2],
            &json!({"formatVersion":1,"game":"minecraft","dependencies":{"minecraft":"1.21.1","neoforge":"21.1.248"}}),
        );
        let result = inspect_at(root.path(), "pack", Some(&expected())).unwrap();
        assert!(result.can_use);
        assert_eq!(result.comparison, MetadataComparison::Unknown);
    }

    #[test]
    fn conflicting_files_cannot_be_auto_applied_or_reported_matching() {
        let root = tempfile::tempdir().unwrap();
        put(
            root.path(),
            FILES[0],
            &curseforge("1.21.1", "neoforge-21.1.248"),
        );
        put(
            root.path(),
            FILES[2],
            &json!({"formatVersion":1,"game":"minecraft","dependencies":{"minecraft":"1.20.1","fabric-loader":"0.16.10"}}),
        );
        let result = inspect_at(root.path(), "pack", Some(&expected())).unwrap();
        assert!(!result.can_use);
        assert!(result.game_version.is_none() && result.mod_loader.is_none());
        assert_eq!(result.comparison, MetadataComparison::Unknown);
    }

    #[test]
    fn valid_instance_does_not_override_broken_or_conflicting_export() {
        let root = tempfile::tempdir().unwrap();
        put(
            root.path(),
            FILES[0],
            &curseforge("1.21.1", "neoforge-21.1.248"),
        );
        fs::write(root.path().join(FILES[1]), b"private broken export").unwrap();
        let broken = inspect_at(root.path(), "pack", Some(&expected())).unwrap();
        assert!(!broken.can_use);
        assert_eq!(broken.comparison, MetadataComparison::Unknown);
        put(
            root.path(),
            FILES[1],
            &json!({"manifestType":"minecraftModpack","manifestVersion":1,"minecraft":{"version":"1.21.1","modLoaders":[{"id":"neoforge-21.1.249","primary":true}]}}),
        );
        let conflicting = inspect_at(root.path(), "pack", Some(&expected())).unwrap();
        assert!(!conflicting.can_use);
        assert_eq!(conflicting.game_version.as_deref(), Some("1.21.1"));
        assert!(conflicting.mod_loader.is_none());
        assert_eq!(conflicting.comparison, MetadataComparison::Unknown);
    }

    #[test]
    fn missing_unknown_and_malformed_metadata_stay_unknown() {
        let root = tempfile::tempdir().unwrap();
        for bytes in [
            None,
            Some(b"{not valid private-json".as_slice()),
            Some(br#"{"formatVersion":99,"game":"minecraft"}"#.as_slice()),
        ] {
            if let Some(bytes) = bytes {
                fs::write(root.path().join(FILES[2]), bytes).unwrap();
            }
            let result = inspect_at(root.path(), "pack", Some(&expected())).unwrap();
            assert!(!result.can_use);
            assert_eq!(result.comparison, MetadataComparison::Unknown);
            assert!(
                !serde_json::to_string(&result)
                    .unwrap()
                    .contains("private-json")
            );
        }
    }

    #[test]
    fn detects_game_and_loader_mismatches_separately() {
        for (version, loader) in [
            ("1.20.1", "neoforge-21.1.248"),
            ("1.21.1", "neoforge-21.1.249"),
        ] {
            let root = tempfile::tempdir().unwrap();
            put(root.path(), FILES[0], &curseforge(version, loader));
            assert_eq!(
                inspect_at(root.path(), "pack", Some(&expected()))
                    .unwrap()
                    .comparison,
                MetadataComparison::Mismatch
            );
        }
    }

    #[test]
    fn refuses_oversized_files_and_directories_without_reading_contents() {
        let root = tempfile::tempdir().unwrap();
        File::create(root.path().join(FILES[0]))
            .unwrap()
            .set_len(MAX_METADATA_BYTES + 1)
            .unwrap();
        fs::create_dir(root.path().join(FILES[1])).unwrap();
        let result = inspect_at(root.path(), "pack", None).unwrap();
        assert!(!result.can_use);
        assert_eq!(result.issues.len(), 2);
    }

    #[test]
    fn handles_bom_and_primary_curseforge_loader_without_copying_other_fields() {
        let root = tempfile::tempdir().unwrap();
        let data = json!({"manifestType":"minecraftModpack","manifestVersion":1,"minecraft":{"version":"1.20.1","modLoaders":[{"id":"forge-47.4.0","primary":true},{"id":"private-unused","primary":false}]},"author":"private-author"});
        let mut bytes = vec![0xef, 0xbb, 0xbf];
        bytes.extend(serde_json::to_vec(&data).unwrap());
        fs::write(root.path().join(FILES[1]), bytes).unwrap();
        let result = inspect_at(root.path(), "pack", None).unwrap();
        assert!(result.can_use);
        assert!(!serde_json::to_string(&result).unwrap().contains("private-"));
    }

    #[test]
    fn refuses_ambiguous_future_or_unsafe_identifiers() {
        for dependencies in [
            json!({"minecraft":"1.21.1","fabric-loader":"0.16.10","forge":"47.4.0"}),
            json!({"minecraft":"1.21.1","future-loader":"1.0"}),
            json!({"minecraft":"C:\\private","neoforge":"21.1.248"}),
        ] {
            assert!(
                parse_evidence(
                    FILES[2],
                    &json!({"formatVersion":1,"game":"minecraft","dependencies":dependencies})
                )
                .is_err()
            );
        }
        assert!(normalize_loader("fabric-0.16.10-1.20.1", "1.21.1").is_err());
        assert!(parse_evidence(FILES[0], &json!({"gameVersion":"1.21.1","baseModLoader":{"name":"neoforge-21.1.248","minecraftVersion":"1.20.1"}})).is_err());
    }

    #[test]
    #[ignore = "Explicit read-only acceptance against a user-selected Minecraft folder"]
    fn inspect_real_source_read_only() {
        let directory = std::env::var_os("MYTHIC_LOOT_METADATA_SMOKE_DIR")
            .expect("Set the explicit read-only source folder");
        let result = inspect_at(Path::new(&directory), "acceptance", None).unwrap();
        assert!(result.can_use, "Metadata did not resolve unambiguously");
        println!(
            "Minecraft: {}; loader: {}",
            result.game_version.unwrap(),
            result.mod_loader.unwrap()
        );
    }
}
