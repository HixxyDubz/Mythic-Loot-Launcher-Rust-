use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::{
    manifest::{self, ChangelogEntry, Manifest, RulesGuide},
    models::GameProfile,
    remote, safe_path, storage,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestContentInput {
    pub announcement: String,
    pub news_banner_url: String,
    pub rules_guide: RulesGuide,
    pub changelog: Vec<ChangelogEntry>,
}

pub fn save_for_profile(
    app: &AppHandle,
    profile: &GameProfile,
    content: ManifestContentInput,
) -> Result<bool, String> {
    let destination = safe_path::safe_join(&storage::data_dir(app)?, &profile.manifest_path)?;
    let loaded = manifest::load_for_profile(app, profile);
    let mut candidate = if loaded.summary.valid {
        loaded.manifest
    } else if !destination.exists() {
        draft_manifest(profile)?
    } else {
        return Err(format!(
            "The existing local manifest is invalid and was left unchanged: {}",
            loaded.summary.errors.join("; ")
        ));
    };

    apply_content(&mut candidate, profile, content)?;
    write_manifest(&destination, &candidate)
}

fn draft_manifest(profile: &GameProfile) -> Result<Manifest, String> {
    if profile.required_modpack_version.trim().is_empty() {
        return Err("Set a modpack version before saving public content".into());
    }
    Ok(Manifest {
        manifest_version: "1.0".into(),
        profile_id: profile.id.clone(),
        game: profile.game.clone(),
        display_name: profile.display_name.clone(),
        required_game_version: profile.required_game_version.clone(),
        modpack_version: profile.required_modpack_version.clone(),
        discord_invite: profile.discord_invite.clone(),
        ..Manifest::default()
    })
}

fn apply_content(
    manifest: &mut Manifest,
    profile: &GameProfile,
    content: ManifestContentInput,
) -> Result<(), String> {
    let mut candidate = manifest.clone();
    candidate.announcement = content.announcement;
    candidate.news_banner_url = content.news_banner_url;
    candidate.rules_guide = content.rules_guide;
    candidate.changelog = content.changelog;
    let errors = manifest::validate(&candidate, Some(profile));
    if !errors.is_empty() {
        return Err(format!(
            "Content changes failed manifest validation: {}",
            errors.join("; ")
        ));
    }
    *manifest = candidate;
    Ok(())
}

fn write_manifest(path: &Path, manifest: &Manifest) -> Result<bool, String> {
    let mut bytes = serde_json::to_vec_pretty(manifest)
        .map_err(|error| format!("Could not serialize the trusted manifest: {error}"))?;
    bytes.push(b'\n');
    remote::write_atomic(path, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{FileEntry, UpdatePart};

    fn profile() -> GameProfile {
        let mut profile = crate::models::LauncherConfig::default().profiles.remove(0);
        profile.id = "fixture".into();
        profile.game = "minecraft".into();
        profile
    }

    fn manifest() -> Manifest {
        Manifest {
            manifest_version: "1.0".into(),
            profile_id: "fixture".into(),
            game: "minecraft".into(),
            modpack_version: "4.2.0".into(),
            update_url: "https://example.invalid/pack.zip".into(),
            update_sha256: "a".repeat(64),
            update_parts: vec![UpdatePart {
                url: "https://example.invalid/pack.zip.001".into(),
                sha256: "b".repeat(64),
                size: 42,
            }],
            files: vec![FileEntry {
                path: "mods/example.jar".into(),
                size: 42,
                hash: "c".repeat(64),
                download_url: "https://example.invalid/example.jar".into(),
                required: true,
                category: "mods".into(),
            }],
            obsolete_files: vec!["mods/old.jar".into()],
            ..Manifest::default()
        }
    }

    fn content() -> ManifestContentInput {
        ManifestContentInput {
            announcement: "A real announcement".into(),
            news_banner_url: "https://example.invalid/banner.webp".into(),
            rules_guide: RulesGuide {
                how_to_join: "Install the current pack.".into(),
                rules: vec!["Be kind.".into()],
                common_fixes: vec!["Run Repair.".into()],
            },
            changelog: vec![ChangelogEntry {
                version: "4.2.0".into(),
                date: "2026-08-31".into(),
                added: vec!["New balance pass".into()],
                changed: Vec::new(),
                fixed: vec!["Startup issue".into()],
                notes: "Release notes".into(),
            }],
        }
    }

    #[test]
    fn content_changes_preserve_every_distribution_field() {
        let mut manifest = manifest();
        let before = serde_json::json!({
            "manifestVersion": manifest.manifest_version,
            "profileId": manifest.profile_id,
            "game": manifest.game,
            "displayName": manifest.display_name,
            "requiredGameVersion": manifest.required_game_version,
            "modpackVersion": manifest.modpack_version,
            "updateUrl": manifest.update_url,
            "updateSha256": manifest.update_sha256,
            "updateParts": manifest.update_parts,
            "releaseDate": manifest.release_date,
            "discordInvite": manifest.discord_invite,
            "newsBannerPath": manifest.news_banner_path,
            "minecraftBaseModLoader": manifest.minecraft_base_mod_loader,
            "minecraftInstanceName": manifest.minecraft_instance_name,
            "files": manifest.files,
            "obsoleteFiles": manifest.obsolete_files,
            "optionalFiles": manifest.optional_files,
        });
        apply_content(&mut manifest, &profile(), content()).unwrap();
        let after = serde_json::json!({
            "manifestVersion": manifest.manifest_version,
            "profileId": manifest.profile_id,
            "game": manifest.game,
            "displayName": manifest.display_name,
            "requiredGameVersion": manifest.required_game_version,
            "modpackVersion": manifest.modpack_version,
            "updateUrl": manifest.update_url,
            "updateSha256": manifest.update_sha256,
            "updateParts": manifest.update_parts,
            "releaseDate": manifest.release_date,
            "discordInvite": manifest.discord_invite,
            "newsBannerPath": manifest.news_banner_path,
            "minecraftBaseModLoader": manifest.minecraft_base_mod_loader,
            "minecraftInstanceName": manifest.minecraft_instance_name,
            "files": manifest.files,
            "obsoleteFiles": manifest.obsolete_files,
            "optionalFiles": manifest.optional_files,
        });
        assert_eq!(before, after);
        assert_eq!(manifest.announcement, "A real announcement");
        assert_eq!(manifest.rules_guide.rules, vec!["Be kind."]);
    }

    #[test]
    fn unsafe_banner_and_oversized_content_are_rejected_without_mutation() {
        let mut manifest = manifest();
        let original = manifest.clone();
        let mut input = content();
        input.news_banner_url = "http://example.invalid/banner.png".into();
        input.announcement = "x".repeat(20_001);
        let error = apply_content(&mut manifest, &profile(), input).unwrap_err();
        assert!(error.contains("newsBannerUrl uses an unsupported URL scheme"));
        assert!(error.contains("announcement exceeds 20000 characters"));
        assert_eq!(manifest.announcement, original.announcement);
        assert_eq!(manifest.update_url, original.update_url);
    }

    #[test]
    fn drafts_use_real_profile_identity_before_the_first_package_release() {
        let profile = profile();
        let manifest = draft_manifest(&profile).unwrap();
        assert_eq!(manifest.profile_id, profile.id);
        assert_eq!(manifest.game, profile.game);
        assert_eq!(manifest.modpack_version, profile.required_modpack_version);
        assert!(manifest::validate(&manifest, Some(&profile)).is_empty());
    }
}
