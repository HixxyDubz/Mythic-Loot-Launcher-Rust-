use std::{
    collections::{HashMap, HashSet},
    env,
    fs::{self, File},
    io::{self, BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::AppHandle;
use time::{Date, format_description::well_known::Iso8601};
use walkdir::WalkDir;
use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter, write::SimpleFileOptions};

use crate::{
    manifest::{self, FileEntry, Manifest, UpdatePart},
    models::GameProfile,
    publisher, safe_path, storage,
};

const TEXT_SCAN_LIMIT: u64 = 32 * 1024 * 1024;
const SINGLE_ASSET_LIMIT: u64 = 2 * 1024 * 1024 * 1024;
const MULTIPART_PART_SIZE: u64 = 1024 * 1024 * 1024;
const MAX_RELEASE_PARTS: usize = 999;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageRequest {
    pub profile_id: String,
    pub source_dir: String,
    pub version: String,
    pub release_date: String,
    pub repository: String,
    pub release_notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackagePreview {
    pub preview_id: String,
    pub profile_id: String,
    pub version: String,
    pub tag: String,
    pub repository: String,
    pub source_dir: String,
    pub output_dir: String,
    pub package_path: String,
    pub manifest_path: String,
    pub file_count: usize,
    pub excluded_count: usize,
    pub total_bytes: u64,
    pub package_bytes: u64,
    pub package_sha256: String,
    pub multipart: bool,
    pub assets: Vec<PackageAssetPreview>,
    pub added: usize,
    pub changed: usize,
    pub removed: usize,
    pub issues: Vec<String>,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageAssetPreview {
    pub file_name: String,
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleasePublication {
    pub profile_id: String,
    pub version: String,
    pub repository: String,
    pub tag: String,
    pub manifest_url: String,
    pub url: String,
    pub message: String,
}

#[derive(Debug, Clone)]
struct SourceFile {
    absolute: PathBuf,
    relative: String,
    size: u64,
    hash: String,
}

#[derive(Debug, Clone)]
struct ReleasePlan {
    profile_id: String,
    version: String,
    repository: String,
    tag: String,
    title: String,
    notes: String,
    output_dir: PathBuf,
    assets: Vec<ReleaseAsset>,
    manifest_path: PathBuf,
    manifest_file_name: String,
    manifest_sha256: String,
}

#[derive(Debug, Clone)]
struct ReleaseAsset {
    file_name: String,
    path: PathBuf,
    bytes: u64,
    sha256: String,
}

static RELEASE_PLANS: OnceLock<Mutex<HashMap<String, ReleasePlan>>> = OnceLock::new();

pub fn prepare(app: &AppHandle, request: &PackageRequest) -> Result<PackagePreview, String> {
    let config = storage::load_or_create(app)?;
    let profile = config
        .profiles
        .iter()
        .find(|profile| profile.id == request.profile_id)
        .ok_or_else(|| "That modpack profile does not exist".to_string())?;
    let base = manifest::load_for_profile(app, profile).manifest;
    let output_root = storage::data_dir(app)?.join("publish-previews");
    let username = env::var("USERNAME").unwrap_or_default();
    let user_profile = env::var("USERPROFILE").unwrap_or_default();
    prepare_at(
        profile,
        &base,
        request,
        &output_root,
        &username,
        &user_profile,
        true,
    )
}

fn prepare_at(
    profile: &GameProfile,
    base: &Manifest,
    request: &PackageRequest,
    output_root: &Path,
    username: &str,
    user_profile: &str,
    remember_plan: bool,
) -> Result<PackagePreview, String> {
    prepare_at_with_limits(
        profile,
        base,
        request,
        output_root,
        username,
        user_profile,
        remember_plan,
        SINGLE_ASSET_LIMIT,
        MULTIPART_PART_SIZE,
    )
}

#[allow(clippy::too_many_arguments)]
fn prepare_at_with_limits(
    profile: &GameProfile,
    base: &Manifest,
    request: &PackageRequest,
    output_root: &Path,
    username: &str,
    user_profile: &str,
    remember_plan: bool,
    single_asset_limit: u64,
    multipart_part_size: u64,
) -> Result<PackagePreview, String> {
    let version = validate_request(profile, request)?;
    let repository = request.repository.trim();
    let source = fs::canonicalize(request.source_dir.trim())
        .map_err(|error| format!("Choose an existing source folder: {error}"))?;
    if !source.is_dir() {
        return Err("The selected source path is not a folder".into());
    }
    fs::create_dir_all(output_root).map_err(|error| {
        format!(
            "Could not create publishing workspace {}: {error}",
            output_root.display()
        )
    })?;
    let output_root = fs::canonicalize(output_root).map_err(|error| {
        format!(
            "Could not resolve publishing workspace {}: {error}",
            output_root.display()
        )
    })?;
    if output_root.starts_with(&source) {
        return Err("The publishing workspace must be outside the modpack source folder".into());
    }

    let scan = scan_source(&source, username, user_profile)?;
    let mut issues = scan.issues;
    let total_bytes = scan
        .files
        .iter()
        .try_fold(0_u64, |total, file| total.checked_add(file.size))
        .ok_or_else(|| "The source inventory is too large to represent safely".to_string())?;
    if scan.files.is_empty() {
        issues.push("The source folder contains no publishable modpack files".into());
    }

    let package_name = format!("{}_{}.zip", profile.id, version);
    let manifest_name = format!("{}-manifest.json", profile.id);
    let tag = format!("v{version}");
    let preview_id = preview_id(profile, request, &scan.files);
    let output_dir = output_root.join(&preview_id);
    let package_path = output_dir.join(&package_name);
    let manifest_path = output_dir.join(&manifest_name);

    let (added, changed, removed_paths) = diff(base, &scan.files);
    let blank = String::new();
    let mut preview = PackagePreview {
        preview_id: preview_id.clone(),
        profile_id: profile.id.clone(),
        version: version.clone(),
        tag: tag.clone(),
        repository: repository.into(),
        source_dir: source.display().to_string(),
        output_dir: output_dir.display().to_string(),
        package_path: blank.clone(),
        manifest_path: blank,
        file_count: scan.files.len(),
        excluded_count: scan.excluded_count,
        total_bytes,
        package_bytes: 0,
        package_sha256: String::new(),
        multipart: false,
        assets: Vec::new(),
        added,
        changed,
        removed: removed_paths.len(),
        issues,
        ready: false,
    };
    if !preview.issues.is_empty() {
        return Ok(preview);
    }

    if output_dir.exists() {
        fs::remove_dir_all(&output_dir).map_err(|error| {
            format!(
                "Could not replace prior preview {}: {error}",
                output_dir.display()
            )
        })?;
    }
    fs::create_dir_all(&output_dir).map_err(|error| {
        format!(
            "Could not create preview folder {}: {error}",
            output_dir.display()
        )
    })?;

    create_zip(&package_path, &scan.files)?;
    validate_zip(&package_path, &scan.files)?;
    let package_bytes = package_path
        .metadata()
        .map_err(|error| format!("Could not inspect package: {error}"))?
        .len();
    let package_sha256 = manifest::sha256(&package_path)?;
    let release_root = format!(
        "https://github.com/{}/releases/download/{}",
        repository, tag
    );
    let multipart = package_bytes >= single_asset_limit;
    let assets = prepare_release_assets(
        &package_path,
        &output_dir,
        &package_name,
        multipart,
        multipart_part_size,
    )?;
    let mut generated = base.clone();
    generated.manifest_version = "1.0".into();
    generated.profile_id = profile.id.clone();
    generated.game = profile.game.clone();
    generated.display_name = profile.display_name.clone();
    generated.required_game_version = profile.required_game_version.clone();
    generated.modpack_version = version.clone();
    generated.update_url = if multipart {
        String::new()
    } else {
        format!("{release_root}/{}", assets[0].file_name)
    };
    generated.update_sha256 = package_sha256.clone();
    generated.update_parts = if multipart {
        assets
            .iter()
            .map(|asset| UpdatePart {
                url: format!("{release_root}/{}", asset.file_name),
                sha256: asset.sha256.clone(),
                size: i64::try_from(asset.bytes).unwrap_or(i64::MAX),
            })
            .collect()
    } else {
        Vec::new()
    };
    generated.release_date = request.release_date.clone();
    generated.files = scan.files.iter().map(file_entry).collect();
    generated.obsolete_files = removed_paths;
    generated.optional_files.clear();
    let errors = manifest::validate(&generated, Some(profile));
    if !errors.is_empty() {
        remove_assets(&assets);
        preview.issues = errors
            .into_iter()
            .map(|error| format!("Generated manifest: {error}"))
            .collect();
        return Ok(preview);
    }
    write_json(&manifest_path, &generated)?;
    let manifest_sha256 = manifest::sha256(&manifest_path)?;

    preview.package_path = if multipart {
        String::new()
    } else {
        assets[0].path.display().to_string()
    };
    preview.manifest_path = manifest_path.display().to_string();
    preview.package_bytes = package_bytes;
    preview.package_sha256 = package_sha256.clone();
    preview.multipart = multipart;
    preview.assets = assets.iter().map(asset_preview).collect();
    preview.ready = true;

    if remember_plan {
        let plan = ReleasePlan {
            profile_id: profile.id.clone(),
            version: version.clone(),
            repository: repository.into(),
            tag: tag.clone(),
            title: format!("{} {}", profile.display_name, version),
            notes: request.release_notes.trim().to_string(),
            output_dir,
            assets,
            manifest_path,
            manifest_file_name: manifest_name,
            manifest_sha256,
        };
        release_plans()
            .lock()
            .map_err(|_| "Release preview cache is unavailable".to_string())?
            .insert(preview_id, plan);
    }
    Ok(preview)
}

pub fn publish(preview_id: &str, confirmed: bool) -> Result<ReleasePublication, String> {
    if !confirmed {
        return Err("Release publication requires explicit confirmation".into());
    }
    let plan = release_plans()
        .lock()
        .map_err(|_| "Release preview cache is unavailable".to_string())?
        .get(preview_id)
        .cloned()
        .ok_or_else(|| "Prepare a fresh local release preview before publishing".to_string())?;
    validate_plan(&plan)?;

    let status = publisher::status();
    if !status.gh_available || !status.authenticated {
        return Err(status.message);
    }
    let existing = publisher::run_gh([
        "release",
        "view",
        plan.tag.as_str(),
        "--repo",
        plan.repository.as_str(),
    ])?;
    if existing.status.success() {
        return Err(format!(
            "Release {} already exists in {}; release tags are immutable in this workflow",
            plan.tag, plan.repository
        ));
    }
    let lookup_message = publisher::output_message(&existing, "GitHub release lookup failed");
    let lookup_lower = lookup_message.to_ascii_lowercase();
    if !lookup_lower.contains("not found") && !lookup_lower.contains("no release found") {
        return Err(format!(
            "Could not prove that release {} is absent: {lookup_message}",
            plan.tag
        ));
    }

    let manifest = plan.manifest_path.to_string_lossy().to_string();
    let mut arguments = vec!["release".into(), "create".into(), plan.tag.clone()];
    arguments.extend(
        plan.assets
            .iter()
            .map(|asset| asset.path.to_string_lossy().to_string()),
    );
    arguments.extend([
        manifest,
        "--repo".into(),
        plan.repository.clone(),
        "--title".into(),
        plan.title.clone(),
        "--notes".into(),
        plan.notes.clone(),
        "--latest".into(),
    ]);
    let output = publisher::run_gh(arguments.iter().map(String::as_str))?;
    if !output.status.success() {
        return Err(publisher::output_message(
            &output,
            "GitHub CLI could not create the release",
        ));
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    release_plans()
        .lock()
        .map_err(|_| "Release preview cache is unavailable".to_string())?
        .remove(preview_id);
    Ok(ReleasePublication {
        profile_id: plan.profile_id,
        version: plan.version,
        manifest_url: format!(
            "https://github.com/{}/releases/latest/download/{}",
            plan.repository, plan.manifest_file_name
        ),
        repository: plan.repository,
        tag: plan.tag,
        url,
        message: format!(
            "GitHub Release created with {} reviewed package asset(s) and the manifest.",
            plan.assets.len()
        ),
    })
}

fn validate_request(profile: &GameProfile, request: &PackageRequest) -> Result<String, String> {
    if request.profile_id != profile.id {
        return Err("The release request does not match the selected modpack profile".into());
    }
    publisher::validate_repository_name(request.repository.trim())?;
    validate_artifact_component(&profile.id, "Profile id")?;
    let version = request.version.trim().trim_start_matches('v');
    if version.is_empty()
        || version.len() > 64
        || !version
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
        || version.starts_with('.')
        || version.ends_with('.')
        || version.contains("..")
    {
        return Err("Version may contain letters, numbers, dots, underscores and hyphens".into());
    }
    Date::parse(request.release_date.trim(), &Iso8601::DATE)
        .map_err(|_| "Release date must use YYYY-MM-DD".to_string())?;
    if request.release_notes.chars().count() > 20_000 {
        return Err("Release notes must be 20,000 characters or fewer".into());
    }
    Ok(version.into())
}

fn validate_artifact_component(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 100
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
    {
        return Err(format!(
            "{label} may contain only letters, numbers, underscores and hyphens"
        ));
    }
    Ok(())
}

struct ScanResult {
    files: Vec<SourceFile>,
    excluded_count: usize,
    issues: Vec<String>,
}

fn scan_source(source: &Path, username: &str, user_profile: &str) -> Result<ScanResult, String> {
    let mut candidates = Vec::new();
    let mut excluded_count = 0;
    let mut issues = Vec::new();
    for entry in WalkDir::new(source)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| {
            if entry.depth() == 0 {
                return true;
            }
            let relative = entry.path().strip_prefix(source).unwrap_or(entry.path());
            let excluded = should_exclude(relative, entry.file_type().is_dir());
            if excluded {
                excluded_count += 1;
            }
            !excluded
        })
    {
        let entry = entry.map_err(|error| format!("Could not scan source folder: {error}"))?;
        if entry.depth() == 0 || entry.file_type().is_dir() {
            continue;
        }
        let raw_relative = entry
            .path()
            .strip_prefix(source)
            .map_err(|_| "A scanned file escaped the source folder".to_string())?;
        let relative = raw_relative.to_string_lossy().replace('\\', "/");
        let relative = match safe_path::normalize_relative(&relative) {
            Ok(relative) => relative,
            Err(error) => {
                issues.push(format!("Unsafe source path {relative}: {error}"));
                continue;
            }
        };
        if entry.file_type().is_symlink() {
            issues.push(format!("Symbolic links are not publishable: {relative}"));
            continue;
        }
        if is_private_filename(&relative) {
            issues.push(format!(
                "Private or credential-shaped file found: {relative}"
            ));
            continue;
        }
        let metadata = entry
            .metadata()
            .map_err(|error| format!("Could not inspect {relative}: {error}"))?;
        if is_text_file(&relative) && metadata.len() <= TEXT_SCAN_LIMIT {
            audit_text(entry.path(), &relative, username, user_profile, &mut issues)?;
        }
        candidates.push((entry.path().to_path_buf(), relative, metadata.len()));
    }

    let mut casefolded = HashSet::new();
    let mut files = Vec::with_capacity(candidates.len());
    for (absolute, relative, size) in candidates {
        if !casefolded.insert(relative.to_ascii_lowercase()) {
            issues.push(format!(
                "Two source files collide on case-insensitive filesystems: {relative}"
            ));
            continue;
        }
        let hash = manifest::sha256(&absolute)?;
        files.push(SourceFile {
            absolute,
            relative,
            size,
            hash,
        });
    }
    files.sort_by(|left, right| left.relative.cmp(&right.relative));
    issues.sort();
    issues.dedup();
    Ok(ScanResult {
        files,
        excluded_count,
        issues,
    })
}

fn should_exclude(relative: &Path, is_directory: bool) -> bool {
    let parts: Vec<_> = relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy().to_ascii_lowercase())
        .collect();
    if parts.iter().any(|part| {
        matches!(
            part.as_str(),
            ".git"
                | ".agents"
                | ".codex"
                | "logs"
                | "log"
                | "screenshots"
                | "crash-reports"
                | "crashreports"
                | "saves"
                | "backups"
                | "cache"
                | "caches"
                | "temp"
                | "tmp"
        )
    }) {
        return true;
    }
    if is_directory {
        return false;
    }
    if parts.last().is_some_and(|name| {
        let (stem, extension) = name.rsplit_once('.').unwrap_or((name, ""));
        matches!(extension, "txt" | "md" | "rtf" | "html" | "pdf")
            && [
                "readme",
                "changelog",
                "change_log",
                "license",
                "licence",
                "credits",
            ]
            .iter()
            .any(|prefix| {
                stem == *prefix
                    || stem.starts_with(&format!("{prefix}_"))
                    || stem.starts_with(&format!("{prefix}-"))
            })
    }) {
        return true;
    }
    matches!(
        parts.last().map(String::as_str),
        Some(
            "usercache.json"
                | "usernamecache.json"
                | "user-prefs.json"
                | ".qmenu_opened.marker"
                | "minecraftinstance.json"
                | "servers.dat"
                | "latest.log"
        )
    )
}

fn is_private_filename(relative: &str) -> bool {
    let name = relative
        .rsplit('/')
        .next()
        .unwrap_or(relative)
        .to_ascii_lowercase();
    matches!(
        name.as_str(),
        ".env"
            | ".env.local"
            | "credentials"
            | "credentials.json"
            | "id_rsa"
            | "id_ed25519"
            | "known_hosts"
            | ".npmrc"
            | ".pypirc"
    ) || name.ends_with(".pem")
        || name.ends_with(".key")
}

fn is_text_file(relative: &str) -> bool {
    Path::new(relative)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "cfg"
                    | "conf"
                    | "ini"
                    | "json"
                    | "json5"
                    | "md"
                    | "properties"
                    | "snbt"
                    | "toml"
                    | "txt"
                    | "xml"
                    | "yaml"
                    | "yml"
            )
        })
        .unwrap_or(false)
}

