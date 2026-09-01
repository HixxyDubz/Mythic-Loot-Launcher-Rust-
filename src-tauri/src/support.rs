use std::{
    collections::{HashMap, HashSet},
    env,
    fs::{self, File, Metadata},
    io::{Read, Seek, SeekFrom, Write},
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

use regex::{Regex, RegexBuilder};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::AppHandle;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use zip::{CompressionMethod, DateTime, ZipWriter, write::SimpleFileOptions};

use crate::{
    models::{GameProfile, ProfileHealth},
    readiness, storage,
};

const SUPPORT_DIRECTORY: &str = "support-bundles";
const MAX_LOG_SOURCE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_LOG_EXCERPT_BYTES: usize = 512 * 1024;
const MAX_LOG_LINES: usize = 500;
const MAX_LOG_CANDIDATES_PER_DIRECTORY: usize = 10_000;
const MAX_CACHED_PREVIEWS: usize = 8;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportPreview {
    pub preview_id: String,
    pub profile_id: String,
    pub display_name: String,
    pub latest_log_path: String,
    pub latest_log_name: String,
    pub source_bytes: u64,
    pub included_bytes: u64,
    pub truncated: bool,
    pub summary: String,
    pub redacted_log: String,
    pub files: Vec<String>,
    pub issues: Vec<String>,
    pub ready: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportBundleOutcome {
    pub profile_id: String,
    pub path: String,
    pub directory: String,
    pub file_name: String,
    pub bytes: u64,
    pub sha256: String,
    pub files: Vec<String>,
    pub message: String,
}

#[derive(Clone)]
struct SupportPlan {
    preview: SupportPreview,
    output_file_name: String,
    summary_json: Vec<u8>,
    summary_text: Vec<u8>,
    log_entry: Option<(String, Vec<u8>)>,
}

struct LogExcerpt {
    path: PathBuf,
    file_name: String,
    source_bytes: u64,
    text: String,
    truncated: bool,
}

struct Redactor {
    homes: Vec<Regex>,
    username: Option<Regex>,
}

static SUPPORT_PLANS: OnceLock<Mutex<HashMap<String, SupportPlan>>> = OnceLock::new();

pub fn prepare(app: &AppHandle, profile_id: &str) -> Result<SupportPreview, String> {
    let config = storage::load_or_create(app)?;
    let profile = config
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .cloned()
        .ok_or_else(|| "That modpack profile does not exist".to_string())?;
    let loaded = crate::manifest::load_for_profile(app, &profile);
    let health = readiness::assess(&profile, Some(&loaded.summary));
    let redactor = Redactor::from_environment()?;
    let latest = find_latest_log(&profile).map_err(|error| redactor.redact(&error))?;
    let plan = build_plan(
        &profile,
        &health,
        latest.as_deref(),
        &redactor,
        SystemTime::now(),
    )?;
    let preview = plan.preview.clone();
    let mut plans = support_plans()
        .lock()
        .map_err(|_| "Support preview cache is unavailable".to_string())?;
    if plans.len() >= MAX_CACHED_PREVIEWS {
        plans.clear();
    }
    plans.insert(preview.preview_id.clone(), plan);
    Ok(preview)
}

pub fn create(
    app: &AppHandle,
    preview_id: &str,
    confirmed: bool,
) -> Result<SupportBundleOutcome, String> {
    require_confirmation(confirmed)?;
    let plan = support_plans()
        .lock()
        .map_err(|_| "Support preview cache is unavailable".to_string())?
        .get(preview_id)
        .cloned()
        .ok_or_else(|| "Support preview expired; review it again before exporting".to_string())?;
    if !plan.preview.ready {
        return Err("The reviewed support bundle is not ready to export".into());
    }
    let data_dir = storage::data_dir(app)?;
    let output_dir = data_dir.join(SUPPORT_DIRECTORY);
    let outcome = write_bundle_at(&output_dir, &plan)?;
    support_plans()
        .lock()
        .map_err(|_| "Support preview cache is unavailable".to_string())?
        .remove(preview_id);
    Ok(outcome)
}

fn build_plan(
    profile: &GameProfile,
    health: &ProfileHealth,
    log_path: Option<&Path>,
    redactor: &Redactor,
    now: SystemTime,
) -> Result<SupportPlan, String> {
    let mut issues = Vec::new();
    let excerpt = match log_path {
        Some(path) => match read_log_excerpt(path, redactor) {
            Ok(excerpt) => Some(excerpt),
            Err(error) => {
                issues.push(error);
                None
            }
        },
        None => None,
    };
    let generated_at = OffsetDateTime::from(now)
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown".into());
    let latest_log_display = excerpt
        .as_ref()
        .map(|value| redactor.redact(&value.path.display().to_string()))
        .unwrap_or_else(|| "not found".into());
    let status = serde_json::to_value(health.status)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".into());
    let summary = build_summary(
        profile,
        health,
        &status,
        &generated_at,
        &latest_log_display,
        excerpt.as_ref(),
        redactor,
    );
    let summary_value = serde_json::json!({
        "launcher": "Mythic Loot Launcher",
        "launcherVersion": env!("CARGO_PKG_VERSION"),
        "generatedAt": generated_at,
        "platform": {
            "os": env::consts::OS,
            "architecture": env::consts::ARCH,
        },
        "profile": {
            "id": profile.id,
            "displayName": redactor.redact(&profile.display_name),
            "game": profile.game,
            "requiredGameVersion": profile.required_game_version,
            "requiredModpackVersion": profile.required_modpack_version,
            "installedModpackVersion": profile.local_modpack_version,
            "minecraftLauncher": profile.minecraft_launcher,
        },
        "readiness": {
            "status": status,
            "headline": redactor.redact(&health.headline),
            "details": health.details.iter().map(|detail| redactor.redact(detail)).collect::<Vec<_>>(),
        },
        "latestLog": {
            "path": latest_log_display,
            "sourceBytes": excerpt.as_ref().map_or(0, |value| value.source_bytes),
            "includedBytes": excerpt.as_ref().map_or(0, |value| value.text.len()),
            "truncated": excerpt.as_ref().is_some_and(|value| value.truncated),
        },
        "privacy": {
            "pathsAndUsernamesRedacted": true,
            "obviousSecretsRedacted": true,
            "networkAddressesAndEmailsRedacted": true,
            "serverConfigurationIncluded": false,
        },
    });
    let mut summary_json = serde_json::to_vec_pretty(&summary_value)
        .map_err(|error| format!("Could not prepare support summary JSON: {error}"))?;
    summary_json.push(b'\n');
    let summary_text = format!("{}\n", redactor.redact(&summary)).into_bytes();
    let log_entry = excerpt.as_ref().map(|value| {
        (
            format!("logs/{}.redacted.txt", safe_file_name(&value.file_name)),
            format!("{}\n", value.text).into_bytes(),
        )
    });
    let mut files = vec!["summary.json".into(), "summary.txt".into()];
    if let Some((name, _)) = &log_entry {
        files.push(name.clone());
    }
    let digest = Sha256::digest(
        [
            summary_json.as_slice(),
            summary_text.as_slice(),
            log_entry
                .as_ref()
                .map(|(_, bytes)| bytes.as_slice())
                .unwrap_or_default(),
        ]
        .concat(),
    );
    let stamp = unix_millis(now);
    let preview_id = format!("{:x}", digest)[..16].to_string();
    let output_file_name = format!("support-{}-{stamp}-{preview_id}.zip", profile.id);
    let preview = SupportPreview {
        preview_id,
        profile_id: profile.id.clone(),
        display_name: profile.display_name.clone(),
        latest_log_path: excerpt
            .as_ref()
            .map(|value| value.path.display().to_string())
            .unwrap_or_default(),
        latest_log_name: excerpt
            .as_ref()
            .map(|value| value.file_name.clone())
            .unwrap_or_default(),
        source_bytes: excerpt.as_ref().map_or(0, |value| value.source_bytes),
        included_bytes: excerpt.as_ref().map_or(0, |value| {
            u64::try_from(value.text.len()).unwrap_or(u64::MAX)
        }),
        truncated: excerpt.as_ref().is_some_and(|value| value.truncated),
        summary: String::from_utf8(summary_text.clone()).unwrap_or_default(),
        redacted_log: excerpt
            .as_ref()
            .map(|value| value.text.clone())
            .unwrap_or_default(),
        files,
        issues,
        ready: true,
        message: if excerpt.is_some() {
            "A privacy-redacted support bundle is ready for review. No file has been written yet."
                .into()
        } else {
            "No supported game log was found. A redacted launcher summary is still ready for review."
                .into()
        },
    };
    Ok(SupportPlan {
        preview,
        output_file_name,
        summary_json,
        summary_text,
        log_entry,
    })
}

fn build_summary(
    profile: &GameProfile,
    health: &ProfileHealth,
    status: &str,
    generated_at: &str,
    latest_log: &str,
    excerpt: Option<&LogExcerpt>,
    redactor: &Redactor,
) -> String {
    let mut lines = vec![
        "Launcher: Mythic Loot Launcher".to_string(),
        format!("Launcher version: {}", env!("CARGO_PKG_VERSION")),
        format!("Generated: {generated_at}"),
        format!("Profile: {} ({})", profile.display_name, profile.id),
        format!("Game: {}", profile.game),
        format!(
            "Required game version: {}",
            value_or_unknown(&profile.required_game_version)
        ),
        format!(
            "Required modpack version: {}",
            value_or_unknown(&profile.required_modpack_version)
        ),
        format!(
            "Installed modpack version: {}",
            value_or_unknown(&profile.local_modpack_version)
        ),
        format!("Readiness: {status} - {}", health.headline),
    ];
    for detail in &health.details {
        lines.push(format!("Readiness detail: {detail}"));
    }
    lines.push(format!("Latest log: {latest_log}"));
    if let Some(excerpt) = excerpt {
        lines.push(format!("Log source bytes: {}", excerpt.source_bytes));
        lines.push(format!("Redacted excerpt bytes: {}", excerpt.text.len()));
        lines.push(format!("Log excerpt truncated: {}", excerpt.truncated));
    }
    lines.push("Server configuration included: no".into());
    redactor.redact(&lines.join("\n"))
}

fn find_latest_log(profile: &GameProfile) -> Result<Option<PathBuf>, String> {
    let mut directories = Vec::new();
    let install = nonempty_path(&profile.install_dir);
    let game = nonempty_path(&profile.game_dir);
    match profile.game.as_str() {
        "minecraft" => {
            if let Some(install) = install {
                directories.push((install.join("logs"), LogPattern::MinecraftLog));
                directories.push((install.join("crash-reports"), LogPattern::MinecraftCrash));
            }
        }
        "seven_days" | "7daystodie" | "7_days_to_die" => {
            for variable in ["APPDATA", "LOCALAPPDATA"] {
                if let Some(root) = env::var_os(variable).filter(|value| !value.is_empty()) {
                    let root = PathBuf::from(root).join("7DaysToDie");
                    directories.push((root.clone(), LogPattern::SevenDays));
                    directories.push((root.join("logs"), LogPattern::SevenDays));
                }
            }
            for root in [install, game].into_iter().flatten() {
                directories.push((root.clone(), LogPattern::SevenDays));
                directories.push((root.join("logs"), LogPattern::SevenDays));
            }
        }
        _ => {
            for root in [install, game].into_iter().flatten() {
                directories.push((root.join("logs"), LogPattern::Generic));
                directories.push((root.join("Logs"), LogPattern::Generic));
            }
        }
    }
    find_latest_in_directories(&directories)
}

#[derive(Clone, Copy)]
enum LogPattern {
    MinecraftLog,
    MinecraftCrash,
    SevenDays,
    Generic,
}

fn find_latest_in_directories(
    directories: &[(PathBuf, LogPattern)],
) -> Result<Option<PathBuf>, String> {
    let mut seen = HashSet::new();
    let mut candidates = Vec::new();
    for (directory, pattern) in directories {
        let key = directory.to_string_lossy().to_ascii_lowercase();
        if !seen.insert(key) || !directory.exists() {
            continue;
        }
        let metadata = match fs::symlink_metadata(directory) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        if is_link_like(&metadata) || !metadata.is_dir() {
            continue;
        }
        for entry in fs::read_dir(directory)
            .map_err(|error| {
                format!(
                    "Could not inspect log folder {}: {error}",
                    directory.display()
                )
            })?
            .take(MAX_LOG_CANDIDATES_PER_DIRECTORY)
        {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            let file_name = entry.file_name().to_string_lossy().to_string();
            if !matches_log_name(&file_name, *pattern) {
                continue;
            }
            let metadata = match fs::symlink_metadata(entry.path()) {
                Ok(metadata) => metadata,
                Err(_) => continue,
            };
            if is_link_like(&metadata)
                || !metadata.is_file()
                || metadata.len() > MAX_LOG_SOURCE_BYTES
            {
                continue;
            }
            candidates.push((metadata.modified().unwrap_or(UNIX_EPOCH), entry.path()));
        }
    }
    candidates.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1)));
    Ok(candidates.into_iter().next().map(|(_, path)| path))
}

