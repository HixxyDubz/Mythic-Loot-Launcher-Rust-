use std::{
    fs,
    path::{Path, PathBuf},
    process,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use time::OffsetDateTime;

use crate::{remote, storage};

const ACTIVITY_FILE: &str = "activity-history.json";
const ACTIVITY_SCHEMA_VERSION: u32 = 1;
const MAX_ACTIVITY_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_STORED_ITEMS: usize = 100;
const RECENT_ITEMS: usize = 12;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ActivityKind {
    Catalogue,
    Storage,
    Verifying,
    Updating,
    Repairing,
    Restoring,
    Publishing,
    Launching,
    Setup,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityItem {
    pub id: String,
    pub title: String,
    pub kind: ActivityKind,
    pub message: String,
    pub progress: Option<f64>,
    pub bytes_done: Option<u64>,
    pub bytes_total: Option<u64>,
    pub started_at: i64,
    pub updated_at: i64,
    pub done: bool,
    pub success: Option<bool>,
    session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActivityStore {
    schema_version: u32,
    items: Vec<ActivityItem>,
}

impl Default for ActivityStore {
    fn default() -> Self {
        Self {
            schema_version: ACTIVITY_SCHEMA_VERSION,
            items: Vec::new(),
        }
    }
}

static ACTIVITY_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static SESSION_ID: OnceLock<String> = OnceLock::new();
static ACTIVITY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub fn recent(app: &AppHandle) -> Result<Vec<ActivityItem>, String> {
    recent_at(&activity_path(app)?, session_id())
}

pub fn clear_finished(app: &AppHandle) -> Result<Vec<ActivityItem>, String> {
    clear_finished_at(&activity_path(app)?, session_id())
}

pub fn has_active(app: &AppHandle) -> Result<bool, String> {
    has_active_at(&activity_path(app)?, session_id())
}

pub fn track<T, F, D>(
    app: &AppHandle,
    title: impl Into<String>,
    kind: ActivityKind,
    start_message: impl Into<String>,
    operation: F,
    describe: D,
) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String>,
    D: FnOnce(&T) -> (bool, String),
{
    let title = bounded(title.into(), 200);
    let start_message = bounded(start_message.into(), 4_000);
    let token = start(app, title, kind, start_message).map_err(|error| {
        eprintln!("Activity history could not record a start: {error}");
        error
    });
    let result = operation();
    if let Ok(id) = token {
        let (success, message) = match &result {
            Ok(value) => describe(value),
            Err(error) => (false, error.clone()),
        };
        if let Err(error) = finish(app, &id, success, &message) {
            eprintln!("Activity history could not record completion: {error}");
        }
    }
    result
}

fn start(
    app: &AppHandle,
    title: String,
    kind: ActivityKind,
    message: String,
) -> Result<String, String> {
    start_at(&activity_path(app)?, session_id(), title, kind, message)
}

fn finish(app: &AppHandle, id: &str, success: bool, message: &str) -> Result<(), String> {
    finish_at(&activity_path(app)?, session_id(), id, success, message)
}

fn activity_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(storage::data_dir(app)?.join(ACTIVITY_FILE))
}

fn session_id() -> &'static str {
    SESSION_ID.get_or_init(|| {
        format!(
            "{}-{}",
            process::id(),
            OffsetDateTime::now_utc().unix_timestamp_nanos()
        )
    })
}

fn now_millis() -> i64 {
    let value = OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn start_at(
    path: &Path,
    current_session: &str,
    title: String,
    kind: ActivityKind,
    message: String,
) -> Result<String, String> {
    with_store(path, current_session, |store| {
        let now = now_millis();
        let id = format!(
            "{}-{now}-{}",
            process::id(),
            ACTIVITY_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        );
        store.items.push(ActivityItem {
            id: id.clone(),
            title,
            kind,
            message,
            progress: None,
            bytes_done: None,
            bytes_total: None,
            started_at: now,
            updated_at: now,
            done: false,
            success: None,
            session_id: current_session.into(),
        });
        trim_store(store);
        Ok(id)
    })
}

fn finish_at(
    path: &Path,
    current_session: &str,
    id: &str,
    success: bool,
    message: &str,
) -> Result<(), String> {
    with_store(path, current_session, |store| {
        let item = store
            .items
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or_else(|| "The active activity item is no longer available".to_string())?;
        if item.done {
            return Ok(());
        }
        item.done = true;
        item.success = Some(success);
        item.progress = success.then_some(1.0);
        item.message = bounded(message.into(), 4_000);
        item.updated_at = now_millis();
        Ok(())
    })
}

fn recent_at(path: &Path, current_session: &str) -> Result<Vec<ActivityItem>, String> {
    let _guard = history_lock()
        .lock()
        .map_err(|_| "Activity history lock is unavailable".to_string())?;
    let (mut store, changed) = load_store(path, current_session)?;
    store
        .items
        .sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    if changed {
        save_store(path, &store)?;
    }
    Ok(store.items.into_iter().take(RECENT_ITEMS).collect())
}

fn clear_finished_at(path: &Path, current_session: &str) -> Result<Vec<ActivityItem>, String> {
    with_store(path, current_session, |store| {
        store.items.retain(|item| !item.done);
        store
            .items
            .sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(store.items.iter().take(RECENT_ITEMS).cloned().collect())
    })
}

fn has_active_at(path: &Path, current_session: &str) -> Result<bool, String> {
    let _guard = history_lock()
        .lock()
        .map_err(|_| "Activity history lock is unavailable".to_string())?;
    let (store, changed) = load_store(path, current_session)?;
    let active = store.items.iter().any(|item| !item.done);
    if changed {
        save_store(path, &store)?;
    }
    Ok(active)
}

fn with_store<T>(
    path: &Path,
    current_session: &str,
    mutate: impl FnOnce(&mut ActivityStore) -> Result<T, String>,
) -> Result<T, String> {
    let _guard = history_lock()
        .lock()
        .map_err(|_| "Activity history lock is unavailable".to_string())?;
    let (mut store, _) = load_store(path, current_session)?;
    let result = mutate(&mut store)?;
    trim_store(&mut store);
    save_store(path, &store)?;
    Ok(result)
}

fn load_store(path: &Path, current_session: &str) -> Result<(ActivityStore, bool), String> {
    if !path.is_file() {
        return Ok((ActivityStore::default(), false));
    }
    let size = path
        .metadata()
        .map_err(|error| format!("Could not inspect activity history: {error}"))?
        .len();
    if size > MAX_ACTIVITY_FILE_BYTES {
        preserve_corrupt(path)?;
        return Ok((ActivityStore::default(), false));
    }
    let bytes =
        fs::read(path).map_err(|error| format!("Could not read activity history: {error}"))?;
    let mut store: ActivityStore = match serde_json::from_slice::<ActivityStore>(&bytes) {
        Ok(store) if store.schema_version == ACTIVITY_SCHEMA_VERSION => store,
        _ => {
            preserve_corrupt(path)?;
            return Ok((ActivityStore::default(), false));
        }
    };
    let mut changed = false;
    let now = now_millis();
    for item in &mut store.items {
        if !item.done && item.session_id != current_session {
            item.done = true;
            item.success = Some(false);
            item.progress = None;
            item.message = "Launcher closed before this operation reported completion.".into();
            item.updated_at = now;
            changed = true;
        }
    }
    let before = store.items.len();
    store.items.retain(valid_item);
    changed |= store.items.len() != before;
    let before_trim = store.items.len();
    trim_store(&mut store);
    changed |= store.items.len() != before_trim;
    Ok((store, changed))
}

fn valid_item(item: &ActivityItem) -> bool {
    !item.id.is_empty()
        && item.id.len() <= 200
        && !item.title.trim().is_empty()
        && item.title.chars().count() <= 200
        && item.message.chars().count() <= 4_000
        && item.started_at >= 0
        && item.updated_at >= item.started_at
        && item
            .progress
            .is_none_or(|progress| progress.is_finite() && (0.0..=1.0).contains(&progress))
        && item
            .bytes_done
            .zip(item.bytes_total)
            .is_none_or(|(done, total)| done <= total)
}

fn trim_store(store: &mut ActivityStore) {
    store
        .items
        .sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    store.items.truncate(MAX_STORED_ITEMS);
}

fn save_store(path: &Path, store: &ActivityStore) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(store)
        .map_err(|error| format!("Could not serialize activity history: {error}"))?;
    bytes.push(b'\n');
    remote::write_atomic(path, &bytes).map(|_| ())
}

fn preserve_corrupt(path: &Path) -> Result<(), String> {
    let stamp = OffsetDateTime::now_utc().unix_timestamp_nanos();
    let preserved = path.with_file_name(format!("activity-history.corrupt-{stamp}.json"));
    fs::rename(path, &preserved).map_err(|error| {
        format!(
            "Activity history is invalid and could not be preserved as {}: {error}",
            preserved.display()
        )
    })
}

fn bounded(value: String, maximum: usize) -> String {
    value.chars().take(maximum).collect()
}

fn history_lock() -> &'static Mutex<()> {
    ACTIVITY_LOCK.get_or_init(|| Mutex::new(()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activity_lifecycle_lists_recent_and_clears_only_finished() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(ACTIVITY_FILE);
        let first = start_at(
            &path,
            "session-a",
            "Minecraft".into(),
            ActivityKind::Verifying,
            "Scanning files".into(),
        )
        .unwrap();
        let second = start_at(
            &path,
            "session-a",
            "7 Days".into(),
            ActivityKind::Updating,
            "Staging update".into(),
        )
        .unwrap();
        finish_at(&path, "session-a", &first, true, "All files match").unwrap();
        let recent = recent_at(&path, "session-a").unwrap();
        assert_eq!(recent.len(), 2);
        assert!(recent.iter().any(|item| item.id == first && item.done));
        assert!(recent.iter().any(|item| item.id == second && !item.done));
        let remaining = clear_finished_at(&path, "session-a").unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, second);
    }

    #[test]
    fn unfinished_activity_from_an_old_session_becomes_failed() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(ACTIVITY_FILE);
        let id = start_at(
            &path,
            "old-session",
            "Release".into(),
            ActivityKind::Publishing,
            "Uploading".into(),
        )
        .unwrap();
        let recent = recent_at(&path, "new-session").unwrap();
        let item = recent.iter().find(|item| item.id == id).unwrap();
        assert!(item.done);
        assert_eq!(item.success, Some(false));
        assert!(item.message.contains("closed before"));
    }

    #[test]
    fn corrupt_history_is_preserved_before_recovery() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join(ACTIVITY_FILE);
        fs::write(&path, b"not json").unwrap();
        assert!(recent_at(&path, "session").unwrap().is_empty());
        assert!(!path.exists());
        assert!(fs::read_dir(root.path()).unwrap().flatten().any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("activity-history.corrupt-")
        }));
    }
}