fn audit_text(
    path: &Path,
    relative: &str,
    username: &str,
    user_profile: &str,
    issues: &mut Vec<String>,
) -> Result<(), String> {
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|mut file| file.read_to_end(&mut bytes))
        .map_err(|error| format!("Could not privacy-scan {relative}: {error}"))?;
    let text = String::from_utf8_lossy(&bytes);
    let lower = text.to_ascii_lowercase();
    let relative_lower = relative.to_ascii_lowercase();
    if !user_profile.trim().is_empty() && lower.contains(&user_profile.to_ascii_lowercase()) {
        issues.push(format!("Local profile path found in {relative}"));
    }
    let username = username.trim();
    if username.len() >= 4 {
        let matcher = Regex::new(&format!(
            r"(?i)(^|[^a-z0-9]){}([^a-z0-9]|$)",
            regex::escape(username)
        ))
        .map_err(|error| format!("Could not build privacy rule: {error}"))?;
        if matcher.is_match(&text) || matcher.is_match(&relative_lower) {
            issues.push(format!("Local username found in {relative}"));
        }
    }
    if email_regex().is_match(&text) {
        issues.push(format!("Email address found in {relative}"));
    }
    if windows_profile_regex().is_match(&text) {
        issues.push(format!("Windows user profile path found in {relative}"));
    }
    if credential_regex().is_match(&text) {
        issues.push(format!("Credential-like content found in {relative}"));
    }
    Ok(())
}