fn matches_log_name(file_name: &str, pattern: LogPattern) -> bool {
    let lower = file_name.to_ascii_lowercase();
    match pattern {
        LogPattern::MinecraftLog => lower == "latest.log",
        LogPattern::MinecraftCrash => lower.ends_with(".txt"),
        LogPattern::SevenDays => {
            lower.ends_with(".log")
                || lower.starts_with("output_log") && lower.ends_with(".txt")
                || lower == "player.log"
        }
        LogPattern::Generic => lower.ends_with(".log") || lower == "player.log",
    }
}

fn read_log_excerpt(path: &Path, redactor: &Redactor) -> Result<LogExcerpt, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("Could not inspect the latest log: {error}"))?;
    if is_link_like(&metadata) || !metadata.is_file() {
        return Err("The latest log is linked, redirected or no longer a regular file".into());
    }
    if metadata.len() > MAX_LOG_SOURCE_BYTES {
        return Err(format!(
            "The latest log is larger than the {} MiB safety limit",
            MAX_LOG_SOURCE_BYTES / 1024 / 1024
        ));
    }
    let read_bytes = metadata
        .len()
        .min(u64::try_from(MAX_LOG_EXCERPT_BYTES).unwrap_or(u64::MAX));
    let mut file =
        File::open(path).map_err(|error| format!("Could not open the latest log: {error}"))?;
    file.seek(SeekFrom::End(
        -i64::try_from(read_bytes).unwrap_or(i64::MAX),
    ))
    .map_err(|error| format!("Could not seek within the latest log: {error}"))?;
    let mut bytes = Vec::with_capacity(usize::try_from(read_bytes).unwrap_or_default());
    file.take(read_bytes)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Could not read the latest log: {error}"))?;
    if !bytes.is_empty() && bytes.iter().filter(|byte| **byte == 0).count() > bytes.len() / 100 {
        return Err("The latest log appears to be binary and was not included".into());
    }
    let decoded = String::from_utf8_lossy(&bytes);
    let all_lines: Vec<_> = decoded.lines().collect();
    let first = all_lines.len().saturating_sub(MAX_LOG_LINES);
    let text = redactor.redact(&all_lines[first..].join("\n"));
    Ok(LogExcerpt {
        path: path.to_path_buf(),
        file_name: path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("latest-log")
            .to_string(),
        source_bytes: metadata.len(),
        text,
        truncated: metadata.len() > read_bytes || first > 0,
    })
}

