use std::{
    fs::{self, File},
    io::{BufReader, Read, Write},
    path::Path,
};

use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use tauri::AppHandle;
#[cfg(test)]
use zip::ZipArchive;
use zip::{CompressionMethod, DateTime, ZipWriter, write::SimpleFileOptions};

use crate::{manifest::Manifest, models::GameProfile, storage};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftBootstrapRequest {
    pub profile_id: String,
    pub launcher: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftBootstrapArtifact {
    pub launcher: String,
    pub file_name: String,
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
    pub message: String,
}

pub fn prepare(
    app: &AppHandle,
    request: &MinecraftBootstrapRequest,
) -> Result<MinecraftBootstrapArtifact, String> {
    let config = storage::load_or_create(app)?;
    let profile = config
        .profiles
        .iter()
        .find(|profile| profile.id == request.profile_id)
        .ok_or_else(|| "That modpack profile does not exist".to_string())?;
    let loaded = crate::manifest::load_for_profile(app, profile);
    if !loaded.summary.valid {
        return Err(format!(
            "The trusted Minecraft manifest is not valid: {}",
            loaded.summary.errors.join("; ")
        ));
    }
    let output = storage::data_dir(app)?.join("minecraft-bootstrap");
    prepare_at(profile, &loaded.manifest, &request.launcher, &output)
}

fn prepare_at(
    profile: &GameProfile,
    manifest: &Manifest,
    launcher: &str,
    output_dir: &Path,
) -> Result<MinecraftBootstrapArtifact, String> {
    if profile.game != "minecraft" || manifest.game != "minecraft" {
        return Err("Bootstrap profiles are available only for Minecraft modpacks".into());
    }
    let launcher = launcher.trim().to_ascii_lowercase();
    if !matches!(launcher.as_str(), "curseforge" | "modrinth") {
        return Err("Choose CurseForge or Modrinth as the Minecraft launcher".into());
    }
    let game_version = nonempty(&manifest.required_game_version)
        .or_else(|| nonempty(&profile.required_game_version))
        .ok_or_else(|| "The trusted manifest does not declare a Minecraft version".to_string())?;
    let loader_name = manifest
        .minecraft_base_mod_loader
        .get("name")
        .and_then(serde_json::Value::as_str)
        .and_then(nonempty)
        .ok_or_else(|| {
            "The trusted manifest does not declare a Minecraft mod loader".to_string()
        })?;
    let (loader_kind, loader_version) = split_loader(loader_name)?;
    let instance_name = nonempty(&manifest.minecraft_instance_name)
        .or_else(|| nonempty(&profile.display_name))
        .unwrap_or("Mythic Loot Minecraft");
    let pack_version = nonempty(&manifest.modpack_version)
        .or_else(|| nonempty(&profile.required_modpack_version))
        .unwrap_or("bootstrap");

    fs::create_dir_all(output_dir).map_err(|error| {
        format!(
            "Could not create Minecraft bootstrap folder {}: {error}",
            output_dir.display()
        )
    })?;
    let safe_name = safe_file_stem(instance_name);
    let extension = if launcher == "curseforge" {
        "zip"
    } else {
        "mrpack"
    };
    let file_name = format!("{safe_name}-{launcher}-bootstrap.{extension}");
    let destination = output_dir.join(&file_name);
    let temporary = output_dir.join(format!("{file_name}.tmp"));

    let metadata = if launcher == "curseforge" {
        json!({
            "minecraft": {
                "version": game_version,
                "modLoaders": [{ "id": loader_name, "primary": true }]
            },
            "manifestType": "minecraftModpack",
            "manifestVersion": 1,
            "name": instance_name,
            "version": pack_version,
            "author": "Mythic Loot",
            "files": [],
            "overrides": "overrides"
        })
    } else {
        let mut dependencies = serde_json::Map::new();
        dependencies.insert("minecraft".into(), json!(game_version));
        dependencies.insert(loader_kind.into(), json!(loader_version));
        json!({
            "formatVersion": 1,
            "game": "minecraft",
            "versionId": format!("mythic-loot-{pack_version}"),
            "name": instance_name,
            "summary": "Bootstrap profile for Mythic Loot Launcher synchronization",
            "files": [],
            "dependencies": dependencies
        })
    };
    let metadata_name = if launcher == "curseforge" {
        "manifest.json"
    } else {
        "modrinth.index.json"
    };
    write_bootstrap_archive(&temporary, metadata_name, &metadata)?;
    if destination.exists() {
        fs::remove_file(&destination).map_err(|error| {
            format!(
                "Could not replace previous bootstrap {}: {error}",
                destination.display()
            )
        })?;
    }
    fs::rename(&temporary, &destination).map_err(|error| {
        format!(
            "Could not activate bootstrap {}: {error}",
            destination.display()
        )
    })?;
    let bytes = destination
        .metadata()
        .map_err(|error| format!("Could not inspect {}: {error}", destination.display()))?
        .len();
    let sha256 = sha256_file(&destination)?;
    let message = if launcher == "curseforge" {
        "Bootstrap ZIP ready. Import it in CurseForge, then detect the new profile and synchronize it."
    } else {
        "Bootstrap .mrpack ready. Open it with Modrinth, then detect the new profile and synchronize it."
    };
    Ok(MinecraftBootstrapArtifact {
        launcher,
        file_name,
        path: destination.display().to_string(),
        bytes,
        sha256,
        message: message.into(),
    })
}

fn write_bootstrap_archive(
    path: &Path,
    metadata_name: &str,
    metadata: &serde_json::Value,
) -> Result<(), String> {
    let file = File::create(path)
        .map_err(|error| format!("Could not create bootstrap {}: {error}", path.display()))?;
    let mut archive = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .compression_level(Some(6))
        .last_modified_time(DateTime::default())
        .unix_permissions(0o644);
    archive
        .start_file(metadata_name, options)
        .map_err(|error| format!("Could not add {metadata_name}: {error}"))?;
    let bytes = serde_json::to_vec_pretty(metadata)
        .map_err(|error| format!("Could not encode {metadata_name}: {error}"))?;
    archive
        .write_all(&bytes)
        .map_err(|error| format!("Could not write {metadata_name}: {error}"))?;
    archive
        .add_directory("overrides/", options)
        .map_err(|error| format!("Could not add empty overrides folder: {error}"))?;
    archive
        .finish()
        .map_err(|error| format!("Could not finish bootstrap archive: {error}"))?;
    Ok(())
}

fn split_loader(name: &str) -> Result<(&str, &str), String> {
    for kind in ["neoforge", "forge", "fabric-loader", "quilt-loader"] {
        if let Some(version) = name.strip_prefix(&format!("{kind}-"))
            && !version.trim().is_empty()
        {
            return Ok((kind, version));
        }
    }
    Err(format!("Unsupported Minecraft mod loader identity: {name}"))
}

fn safe_file_stem(value: &str) -> String {
    let filtered: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, ' ' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = filtered.trim().trim_end_matches('.');
    if trimmed.is_empty() {
        "Mythic Loot Minecraft".into()
    } else {
        trimmed.into()
    }
}

fn nonempty(value: &str) -> Option<&str> {
    (!value.trim().is_empty()).then(|| value.trim())
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let input =
        File::open(path).map_err(|error| format!("Could not hash {}: {error}", path.display()))?;
    let mut input = BufReader::new(input);
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = input
            .read(&mut buffer)
            .map_err(|error| format!("Could not hash {}: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::LauncherConfig;
    use tempfile::TempDir;

    fn fixture() -> (GameProfile, Manifest) {
        let profile = LauncherConfig::default().profiles.remove(0);
        let manifest: Manifest =
            serde_json::from_str(include_str!("../resources/manifests/minecraft_main.json"))
                .unwrap();
        (profile, manifest)
    }

    fn archive_json(path: &Path, name: &str) -> serde_json::Value {
        let mut archive = ZipArchive::new(File::open(path).unwrap()).unwrap();
        let mut text = String::new();
        archive
            .by_name(name)
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        serde_json::from_str(&text).unwrap()
    }

    #[test]
    fn creates_a_minimal_curseforge_import_without_modpack_or_personal_state() {
        let root = TempDir::new().unwrap();
        let (profile, manifest) = fixture();
        let artifact = prepare_at(&profile, &manifest, "curseforge", root.path()).unwrap();
        let json = archive_json(Path::new(&artifact.path), "manifest.json");
        assert_eq!(json["minecraft"]["version"], "1.21.1");
        assert_eq!(
            json["minecraft"]["modLoaders"][0]["id"],
            "neoforge-21.1.248"
        );
        assert_eq!(json["files"], json!([]));
        assert!(!serde_json::to_string(&json).unwrap().contains("Users"));
    }

    #[test]
    fn creates_a_minimal_modrinth_import_with_supported_dependencies() {
        let root = TempDir::new().unwrap();
        let (profile, manifest) = fixture();
        let artifact = prepare_at(&profile, &manifest, "modrinth", root.path()).unwrap();
        let json = archive_json(Path::new(&artifact.path), "modrinth.index.json");
        assert_eq!(json["formatVersion"], 1);
        assert_eq!(json["dependencies"]["minecraft"], "1.21.1");
        assert_eq!(json["dependencies"]["neoforge"], "21.1.248");
        assert_eq!(json["files"], json!([]));
    }

    #[test]
    fn rejects_unknown_launchers_and_non_minecraft_profiles() {
        let root = TempDir::new().unwrap();
        let (profile, manifest) = fixture();
        assert!(prepare_at(&profile, &manifest, "unknown", root.path()).is_err());
        let other = LauncherConfig::default().profiles.remove(1);
        assert!(prepare_at(&other, &manifest, "curseforge", root.path()).is_err());
    }
}