fn email_regex() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| Regex::new(r"(?i)\b[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}\b").unwrap())
}

fn windows_profile_regex() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| Regex::new(r#"(?i)[a-z]:[\\/]users[\\/][^\\/\s"']+"#).unwrap())
}

fn credential_regex() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| {
        Regex::new(
            r#"(?i)(github_pat_[a-z0-9_]{20,}|gh[pousr]_[a-z0-9]{20,}|bearer\s+[a-z0-9._~-]{20,}|(?:token|password|passwd|secret|api[_-]?key)\s*[:=]\s*[^\s"']{8,})"#,
        )
        .unwrap()
    })
}

fn diff(base: &Manifest, files: &[SourceFile]) -> (usize, usize, Vec<String>) {
    let old: HashMap<_, _> = base
        .files
        .iter()
        .map(|entry| (entry.path.to_ascii_lowercase(), entry))
        .collect();
    let current: HashSet<_> = files
        .iter()
        .map(|file| file.relative.to_ascii_lowercase())
        .collect();
    let mut added = 0;
    let mut changed = 0;
    for file in files {
        match old.get(&file.relative.to_ascii_lowercase()) {
            None => added += 1,
            Some(entry)
                if entry.hash.to_ascii_lowercase() != file.hash
                    || u64::try_from(entry.size).ok() != Some(file.size) =>
            {
                changed += 1;
            }
            Some(_) => {}
        }
    }
    let mut removed: Vec<_> = base
        .files
        .iter()
        .filter(|entry| !current.contains(&entry.path.to_ascii_lowercase()))
        .map(|entry| entry.path.clone())
        .collect();
    removed.extend(
        base.obsolete_files
            .iter()
            .filter(|path| !current.contains(&path.to_ascii_lowercase()))
            .cloned(),
    );
    removed.sort_by_key(|path| path.to_ascii_lowercase());
    removed.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    (added, changed, removed)
}