fn write_bundle_at(output_dir: &Path, plan: &SupportPlan) -> Result<SupportBundleOutcome, String> {
    fs::create_dir_all(output_dir)
        .map_err(|error| format!("Could not create the support bundle folder: {error}"))?;
    reject_link_root(output_dir)?;
    let destination = output_dir.join(&plan.output_file_name);
    let partial = output_dir.join(format!(".{}.partial", plan.output_file_name));
    if destination.exists() {
        return Err("That reviewed support bundle already exists".into());
    }
    if partial.exists() {
        let metadata = fs::symlink_metadata(&partial)
            .map_err(|error| format!("Could not inspect prior support staging: {error}"))?;
        if is_link_like(&metadata) || !metadata.is_file() {
            return Err("Prior support staging is not a regular launcher-owned file".into());
        }
        fs::remove_file(&partial)
            .map_err(|error| format!("Could not clear prior support staging: {error}"))?;
    }
    let result = (|| {
        let output = File::create(&partial)
            .map_err(|error| format!("Could not create support bundle staging: {error}"))?;
        let mut archive = ZipWriter::new(output);
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .compression_level(Some(6))
            .last_modified_time(DateTime::default())
            .unix_permissions(0o644);
        write_zip_entry(&mut archive, "summary.json", &plan.summary_json, options)?;
        write_zip_entry(&mut archive, "summary.txt", &plan.summary_text, options)?;
        if let Some((name, bytes)) = &plan.log_entry {
            write_zip_entry(&mut archive, name, bytes, options)?;
        }
        let output = archive
            .finish()
            .map_err(|error| format!("Could not finish the support bundle: {error}"))?;
        output
            .sync_all()
            .map_err(|error| format!("Could not flush the support bundle: {error}"))?;
        fs::rename(&partial, &destination)
            .map_err(|error| format!("Could not activate the support bundle: {error}"))?;
        Ok::<(), String>(())
    })();
    if result.is_err() {
        fs::remove_file(&partial).ok();
    }
    result?;
    let bytes = destination
        .metadata()
        .map_err(|error| format!("Could not inspect the support bundle: {error}"))?
        .len();
    let sha256 = crate::manifest::sha256(&destination)?;
    Ok(SupportBundleOutcome {
        profile_id: plan.preview.profile_id.clone(),
        path: destination.display().to_string(),
        directory: output_dir.display().to_string(),
        file_name: plan.output_file_name.clone(),
        bytes,
        sha256,
        files: plan.preview.files.clone(),
        message: format!(
            "Privacy-redacted support bundle created with {} reviewed file(s).",
            plan.preview.files.len()
        ),
    })
}

