//! Process-lifecycle admission and cross-edition maintenance exclusion.
//!
//! One OS lock serializes maintenance even when profiles use overlapping roots
//! or Player and Developer run together. Nothing is written into a game folder.
use std::{
    fs::{self, File, OpenOptions},
    path::Path,
    sync::{Arc, Mutex, OnceLock},
};

#[derive(Default)]
struct Lifecycle {
    active: usize,
    closing: bool,
    shutdown_pending: bool,
}
static LIFECYCLE: OnceLock<Arc<Mutex<Lifecycle>>> = OnceLock::new();
fn lifecycle() -> Arc<Mutex<Lifecycle>> {
    LIFECYCLE.get_or_init(Default::default).clone()
}

pub struct WorkGuard {
    state: Arc<Mutex<Lifecycle>>,
}
impl WorkGuard {
    pub fn begin() -> Result<Self, String> {
        Self::begin_at(lifecycle())
    }
    fn begin_at(state: Arc<Mutex<Lifecycle>>) -> Result<Self, String> {
        {
            let mut current = state
                .lock()
                .map_err(|_| "Launcher operation state is unavailable")?;
            if current.closing {
                return Err("The launcher is closing or restarting; no new work can start".into());
            }
            current.active += 1;
        }
        Ok(Self { state })
    }
}
impl Drop for WorkGuard {
    fn drop(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            state.active = state.active.saturating_sub(1);
        }
    }
}

pub struct ShutdownGuard {
    state: Arc<Mutex<Lifecycle>>,
    committed: bool,
}
impl ShutdownGuard {
    pub fn begin() -> Result<Self, String> {
        Self::begin_at(lifecycle())
    }
    fn begin_at(state: Arc<Mutex<Lifecycle>>) -> Result<Self, String> {
        {
            let mut current = state
                .lock()
                .map_err(|_| "Launcher operation state is unavailable")?;
            if current.active != 0 || current.closing {
                return Err(
                    "Wait for current launcher work to finish before installing an app update"
                        .into(),
                );
            }
            current.closing = true;
            current.shutdown_pending = true;
        }
        Ok(Self {
            state,
            committed: false,
        })
    }
    pub fn commit(mut self) {
        if let Ok(mut state) = self.state.lock() {
            state.shutdown_pending = false;
        }
        self.committed = true;
    }
}
impl Drop for ShutdownGuard {
    fn drop(&mut self) {
        if !self.committed
            && let Ok(mut state) = self.state.lock()
        {
            state.closing = false;
            state.shutdown_pending = false;
        }
    }
}

pub fn request_close() -> bool {
    request_close_at(&lifecycle())
}
fn request_close_at(state: &Mutex<Lifecycle>) -> bool {
    let Ok(mut state) = state.lock() else {
        return false;
    };
    if state.active != 0 || state.shutdown_pending {
        return false;
    }
    state.closing = true;
    true
}

pub struct MaintenanceGuard {
    _file: File,
    _work: WorkGuard,
}
impl MaintenanceGuard {
    pub fn acquire() -> Result<Self, String> {
        let root = std::env::var_os("LOCALAPPDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("MythicLootLauncher")
            .join("locks");
        Self::acquire_at(&root)
    }
    fn acquire_at(root: &Path) -> Result<Self, String> {
        let work = WorkGuard::begin()?;
        crate::safe_path::reject_link_path(root)?;
        fs::create_dir_all(root)
            .map_err(|e| format!("Could not create maintenance lock directory: {e}"))?;
        let path = root.join("maintenance.lock");
        crate::safe_path::reject_link_path(&path)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|e| format!("Could not open maintenance lock: {e}"))?;
        fs2::FileExt::try_lock_exclusive(&file).map_err(|_| "Another Player or Developer operation is using the modpack files. Wait for it to finish, then try again.".to_string())?;
        Ok(Self {
            _file: file,
            _work: work,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "invoked by the cross-process lock regression"]
    fn maintenance_lock_child() {
        let root = std::env::var_os("MYTHIC_TEST_LOCK_ROOT").expect("test root");
        let expected = std::env::var("MYTHIC_TEST_LOCK_AVAILABLE").unwrap() == "yes";
        assert_eq!(
            MaintenanceGuard::acquire_at(Path::new(&root)).is_ok(),
            expected
        );
    }

    #[test]
    fn maintenance_lock_is_enforced_across_processes() {
        let root = tempfile::tempdir().unwrap();
        let child = |available| {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "operations::tests::maintenance_lock_child",
                    "--ignored",
                ])
                .env("MYTHIC_TEST_LOCK_ROOT", root.path())
                .env("MYTHIC_TEST_LOCK_AVAILABLE", available)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stdout)
            );
        };
        let guard = MaintenanceGuard::acquire_at(root.path()).unwrap();
        child("no");
        drop(guard);
        child("yes");
    }
    #[test]
    fn maintenance_is_exclusive_and_released_on_drop() {
        let root = tempfile::tempdir().unwrap();
        let first = MaintenanceGuard::acquire_at(root.path()).unwrap();
        assert!(MaintenanceGuard::acquire_at(root.path()).is_err());
        drop(first);
        assert!(MaintenanceGuard::acquire_at(root.path()).is_ok());
    }
    #[test]
    fn close_and_update_admission_are_atomic_with_work() {
        let state = Arc::new(Mutex::new(Lifecycle::default()));
        let work = WorkGuard::begin_at(state.clone()).unwrap();
        assert!(!request_close_at(&state));
        assert!(ShutdownGuard::begin_at(state.clone()).is_err());
        drop(work);
        let shutdown = ShutdownGuard::begin_at(state.clone()).unwrap();
        assert!(WorkGuard::begin_at(state.clone()).is_err());
        assert!(!request_close_at(&state)); // The window must stay open until helper readiness.
        drop(shutdown); // Failed helper leaves the app usable.
        assert!(WorkGuard::begin_at(state.clone()).is_ok());
        assert!(request_close_at(&state));
        assert!(WorkGuard::begin_at(state).is_err());
    }

    #[test]
    fn verified_helper_commit_allows_exit_but_not_new_work() {
        let state = Arc::new(Mutex::new(Lifecycle::default()));
        let shutdown = ShutdownGuard::begin_at(state.clone()).unwrap();
        assert!(!request_close_at(&state));
        shutdown.commit();
        assert!(request_close_at(&state));
        assert!(WorkGuard::begin_at(state).is_err());
    }
}