fn file_entry(file: &SourceFile) -> FileEntry {
    let category = file
        .relative
        .split('/')
        .next()
        .unwrap_or("files")
        .to_string();
    FileEntry {
        path: file.relative.clone(),
        size: i64::try_from(file.size).unwrap_or(i64::MAX),
        hash: file.hash.clone(),
        download_url: String::new(),
        required: true,
        category,
    }
}

fn preview_id(profile: &GameProfile, request: &PackageRequest, files: &[SourceFile]) -> String {
    let mut digest = Sha256::new();
    digest.update(profile.id.as_bytes());
    digest.update(request.version.trim().as_bytes());
    digest.update(request.release_date.trim().as_bytes());
    digest.update(request.repository.trim().as_bytes());
    for file in files {
        digest.update(file.relative.as_bytes());
        digest.update(file.size.to_le_bytes());
        digest.update(file.hash.as_bytes());
    }
    format!("{:x}", digest.finalize())[..16].to_string()
}

fn create_zip(path: &Path, files: &[SourceFile]) -> Result<(), String> {
    let output = BufWriter::new(
        File::create(path)
            .map_err(|error| format!("Could not create package {}: {error}", path.display()))?,
    );
    let mut archive = ZipWriter::new(output);
    for source in files {
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .compression_level(Some(6))
            .last_modified_time(DateTime::default())
            .unix_permissions(0o644)
            .large_file(source.size > u64::from(u32::MAX));
        archive
            .start_file(&source.relative, options)
            .map_err(|error| format!("Could not add {} to package: {error}", source.relative))?;
        let mut input = BufReader::new(File::open(&source.absolute).map_err(|error| {
            format!(
                "Could not reopen {} during packaging: {error}",
                source.relative
            )
        })?);
        io::copy(&mut input, &mut archive)
            .map_err(|error| format!("Could not package {}: {error}", source.relative))?;
    }
    let mut output = archive
        .finish()
        .map_err(|error| format!("Could not finalize package: {error}"))?;
    output
        .flush()
        .map_err(|error| format!("Could not flush package to disk: {error}"))?;
    Ok(())
}