fn write_zip_entry(
    archive: &mut ZipWriter<File>,
    name: &str,
    bytes: &[u8],
    options: SimpleFileOptions,
) -> Result<(), String> {
    archive
        .start_file(name, options)
        .map_err(|error| format!("Could not add {name} to the support bundle: {error}"))?;
    archive
        .write_all(bytes)
        .map_err(|error| format!("Could not write {name} to the support bundle: {error}"))
}

impl Redactor {
    fn from_environment() -> Result<Self, String> {
        let username = env::var("USERNAME")
            .ok()
            .filter(|value| value.chars().count() > 1)
            .or_else(|| {
                env::var("USER")
                    .ok()
                    .filter(|value| value.chars().count() > 1)
            });
        let mut homes = Vec::new();
        for variable in ["USERPROFILE", "HOME"] {
            if let Some(value) = env::var_os(variable).filter(|value| !value.is_empty()) {
                let value = PathBuf::from(value).display().to_string();
                homes.push(value.clone());
                homes.push(value.replace('\\', "/"));
            }
        }
        Self::new(username.as_deref(), &homes)
    }

    fn new(username: Option<&str>, homes: &[String]) -> Result<Self, String> {
        let mut unique = HashSet::new();
        let mut compiled_homes = Vec::new();
        for home in homes.iter().filter(|value| !value.trim().is_empty()) {
            if unique.insert(home.to_ascii_lowercase()) {
                compiled_homes.push(case_insensitive_literal(home)?);
            }
        }
        let username = username
            .filter(|value| value.chars().count() > 1)
            .map(case_insensitive_literal)
            .transpose()?;
        Ok(Self {
            homes: compiled_homes,
            username,
        })
    }

