use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::AppHandle;
use time::{OffsetDateTime, format_description::well_known::Iso8601};

use crate::{
    catalog::{
        self, CATALOG_BRANCH, CATALOG_FILE_PATH, CATALOG_REPOSITORY, CatalogProfile, PublicCatalog,
    },
    models::LauncherConfig,
    publisher, storage,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogProfilePreview {
    pub id: String,
    pub display_name: String,
    pub version: String,
    pub manifest_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPreview {
    pub preview_id: String,
    pub repository: String,
    pub branch: String,
    pub public_url: String,
    pub output_path: String,
    pub bytes: u64,
    pub sha256: String,
    pub profiles: Vec<CatalogProfilePreview>,
    pub hidden_profiles: usize,
    pub issues: Vec<String>,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPublication {
    pub repository: String,
    pub branch: String,
    pub public_url: String,
    pub commit_url: String,
    pub profiles: usize,
    pub message: String,
}

#[derive(Debug, Clone)]
struct CatalogPlan {
    output_dir: PathBuf,
    catalog_path: PathBuf,
    sha256: String,
    generated_at: String,
    profiles: usize,
}

#[derive(Serialize)]
struct GitHubContentRequest<'a> {
    message: String,
    content: String,
    branch: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    sha: Option<String>,
}

static CATALOG_PLANS: OnceLock<Mutex<HashMap<String, CatalogPlan>>> = OnceLock::new();

pub fn prepare(app: &AppHandle) -> Result<CatalogPreview, String> {
    let config = storage::load_or_create(app)?;
    let output_root = storage::data_dir(app)?.join("catalog-previews");
    let generated_at = OffsetDateTime::now_utc()
        .format(&Iso8601::DEFAULT)
        .map_err(|error| format!("Could not format the catalogue timestamp: {error}"))?;
    prepare_at(&config, &output_root, &generated_at, true)
}

pub fn publish(preview_id: &str, confirmed: bool) -> Result<CatalogPublication, String> {
    if !confirmed {
        return Err("Public catalogue publication requires explicit confirmation".into());
    }
    let plan = catalog_plans()
        .lock()
        .map_err(|_| "Catalogue preview cache is unavailable".to_string())?
        .get(preview_id)
        .cloned()
        .ok_or_else(|| "Prepare a fresh public catalogue preview before publishing".to_string())?;
    validate_plan(&plan)?;

    let status = publisher::status();
    if !status.gh_available || !status.authenticated {
        return Err(status.message);
    }
    let endpoint = format!("repos/{CATALOG_REPOSITORY}/contents/{CATALOG_FILE_PATH}");
    let lookup_endpoint = format!("{endpoint}?ref={CATALOG_BRANCH}");
    let existing = publisher::run_gh(["api", lookup_endpoint.as_str(), "--jq", ".sha"])?;
    let current_sha = if existing.status.success() {
        let value = String::from_utf8_lossy(&existing.stdout).trim().to_string();
        if value.is_empty() {
            return Err("GitHub returned an empty SHA for the existing public catalogue".into());
        }
        Some(value)
    } else {
        let message = publisher::output_message(&existing, "GitHub catalogue lookup failed");
        let lower = message.to_ascii_lowercase();
        if lower.contains("http 404") || lower.contains("not found") {
            None
        } else {
            return Err(format!(
                "Could not inspect the current public catalogue: {message}"
            ));
        }
    };

    let bytes = fs::read(&plan.catalog_path)
        .map_err(|error| format!("Could not reopen the reviewed public catalogue: {error}"))?;
    let request = GitHubContentRequest {
        message: format!("Publish launcher catalogue {}", plan.generated_at),
        content: STANDARD.encode(bytes),
        branch: CATALOG_BRANCH,
        sha: current_sha,
    };
    let request_path = plan.output_dir.join("github-content-request.json");
    let request_bytes = serde_json::to_vec(&request)
        .map_err(|error| format!("Could not serialize the GitHub catalogue request: {error}"))?;
    fs::write(&request_path, request_bytes)
        .map_err(|error| format!("Could not stage the GitHub catalogue request: {error}"))?;
    let request_path_text = request_path.to_string_lossy().to_string();
    let output = publisher::run_gh([
        "api",
        "--method",
        "PUT",
        endpoint.as_str(),
        "--input",
        request_path_text.as_str(),
        "--jq",
        ".commit.html_url",
    ]);
    fs::remove_file(&request_path).ok();
    let output = output?;
    if !output.status.success() {
        return Err(publisher::output_message(
            &output,
            "GitHub could not publish the public catalogue",
        ));
    }
    let commit_url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    catalog_plans()
        .lock()
        .map_err(|_| "Catalogue preview cache is unavailable".to_string())?
        .remove(preview_id);
    Ok(CatalogPublication {
        repository: CATALOG_REPOSITORY.into(),
        branch: CATALOG_BRANCH.into(),
        public_url: catalog::DEFAULT_CATALOG_URL.into(),
        commit_url,
        profiles: plan.profiles,
        message: format!(
            "Published {} public modpack profile{} for Player startup.",
            plan.profiles,
            if plan.profiles == 1 { "" } else { "s" }
        ),
    })
}

fn prepare_at(
    config: &LauncherConfig,
    output_root: &Path,
    generated_at: &str,
    remember_plan: bool,
) -> Result<CatalogPreview, String> {
    let hidden_profiles = config
        .profiles
        .iter()
        .filter(|profile| !profile.catalog_visible)
        .count();
    let profiles: Vec<_> = config
        .profiles
        .iter()
        .filter(|profile| profile.catalog_visible)
        .map(|profile| CatalogProfile {
            id: profile.id.clone(),
            game: profile.game.clone(),
            display_name: profile.display_name.clone(),
            required_game_version: profile.required_game_version.clone(),
            required_modpack_version: profile.required_modpack_version.clone(),
            manifest_url: profile.manifest_url.clone(),
            deployment_subdir: profile.deployment_subdir.clone(),
            logo_url: profile.logo_path.clone(),
            discord_invite: profile.discord_invite.clone(),
        })
        .collect();
    let catalog = PublicCatalog {
        catalog_version: "1.0".into(),
        generated_at: generated_at.into(),
        profiles,
    };
    let profile_previews = catalog
        .profiles
        .iter()
        .map(|profile| CatalogProfilePreview {
            id: profile.id.clone(),
            display_name: profile.display_name.clone(),
            version: profile.required_modpack_version.clone(),
            manifest_url: profile.manifest_url.clone(),
        })
        .collect::<Vec<_>>();
    let mut issues = Vec::new();
    if catalog.profiles.is_empty() {
        issues.push("At least one modpack must be visible in the Player catalogue".into());
    }
    if let Err(error) = catalog::validate(&catalog) {
        issues.push(error);
    }
    let mut preview = CatalogPreview {
        preview_id: String::new(),
        repository: CATALOG_REPOSITORY.into(),
        branch: CATALOG_BRANCH.into(),
        public_url: catalog::DEFAULT_CATALOG_URL.into(),
        output_path: String::new(),
        bytes: 0,
        sha256: String::new(),
        profiles: profile_previews,
        hidden_profiles,
        issues,
        ready: false,
    };
    if !preview.issues.is_empty() {
        return Ok(preview);
    }

    let mut bytes = serde_json::to_vec_pretty(&catalog)
        .map_err(|error| format!("Could not serialize the public catalogue: {error}"))?;
    bytes.push(b'\n');
    catalog::parse(&bytes)?;
    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    let preview_id = sha256[..16].to_string();
    let output_dir = output_root.join(&preview_id);
    fs::create_dir_all(&output_dir)
        .map_err(|error| format!("Could not create the catalogue preview folder: {error}"))?;
    let output_dir = fs::canonicalize(&output_dir)
        .map_err(|error| format!("Could not resolve the catalogue preview folder: {error}"))?;
    let catalog_path = output_dir.join(CATALOG_FILE_PATH);
    fs::write(&catalog_path, &bytes)
        .map_err(|error| format!("Could not write the public catalogue preview: {error}"))?;

    preview.preview_id = preview_id.clone();
    preview.output_path = catalog_path.display().to_string();
    preview.bytes = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    preview.sha256 = sha256.clone();
    preview.ready = true;
    if remember_plan {
        catalog_plans()
            .lock()
            .map_err(|_| "Catalogue preview cache is unavailable".to_string())?
            .insert(
                preview_id,
                CatalogPlan {
                    output_dir,
                    catalog_path,
                    sha256,
                    generated_at: generated_at.into(),
                    profiles: catalog.profiles.len(),
                },
            );
    }
    Ok(preview)
}

fn validate_plan(plan: &CatalogPlan) -> Result<(), String> {
    let path = fs::canonicalize(&plan.catalog_path)
        .map_err(|error| format!("The reviewed public catalogue is unavailable: {error}"))?;
    if !path.starts_with(&plan.output_dir) || !path.is_file() {
        return Err("The reviewed public catalogue escaped its native preview folder".into());
    }
    let bytes = fs::read(&path)
        .map_err(|error| format!("Could not re-read the reviewed public catalogue: {error}"))?;
    if format!("{:x}", Sha256::digest(&bytes)) != plan.sha256 {
        return Err("The reviewed public catalogue changed after preview; prepare it again".into());
    }
    catalog::parse(&bytes)?;
    Ok(())
}

fn catalog_plans() -> &'static Mutex<HashMap<String, CatalogPlan>> {
    CATALOG_PLANS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_preview_contains_only_public_profile_state() {
        let root = tempfile::tempdir().unwrap();
        let mut config = LauncherConfig::default();
        config.profiles[0].install_dir = "C:\\Users\\Owner\\Private Pack".into();
        config.profiles[0].game_exe_path = "C:\\Secret\\launcher.exe".into();
        config.profiles[1].catalog_visible = false;
        let preview = prepare_at(&config, root.path(), "2026-08-30T18:00:00Z", false).unwrap();
        assert!(preview.ready);
        assert_eq!(preview.profiles.len(), 1);
        assert_eq!(preview.hidden_profiles, 1);
        let bytes = fs::read(preview.output_path).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(!text.contains("Private Pack"));
        assert!(!text.contains("launcher.exe"));
        assert!(!text.contains("localModpackVersion"));
    }

    #[test]
    fn visible_profile_without_manifest_is_blocked() {
        let root = tempfile::tempdir().unwrap();
        let mut config = LauncherConfig::default();
        config.profiles[0].manifest_url.clear();
        let preview = prepare_at(&config, root.path(), "2026-08-30T18:00:00Z", false).unwrap();
        assert!(!preview.ready);
        assert!(
            preview
                .issues
                .iter()
                .any(|issue| issue.contains("manifestUrl"))
        );
    }

    #[test]
    fn publication_is_fail_closed_without_confirmation() {
        assert!(
            publish("not-a-preview", false)
                .unwrap_err()
                .contains("explicit confirmation")
        );
    }
}