fn prepare_release_assets(
    package_path: &Path,
    output_dir: &Path,
    package_name: &str,
    multipart: bool,
    part_size: u64,
) -> Result<Vec<ReleaseAsset>, String> {
    if !multipart {
        let bytes = package_path
            .metadata()
            .map_err(|error| format!("Could not inspect release package: {error}"))?
            .len();
        return Ok(vec![ReleaseAsset {
            file_name: package_name.into(),
            path: package_path.to_path_buf(),
            bytes,
            sha256: manifest::sha256(package_path)?,
        }]);
    }
    split_package(package_path, output_dir, package_name, part_size)
}

fn split_package(
    package_path: &Path,
    output_dir: &Path,
    package_name: &str,
    part_size: u64,
) -> Result<Vec<ReleaseAsset>, String> {
    if part_size == 0 {
        return Err("Multipart release part size must be greater than zero".into());
    }
    let package_bytes = package_path
        .metadata()
        .map_err(|error| format!("Could not inspect package before splitting: {error}"))?
        .len();
    let part_count = package_bytes.div_ceil(part_size);
    if part_count == 0
        || usize::try_from(part_count)
            .ok()
            .is_none_or(|count| count > MAX_RELEASE_PARTS)
    {
        return Err(format!(
            "The package requires {part_count} release parts; the safe limit is {MAX_RELEASE_PARTS}"
        ));
    }

    let mut created = Vec::new();
    let result = (|| {
        let mut input = BufReader::new(
            File::open(package_path)
                .map_err(|error| format!("Could not reopen package for splitting: {error}"))?,
        );
        let mut assets = Vec::with_capacity(usize::try_from(part_count).unwrap_or(0));
        let mut buffer = vec![0_u8; 1024 * 1024];
        for index in 0..part_count {
            let file_name = format!("{package_name}.part{:03}", index + 1);
            let path = output_dir.join(&file_name);
            let mut output = BufWriter::new(File::create(&path).map_err(|error| {
                format!("Could not create release part {}: {error}", index + 1)
            })?);
            created.push(path.clone());
            let mut digest = Sha256::new();
            let mut written = 0_u64;
            while written < part_size {
                let remaining = part_size - written;
                let capacity =
                    usize::try_from(remaining.min(buffer.len() as u64)).unwrap_or(buffer.len());
                let count = input.read(&mut buffer[..capacity]).map_err(|error| {
                    format!("Could not read package part {}: {error}", index + 1)
                })?;
                if count == 0 {
                    break;
                }
                output.write_all(&buffer[..count]).map_err(|error| {
                    format!("Could not write package part {}: {error}", index + 1)
                })?;
                digest.update(&buffer[..count]);
                written += u64::try_from(count).unwrap_or(0);
            }
            output
                .flush()
                .map_err(|error| format!("Could not flush package part {}: {error}", index + 1))?;
            if written == 0 {
                return Err(format!("Package part {} would be empty", index + 1));
            }
            assets.push(ReleaseAsset {
                file_name,
                path,
                bytes: written,
                sha256: format!("{:x}", digest.finalize()),
            });
        }
        let total = assets
            .iter()
            .try_fold(0_u64, |sum, asset| sum.checked_add(asset.bytes));
        if total != Some(package_bytes) {
            return Err(
                "Multipart release parts do not reconstruct the complete package size".into(),
            );
        }
        fs::remove_file(package_path)
            .map_err(|error| format!("Could not remove the split source package: {error}"))?;
        Ok(assets)
    })();
    if result.is_err() {
        for path in created {
            fs::remove_file(path).ok();
        }
    }
    result
}