    fn redact(&self, text: &str) -> String {
        let mut output = text.to_string();
        for home in &self.homes {
            output = home.replace_all(&output, "<HOME>").into_owned();
        }
        if let Some(username) = &self.username {
            output = username.replace_all(&output, "<USER>").into_owned();
        }
        output = redact_ip_addresses(&output);
        for (pattern, replacement) in redaction_patterns() {
            output = pattern.replace_all(&output, *replacement).into_owned();
        }
        output
    }
}

fn redaction_patterns() -> &'static Vec<(Regex, &'static str)> {
    static PATTERNS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        [
            (
                r#"(?i)\b(password|passwd|secret|token|api[_-]?key|authorization|auth|credential|session)\s*[:=]\s*[^\s,;"']+"#,
                "$1=<REDACTED>",
            ),
            (r"(?i)\bbearer\s+[A-Za-z0-9._~+/=-]+", "Bearer <REDACTED>"),
            (r"(?i)\b(?:gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,})\b", "<REDACTED_TOKEN>"),
            (r"(?i)(https?://)[^/\s:@]+:[^/\s@]+@", "$1<REDACTED>@"),
            (r"(?i)\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b", "<REDACTED_EMAIL>"),
            (r"(?i)\b(?:[0-9a-f]{2}[:-]){5}[0-9a-f]{2}\b", "<REDACTED_NETWORK_ADDRESS>"),
            (r"(?i)\b[A-Z]:[\\/]Users[\\/][^\\/\s]+", "<HOME>"),
            (r"(?i)/(?:home|users)/[^/\s]+", "<HOME>"),
        ]
        .into_iter()
        .map(|(pattern, replacement)| {
            (
                Regex::new(pattern).expect("support redaction regex must compile"),
                replacement,
            )
        })
        .collect()
    })
}