fn asset_preview(asset: &ReleaseAsset) -> PackageAssetPreview {
    PackageAssetPreview {
        file_name: asset.file_name.clone(),
        path: asset.path.display().to_string(),
        bytes: asset.bytes,
        sha256: asset.sha256.clone(),
    }
}

fn remove_assets(assets: &[ReleaseAsset]) {
    for asset in assets {
        fs::remove_file(&asset.path).ok();
    }
}

fn validate_zip(path: &Path, expected: &[SourceFile]) -> Result<(), String> {
    let file = File::open(path).map_err(|error| format!("Could not reopen package: {error}"))?;
    let mut archive =
        ZipArchive::new(file).map_err(|error| format!("Generated package is invalid: {error}"))?;
    if archive.len() != expected.len() {
        return Err("Generated package file count does not match the source inventory".into());
    }
    for expected_file in expected {
        let mut member = archive
            .by_name(&expected_file.relative)
            .map_err(|error| format!("Package is missing {}: {error}", expected_file.relative))?;
        safe_path::validate_archive_member(member.name(), member.is_dir())?;
        if member.size() != expected_file.size {
            return Err(format!(
                "Packaged size for {} does not match the source",
                expected_file.relative
            ));
        }
        let mut digest = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let count = member.read(&mut buffer).map_err(|error| {
                format!(
                    "Could not verify packaged content for {}: {error}",
                    expected_file.relative
                )
            })?;
            if count == 0 {
                break;
            }
            digest.update(&buffer[..count]);
        }
        if format!("{:x}", digest.finalize()) != expected_file.hash {
            return Err(format!(
                "Packaged content for {} changed during preparation",
                expected_file.relative
            ));
        }
    }
    Ok(())
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("Could not serialize generated manifest: {error}"))?;
    fs::write(path, bytes).map_err(|error| format!("Could not write {}: {error}", path.display()))
}