fn redact_ip_addresses(text: &str) -> String {
    static CANDIDATES: OnceLock<Regex> = OnceLock::new();
    let candidates = CANDIDATES.get_or_init(|| {
        Regex::new(r"(?i)[0-9a-f:.]{1,44}[0-9a-f]").expect("IP candidate regex must compile")
    });
    candidates
        .replace_all(text, |captures: &regex::Captures<'_>| {
            let value = captures.get(0).map_or("", |matched| matched.as_str());
            if value.parse::<IpAddr>().is_ok() || value.parse::<SocketAddr>().is_ok() {
                "<REDACTED_IP>".to_string()
            } else {
                value.to_string()
            }
        })
        .into_owned()
}

fn case_insensitive_literal(value: &str) -> Result<Regex, String> {
    RegexBuilder::new(&regex::escape(value))
        .case_insensitive(true)
        .build()
        .map_err(|error| format!("Could not prepare privacy redaction: {error}"))
}

fn support_plans() -> &'static Mutex<HashMap<String, SupportPlan>> {
    SUPPORT_PLANS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn require_confirmation(confirmed: bool) -> Result<(), String> {
    if confirmed {
        Ok(())
    } else {
        Err("Support bundle export requires explicit confirmation".into())
    }
}

fn reject_link_root(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("Could not inspect support bundle folder: {error}"))?;
    if is_link_like(&metadata) || !metadata.is_dir() {
        return Err("Support bundle folder is linked, redirected or not a directory".into());
    }
    Ok(())
}

fn is_link_like(metadata: &Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn nonempty_path(value: &str) -> Option<PathBuf> {
    (!value.trim().is_empty()).then(|| PathBuf::from(value.trim()))
}

fn safe_file_name(value: &str) -> String {
    let filtered: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .take(100)
        .collect();
    let filtered = filtered.trim_matches('.');
    if filtered.is_empty() {
        "latest-log".into()
    } else {
        filtered.into()
    }
}

fn value_or_unknown(value: &str) -> &str {
    if value.trim().is_empty() {
        "unknown"
    } else {
        value
    }
}

fn unix_millis(value: SystemTime) -> i64 {
    value
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use zip::ZipArchive;

    fn fixture_profile(install: &Path) -> GameProfile {
        let mut profile = crate::models::LauncherConfig::default().profiles.remove(0);
        profile.install_dir = install.display().to_string();
        profile.game_exe_path = install.join("game.exe").display().to_string();
        profile
    }

    fn fixture_health(profile: &GameProfile) -> ProfileHealth {
        crate::models::ProfileHealth {
            profile_id: profile.id.clone(),
            status: crate::models::ReadinessStatus::RepairNeeded,
            headline: "Files need attention".into(),
            details: vec!["Log path C:\\Users\\HixxyTest\\secret".into()],
        }
    }

    fn fixture_redactor() -> Redactor {
        Redactor::new(
            Some("HixxyTest"),
            &[r"C:\Users\HixxyTest".into(), "/home/HixxyTest".into()],
        )
        .unwrap()
    }

    #[test]
    fn redaction_removes_paths_secrets_accounts_and_network_identifiers() {
        let input = "C:\\Users\\HixxyTest\\AppData password=hunter2 token:abc123 Bearer abc.def@example user@example.com 203.0.113.9 fe80:0:0:0:1:2:3:4 ::1 00:11:22:33:44:55 ghp_abcdefghijklmnopqrstuvwxyz123456";
        let redacted = fixture_redactor().redact(input);
        for private in [
            "HixxyTest",
            "hunter2",
            "abc123",
            "abc.def@example",
            "user@example.com",
            "203.0.113.9",
            "fe80:0:0:0:1:2:3:4",
            "::1",
            "00:11:22:33:44:55",
            "ghp_abcdefghijklmnopqrstuvwxyz123456",
        ] {
            assert!(
                !redacted.contains(private),
                "private value survived: {private}"
            );
        }
        assert!(redacted.contains("<HOME>"));
        assert!(redacted.contains("<REDACTED>"));
    }

    #[test]
    fn minecraft_discovery_uses_known_log_locations_only() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("logs")).unwrap();
        fs::write(root.path().join("logs/latest.log"), b"latest").unwrap();
        fs::write(root.path().join("unrelated.log"), b"ignore").unwrap();
        let profile = fixture_profile(root.path());
        let directories = vec![
            (root.path().join("logs"), LogPattern::MinecraftLog),
            (
                root.path().join("crash-reports"),
                LogPattern::MinecraftCrash,
            ),
        ];
        let found = find_latest_in_directories(&directories).unwrap().unwrap();
        assert_eq!(found, root.path().join("logs/latest.log"));
        assert_ne!(found, root.path().join("unrelated.log"));
        assert_eq!(profile.game, "minecraft");
    }

    #[test]
    fn reviewed_bundle_contains_only_redacted_bounded_files() {
        let root = tempfile::tempdir().unwrap();
        let install = root.path().join("install");
        fs::create_dir_all(&install).unwrap();
        let log = install.join("latest.log");
        fs::write(
            &log,
            b"C:\\Users\\HixxyTest\\AppData password=hunter2 203.0.113.9\ncrash line",
        )
        .unwrap();
        let profile = fixture_profile(&install);
        let plan = build_plan(
            &profile,
            &fixture_health(&profile),
            Some(&log),
            &fixture_redactor(),
            SystemTime::now(),
        )
        .unwrap();
        assert!(plan.preview.ready);
        assert!(!plan.preview.redacted_log.contains("hunter2"));
        assert!(!plan.preview.redacted_log.contains("HixxyTest"));
        assert!(!plan.preview.redacted_log.contains("203.0.113.9"));

        let output = root.path().join("support");
        let outcome = write_bundle_at(&output, &plan).unwrap();
        let file = File::open(outcome.path).unwrap();
        let mut archive = ZipArchive::new(file).unwrap();
        assert_eq!(archive.len(), 3);
        assert!(archive.by_name("serverconfig.xml").is_err());
        let mut combined = String::new();
        for name in plan.preview.files {
            archive
                .by_name(&name)
                .unwrap()
                .read_to_string(&mut combined)
                .unwrap();
        }
        assert!(!combined.contains("hunter2"));
        assert!(!combined.contains("HixxyTest"));
        assert!(!combined.contains("203.0.113.9"));
        assert!(combined.contains("Server configuration included: no"));
    }

    #[test]
    fn support_export_is_fail_closed_without_confirmation() {
        assert!(require_confirmation(false).is_err());
        assert!(require_confirmation(true).is_ok());
    }
}