fn release_plans() -> &'static Mutex<HashMap<String, ReleasePlan>> {
    RELEASE_PLANS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn validate_plan(plan: &ReleasePlan) -> Result<(), String> {
    if plan.assets.is_empty() {
        return Err("The reviewed release contains no package assets".into());
    }
    for path in plan
        .assets
        .iter()
        .map(|asset| &asset.path)
        .chain(std::iter::once(&plan.manifest_path))
    {
        let canonical = fs::canonicalize(path)
            .map_err(|error| format!("A reviewed release asset is unavailable: {error}"))?;
        if !canonical.starts_with(&plan.output_dir) || !canonical.is_file() {
            return Err("A reviewed release asset escaped its native preview folder".into());
        }
    }
    if plan.assets.iter().any(|asset| {
        asset.path.metadata().map(|value| value.len()).ok() != Some(asset.bytes)
            || manifest::sha256(&asset.path).ok().as_deref() != Some(asset.sha256.as_str())
    }) || manifest::sha256(&plan.manifest_path)? != plan.manifest_sha256
    {
        return Err("A reviewed release asset changed after preview; prepare it again".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn profile() -> GameProfile {
        GameProfile {
            id: "fixture".into(),
            game: "minecraft".into(),
            display_name: "Fixture Pack".into(),
            required_game_version: "1.21.1".into(),
            required_modpack_version: "1.0.0".into(),
            local_modpack_version: String::new(),
            manifest_path: "manifests/fixture.json".into(),
            install_dir: String::new(),
            game_dir: String::new(),
            game_exe_path: String::new(),
            launch_args: String::new(),
            minecraft_launcher: String::new(),
            discord_invite: String::new(),
            update_source: String::new(),
            manifest_url: String::new(),
            deployment_subdir: String::new(),
            logo_path: String::new(),
            catalog_visible: true,
        }
    }

    fn request(source: &Path) -> PackageRequest {
        PackageRequest {
            profile_id: "fixture".into(),
            source_dir: source.display().to_string(),
            version: "2.0.0".into(),
            release_date: "2026-08-24".into(),
            repository: "owner/repository".into(),
            release_notes: "Verified fixture release".into(),
        }
    }

    #[test]
    fn builds_reproducible_package_and_safe_manifest() {
        let source = TempDir::new().unwrap();
        fs::create_dir_all(source.path().join("mods")).unwrap();
        fs::create_dir_all(source.path().join("config")).unwrap();
        fs::create_dir_all(source.path().join("logs")).unwrap();
        fs::write(source.path().join("mods/example.jar"), b"example mod").unwrap();
        fs::write(source.path().join("config/settings.toml"), b"enabled=true").unwrap();
        fs::write(source.path().join("user-prefs.json"), b"private").unwrap();
        fs::write(source.path().join("logs/latest.log"), b"private").unwrap();
        let base = Manifest {
            manifest_version: "1.0".into(),
            profile_id: "fixture".into(),
            game: "minecraft".into(),
            modpack_version: "1.0.0".into(),
            files: vec![FileEntry {
                path: "mods/removed.jar".into(),
                size: 3,
                hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
                required: true,
                ..FileEntry::default()
            }],
            obsolete_files: vec!["mods/already-obsolete.jar".into()],
            ..Manifest::default()
        };
        let output_one = TempDir::new().unwrap();
        let output_two = TempDir::new().unwrap();
        let first = prepare_at(
            &profile(),
            &base,
            &request(source.path()),
            output_one.path(),
            "FixtureUser",
            r"C:\Users\FixtureUser",
            false,
        )
        .unwrap();
        let second = prepare_at(
            &profile(),
            &base,
            &request(source.path()),
            output_two.path(),
            "FixtureUser",
            r"C:\Users\FixtureUser",
            false,
        )
        .unwrap();
        assert!(first.ready, "{:?}", first.issues);
        assert_eq!(first.package_sha256, second.package_sha256);
        assert_eq!(first.file_count, 2);
        assert_eq!(first.added, 2);
        assert_eq!(first.removed, 2);
        assert!(first.excluded_count >= 2);

        let manifest: Manifest =
            serde_json::from_slice(&fs::read(&first.manifest_path).unwrap()).unwrap();
        assert!(manifest::validate(&manifest, Some(&profile())).is_empty());
        assert_eq!(manifest.update_sha256, first.package_sha256);
        assert_eq!(
            manifest.obsolete_files,
            ["mods/already-obsolete.jar", "mods/removed.jar"]
        );
        assert!(manifest.update_url.ends_with("/v2.0.0/fixture_2.0.0.zip"));

        let mut zip = ZipArchive::new(File::open(&first.package_path).unwrap()).unwrap();
        assert!(zip.by_name("mods/example.jar").is_ok());
        assert!(zip.by_name("config/settings.toml").is_ok());
        assert!(zip.by_name("user-prefs.json").is_err());
        assert!(zip.by_name("logs/latest.log").is_err());
    }

    #[test]
    fn builds_ordered_multipart_assets_that_reconstruct_the_verified_zip() {
        let source = TempDir::new().unwrap();
        fs::create_dir_all(source.path().join("Mods/Example")).unwrap();
        let content: Vec<u8> = (0_u8..=255).cycle().take(4096).collect();
        fs::write(source.path().join("Mods/Example/example.dll"), &content).unwrap();
        let output = TempDir::new().unwrap();
        let preview = prepare_at_with_limits(
            &profile(),
            &Manifest {
                manifest_version: "1.0".into(),
                profile_id: "fixture".into(),
                game: "minecraft".into(),
                modpack_version: "1.0.0".into(),
                ..Manifest::default()
            },
            &request(source.path()),
            output.path(),
            "FixtureUser",
            r"C:\Users\FixtureUser",
            false,
            1,
            64,
        )
        .unwrap();

        assert!(preview.ready, "{:?}", preview.issues);
        assert!(preview.multipart);
        assert!(preview.assets.len() > 1);
        assert!(preview.package_path.is_empty());
        assert!(preview.assets.iter().all(|asset| asset.bytes <= 64));
        assert_eq!(
            preview.assets.iter().map(|asset| asset.bytes).sum::<u64>(),
            preview.package_bytes
        );

        let generated: Manifest =
            serde_json::from_slice(&fs::read(&preview.manifest_path).unwrap()).unwrap();
        assert!(generated.update_url.is_empty());
        assert_eq!(generated.update_sha256, preview.package_sha256);
        assert_eq!(generated.update_parts.len(), preview.assets.len());
        for (part, asset) in generated.update_parts.iter().zip(&preview.assets) {
            assert!(part.url.ends_with(&asset.file_name));
            assert_eq!(part.sha256, asset.sha256);
            assert_eq!(u64::try_from(part.size).unwrap(), asset.bytes);
        }

        let assembled = output.path().join("assembled.zip");
        let mut combined = File::create(&assembled).unwrap();
        for asset in &preview.assets {
            io::copy(&mut File::open(&asset.path).unwrap(), &mut combined).unwrap();
        }
        drop(combined);
        assert_eq!(
            manifest::sha256(&assembled).unwrap(),
            preview.package_sha256
        );
        let mut zip = ZipArchive::new(File::open(assembled).unwrap()).unwrap();
        let mut packaged = Vec::new();
        zip.by_name("Mods/Example/example.dll")
            .unwrap()
            .read_to_end(&mut packaged)
            .unwrap();
        assert_eq!(packaged, content);
    }

    #[test]
    fn privacy_scan_reaches_large_text_and_ignores_incidental_root_path() {
        let source = TempDir::new().unwrap();
        let mut content = vec![b'x'; 4 * 1024 * 1024];
        content.extend_from_slice(b"\ncontact=person@example.com\n");
        fs::write(source.path().join("large.properties"), content).unwrap();
        let scan = scan_source(source.path(), "FixtureUser", r"C:\Users\FixtureUser").unwrap();
        assert!(
            scan.issues
                .iter()
                .any(|issue| issue.contains("Email address"))
        );
        assert!(
            !scan
                .issues
                .iter()
                .any(|issue| issue.contains("Local username"))
        );
    }

    #[test]
    fn privacy_scan_rejects_private_files_and_credentials() {
        let source = TempDir::new().unwrap();
        fs::write(source.path().join(".env"), b"SAFE=false").unwrap();
        fs::write(
            source.path().join("settings.toml"),
            b"api_key=super-secret-value",
        )
        .unwrap();
        let scan = scan_source(source.path(), "FixtureUser", "").unwrap();
        assert!(scan.issues.iter().any(|issue| issue.contains("Private")));
        assert!(
            scan.issues
                .iter()
                .any(|issue| issue.contains("Credential-like"))
        );
    }

    #[test]
    fn excludes_upstream_documentation_without_hiding_runtime_files() {
        let source = TempDir::new().unwrap();
        fs::create_dir_all(source.path().join("ExampleMod")).unwrap();
        fs::write(source.path().join("ExampleMod/runtime.dll"), b"runtime").unwrap();
        fs::write(
            source.path().join("ExampleMod/CHANGELOG_v3.0.txt"),
            b"contact=upstream@example.com",
        )
        .unwrap();
        fs::write(
            source.path().join("ExampleMod/README.txt"),
            br"example=C:\Users\Example\Mods",
        )
        .unwrap();

        let scan = scan_source(source.path(), "FixtureUser", "").unwrap();
        assert!(scan.issues.is_empty(), "{:?}", scan.issues);
        assert_eq!(scan.files.len(), 1);
        assert_eq!(scan.files[0].relative, "ExampleMod/runtime.dll");
        assert_eq!(scan.excluded_count, 2);
    }

    #[test]
    fn release_publication_is_fail_closed_before_github_is_called() {
        assert!(
            publish("not-a-preview", false)
                .unwrap_err()
                .contains("explicit confirmation")
        );
    }

    #[test]
    fn cached_release_plan_rehashes_every_reviewed_part() {
        let output = TempDir::new().unwrap();
        let output_dir = fs::canonicalize(output.path()).unwrap();
        let part = output_dir.join("fixture.zip.part001");
        let manifest_path = output_dir.join("fixture-manifest.json");
        fs::write(&part, b"reviewed part").unwrap();
        fs::write(&manifest_path, b"reviewed manifest").unwrap();
        let plan = ReleasePlan {
            profile_id: "fixture".into(),
            version: "1.0.0".into(),
            repository: "owner/repository".into(),
            tag: "v1.0.0".into(),
            title: "Fixture".into(),
            notes: "Fixture".into(),
            output_dir,
            assets: vec![ReleaseAsset {
                file_name: "fixture.zip.part001".into(),
                path: part.clone(),
                bytes: part.metadata().unwrap().len(),
                sha256: manifest::sha256(&part).unwrap(),
            }],
            manifest_path: manifest_path.clone(),
            manifest_file_name: "fixture-manifest.json".into(),
            manifest_sha256: manifest::sha256(&manifest_path).unwrap(),
        };
        assert!(validate_plan(&plan).is_ok());
        fs::write(&part, b"changed after preview").unwrap();
        assert!(
            validate_plan(&plan)
                .unwrap_err()
                .contains("changed after preview")
        );
    }

    #[test]
    #[ignore = "requires MYTHIC_LOOT_7DTD_SOURCE and several GiB of temporary disk space"]
    fn live_7dtd_source_builds_a_verified_multipart_release() {
        let source = PathBuf::from(
            env::var("MYTHIC_LOOT_7DTD_SOURCE")
                .expect("Set MYTHIC_LOOT_7DTD_SOURCE to the live read-only Mods folder"),
        );
        let mut profile = profile();
        profile.id = "seven_days_main".into();
        profile.game = "seven_days".into();
        profile.display_name = "Mythic Loot 7 Days".into();
        profile.required_game_version = "1.0".into();
        let base: Manifest =
            serde_json::from_str(include_str!("../resources/manifests/seven_days_main.json"))
                .unwrap();
        let request = PackageRequest {
            profile_id: profile.id.clone(),
            source_dir: source.display().to_string(),
            version: "1.0.1-acceptance".into(),
            release_date: "2026-08-28".into(),
            repository: "HixxyDubz/Mythic-Loot-7DTD-Modpack".into(),
            release_notes: "Local acceptance only; never published".into(),
        };
        let output = TempDir::new().unwrap();
        let username = env::var("USERNAME").unwrap_or_default();
        let user_profile = env::var("USERPROFILE").unwrap_or_default();

        let preview = prepare_at(
            &profile,
            &base,
            &request,
            output.path(),
            &username,
            &user_profile,
            false,
        )
        .unwrap();
        assert!(preview.ready, "{:?}", preview.issues);
        assert!(preview.multipart);
        assert!(preview.assets.len() > 1);
        assert_eq!(
            preview.assets.iter().map(|asset| asset.bytes).sum::<u64>(),
            preview.package_bytes
        );
        let manifest: Manifest =
            serde_json::from_slice(&fs::read(&preview.manifest_path).unwrap()).unwrap();
        assert!(manifest::validate(&manifest, Some(&profile)).is_empty());
        assert_eq!(manifest.update_parts.len(), preview.assets.len());
    }
}
