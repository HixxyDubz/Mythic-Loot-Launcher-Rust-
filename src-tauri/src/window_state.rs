//! Windows-only, edition-local placement. Never restore minimized/hidden windows.
use crate::{safe_path, storage};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    path::Path,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{Manager, Window};
use windows_sys::Win32::{
    Foundation::{POINT, RECT},
    UI::WindowsAndMessaging::{
        GetWindowPlacement, SW_SHOWMAXIMIZED, SW_SHOWMINIMIZED, SW_SHOWNORMAL, SetWindowPlacement,
        WINDOWPLACEMENT,
    },
};

const FILE: &str = "window-state.json";
const MAX_BYTES: u64 = 4096;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Geometry {
    schema_version: u32,
    x: i32,
    y: i32,
    width: f64,
    height: f64,
    scale: f64,
    maximized: bool,
}

impl Geometry {
    fn valid(&self) -> bool {
        self.schema_version == 1
            && self.x.abs_diff(0) <= 1_000_000
            && self.y.abs_diff(0) <= 1_000_000
            && self.width.is_finite()
            && self.height.is_finite()
            && self.scale.is_finite()
            && (100.0..=16384.0).contains(&self.width)
            && (100.0..=16384.0).contains(&self.height)
            && (0.5..=8.0).contains(&self.scale)
    }
}
struct Cache(Mutex<Option<Geometry>>);

#[derive(Clone, Copy, Debug)]
struct Screen {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    scale: f64,
    offset_x: i32,
    offset_y: i32,
}

fn screens(window: &Window) -> Result<Vec<Screen>, String> {
    let primary = window.primary_monitor().map_err(|e| e.to_string())?;
    let mut monitors = window.available_monitors().map_err(|e| e.to_string())?;
    // Fallback should be the primary work area, not an arbitrary enumeration entry.
    monitors.sort_by_key(|m| {
        primary
            .as_ref()
            .is_none_or(|p| p.position() != m.position())
    });
    Ok(monitors
        .into_iter()
        .filter_map(|m| {
            let area = m.work_area();
            (area.size.width > 0 && area.size.height > 0 && (0.5..=8.0).contains(&m.scale_factor()))
                .then_some(Screen {
                    x: area.position.x,
                    y: area.position.y,
                    width: area.size.width,
                    height: area.size.height,
                    scale: m.scale_factor(),
                    offset_x: area.position.x - m.position().x,
                    offset_y: area.position.y - m.position().y,
                })
        })
        .collect())
}

// Return a physical screen rectangle and the monitor whose work area contains it.
fn fit(saved: Option<&Geometry>, screens: &[Screen]) -> Option<(Screen, i32, i32, u32, u32, bool)> {
    let saved = saved.filter(|g| g.valid());
    let mut chosen = *screens.first()?;
    let mut overlap = 0_i64;
    if let Some(g) = saved {
        for screen in screens {
            let width = (i64::from(screen.x) + i64::from(screen.width))
                .min(i64::from(g.x) + (g.width * g.scale) as i64)
                - i64::from(screen.x.max(g.x));
            let height = (i64::from(screen.y) + i64::from(screen.height))
                .min(i64::from(g.y) + (g.height * g.scale) as i64)
                - i64::from(screen.y.max(g.y));
            let area = width.max(0) * height.max(0);
            if area > overlap {
                chosen = *screen;
                overlap = area;
            }
        }
    }
    let width = ((saved.map_or(1180.0, |g| g.width).max(940.0) * chosen.scale).round() as u32)
        .min(chosen.width);
    let height = ((saved.map_or(760.0, |g| g.height).max(640.0) * chosen.scale).round() as u32)
        .min(chosen.height);
    let max_x = i64::from(chosen.x) + i64::from(chosen.width - width);
    let max_y = i64::from(chosen.y) + i64::from(chosen.height - height);
    let (x, y) = if let Some(g) = saved.filter(|_| overlap > 0) {
        (
            i64::from(g.x).clamp(i64::from(chosen.x), max_x),
            i64::from(g.y).clamp(i64::from(chosen.y), max_y),
        )
    } else {
        (
            (i64::from(chosen.x) + max_x) / 2,
            (i64::from(chosen.y) + max_y) / 2,
        )
    };
    Some((
        chosen,
        x as i32,
        y as i32,
        width,
        height,
        saved.is_some_and(|g| g.maximized),
    ))
}

fn capture(window: &Window) -> Result<Option<Geometry>, String> {
    let hwnd = window.hwnd().map_err(|e| e.to_string())?;
    let mut placement = WINDOWPLACEMENT {
        length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
        ..Default::default()
    };
    // HWND is owned by this live Tauri window; placement has the required size and lifetime.
    if unsafe { GetWindowPlacement(hwnd.0, &mut placement) } == 0 {
        return Err("Cannot read native window placement".into());
    }
    if placement.showCmd == SW_SHOWMINIMIZED as u32 {
        return Ok(None);
    }
    let monitor = window
        .current_monitor()
        .map_err(|e| e.to_string())?
        .ok_or("No current monitor")?;
    let scale = monitor.scale_factor();
    let normal = placement.rcNormalPosition;
    let g = Geometry {
        schema_version: 1,
        x: normal
            .left
            .saturating_add(monitor.work_area().position.x - monitor.position().x),
        y: normal
            .top
            .saturating_add(monitor.work_area().position.y - monitor.position().y),
        width: (i64::from(normal.right) - i64::from(normal.left)) as f64 / scale,
        height: (i64::from(normal.bottom) - i64::from(normal.top)) as f64 / scale,
        scale,
        maximized: placement.showCmd == SW_SHOWMAXIMIZED as u32,
    };
    Ok(g.valid().then_some(g))
}

pub fn track(window: &Window) {
    if window.label() != "main" {
        return;
    }
    if let Some(cache) = window.app_handle().try_state::<Cache>()
        && let Ok(Some(g)) = capture(window)
        && let Ok(mut state) = cache.0.lock()
    {
        *state = Some(g);
    }
}

pub fn initialize(window: &Window) {
    let saved = (|| -> Result<Option<Geometry>, String> {
        let config = storage::load_or_create(window.app_handle())?;
        if !config.preferences.remember_window {
            return Ok(None);
        }
        let root = storage::data_dir(window.app_handle())?;
        let _lock = lock(&root)?;
        read_at(&root.join(FILE))
    })()
    .unwrap_or_else(|e| {
        eprintln!("Window state ignored: {e}");
        None
    });
    if let Err(e) = restore(window, saved.as_ref()) {
        eprintln!("Could not restore window placement: {e}");
    }
    window
        .app_handle()
        .manage(Cache(Mutex::new(capture(window).ok().flatten())));
}

fn restore(window: &Window, saved: Option<&Geometry>) -> Result<(), String> {
    let Some((screen, x, y, width, height, maximized)) = fit(saved, &screens(window)?) else {
        return Ok(());
    };
    window
        .set_min_size(Some(tauri::PhysicalSize::new(
            ((940.0 * screen.scale).round() as u32).min(screen.width),
            ((640.0 * screen.scale).round() as u32).min(screen.height),
        )))
        .map_err(|e| e.to_string())?;
    let hwnd = window.hwnd().map_err(|e| e.to_string())?;
    let left = x - screen.offset_x;
    let top = y - screen.offset_y;
    let placement = WINDOWPLACEMENT {
        length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
        flags: 0,
        showCmd: if maximized {
            SW_SHOWMAXIMIZED
        } else {
            SW_SHOWNORMAL
        } as u32,
        ptMinPosition: POINT { x: -1, y: -1 },
        ptMaxPosition: POINT { x: -1, y: -1 },
        rcNormalPosition: RECT {
            left,
            top,
            right: left + width as i32,
            bottom: top + height as i32,
        },
    };
    // Convert screen coordinates back to Win32 workspace coordinates; never feed them to SetWindowPos.
    if unsafe { SetWindowPlacement(hwnd.0, &placement) } == 0 {
        return Err("Cannot apply native window placement".into());
    }
    Ok(())
}

pub fn save(window: &Window) {
    track(window);
    let result = (|| -> Result<(), String> {
        if !storage::load_or_create(window.app_handle())?
            .preferences
            .remember_window
        {
            return Ok(());
        }
        let Some(cache) = window.app_handle().try_state::<Cache>() else {
            return Ok(());
        };
        let g = cache
            .0
            .lock()
            .map_err(|_| "Window state lock unavailable")?
            .clone();
        if let Some(g) = g {
            save_at(&storage::data_dir(window.app_handle())?, &g)?;
        }
        Ok(())
    })();
    if let Err(e) = result {
        eprintln!("Could not save window placement: {e}");
    }
}

fn lock(root: &Path) -> Result<File, String> {
    safe_path::reject_link_path(root)?;
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let path = root.join("window-state.lock");
    safe_path::reject_link_path(&path)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|e| e.to_string())?;
    fs2::FileExt::try_lock_exclusive(&file)
        .map_err(|_| "Window state is being saved by another instance")?;
    Ok(file)
}

fn read_at(path: &Path) -> Result<Option<Geometry>, String> {
    safe_path::reject_link_path(path)?;
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > MAX_BYTES {
        return Err("Invalid window-state file size/type".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("Oversized window-state file".into());
    }
    let geometry: Geometry =
        serde_json::from_slice(&bytes).map_err(|_| "Malformed window state")?;
    if !geometry.valid() {
        return Err("Unsupported window state".into());
    }
    Ok(Some(geometry))
}

fn save_at(root: &Path, g: &Geometry) -> Result<(), String> {
    if !g.valid() {
        return Err("Invalid geometry cannot be saved".into());
    }
    let _lock = lock(root)?;
    let path = root.join(FILE);
    for path in [
        &path,
        &root.join("window-state.json.download"),
        &root.join("window-state.json.previous"),
    ] {
        safe_path::reject_link_path(path)?;
    }
    if path.exists() && read_at(&path).is_err() {
        if !fs::symlink_metadata(&path)
            .map_err(|e| e.to_string())?
            .is_file()
        {
            return Err("Window-state destination is not a regular file".into());
        }
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let preserved = root.join(format!("window-state.invalid-{stamp}.json"));
        if preserved.exists() {
            return Err("Window state recovery name already exists".into());
        }
        fs::rename(&path, preserved).map_err(|e| e.to_string())?;
    }
    let bytes = serde_json::to_vec_pretty(g).map_err(|e| e.to_string())?;
    crate::remote::write_atomic(&path, &bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn geometry() -> Geometry {
        Geometry {
            schema_version: 1,
            x: 120,
            y: 100,
            width: 1100.0,
            height: 700.0,
            scale: 1.0,
            maximized: false,
        }
    }
    fn screen() -> Screen {
        Screen {
            x: 0,
            y: 40,
            width: 1920,
            height: 1040,
            scale: 1.0,
            offset_x: 0,
            offset_y: 40,
        }
    }

    #[test]
    fn keeps_normal_bounds_and_maximized_state_separate() {
        let g = geometry();
        let (_, x, y, w, h, max) = fit(Some(&g), &[screen()]).unwrap();
        assert_eq!((x, y, w, h, max), (120, 100, 1100, 700, false));
        let mut maximized = g;
        maximized.maximized = true;
        let (_, x, y, w, h, max) = fit(Some(&maximized), &[screen()]).unwrap();
        assert_eq!((x, y, w, h, max), (120, 100, 1100, 700, true));
    }

    #[test]
    fn missing_monitor_recenters_and_small_work_areas_are_respected() {
        let mut g = geometry();
        g.x = 5000;
        let (_, x, y, w, h, _) = fit(Some(&g), &[screen()]).unwrap();
        assert_eq!((x, y, w, h), (410, 210, 1100, 700));
        let small = Screen {
            width: 800,
            height: 500,
            ..screen()
        };
        let (_, x, y, w, h, _) = fit(Some(&g), &[small]).unwrap();
        assert_eq!((x, y, w, h), (0, 40, 800, 500));
        assert!(fit(Some(&g), &[]).is_none());
    }

    #[test]
    fn supports_negative_coordinates_and_rescales_to_current_dpi() {
        let left = Screen {
            x: -2560,
            width: 2560,
            height: 1400,
            scale: 1.5,
            ..screen()
        };
        let g = Geometry {
            x: -2200,
            ..geometry()
        };
        let (_, x, y, w, h, _) = fit(Some(&g), &[screen(), left]).unwrap();
        assert_eq!((x, y, w, h), (-2200, 100, 1650, 1050));
    }

    #[test]
    fn rejects_bad_geometry_and_defaults_never_restore_maximized() {
        for g in [
            Geometry {
                schema_version: 2,
                ..geometry()
            },
            Geometry {
                width: f64::NAN,
                ..geometry()
            },
            Geometry {
                height: -1.0,
                ..geometry()
            },
            Geometry {
                scale: 0.0,
                ..geometry()
            },
            Geometry {
                x: i32::MIN,
                ..geometry()
            },
        ] {
            assert!(!g.valid());
            let (_, x, y, w, h, max) = fit(Some(&g), &[screen()]).unwrap();
            assert_eq!((x, y, w, h, max), (370, 180, 1180, 760, false));
        }
    }

    #[test]
    fn placement_roundtrip_does_not_touch_modpack_settings() {
        let root = tempfile::tempdir().unwrap();
        let config = root.path().join("launcher-config.json");
        fs::write(&config, "profiles remain untouched").unwrap();
        save_at(root.path(), &geometry()).unwrap();
        assert_eq!(read_at(&root.path().join(FILE)).unwrap(), Some(geometry()));
        assert_eq!(
            fs::read_to_string(config).unwrap(),
            "profiles remain untouched"
        );
        save_at(root.path(), &geometry()).unwrap();
        assert!(!root.path().join("window-state.json.previous").exists());
    }

    #[test]
    fn malformed_state_is_bounded_and_preserved_before_replacement() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join(FILE);
        assert_eq!(read_at(&file).unwrap(), None);
        fs::write(&file, "broken geometry").unwrap();
        assert!(read_at(&file).is_err());
        save_at(root.path(), &geometry()).unwrap();
        let backup = fs::read_dir(root.path())
            .unwrap()
            .flatten()
            .find(|p| {
                p.file_name()
                    .to_string_lossy()
                    .starts_with("window-state.invalid-")
            })
            .unwrap();
        assert_eq!(
            fs::read_to_string(backup.path()).unwrap(),
            "broken geometry"
        );
        fs::write(&file, vec![b'x'; MAX_BYTES as usize + 1]).unwrap();
        assert!(read_at(&file).is_err());
    }

    #[test]
    fn concurrent_writer_fails_without_blocking_or_mutating() {
        let root = tempfile::tempdir().unwrap();
        let _guard = lock(root.path()).unwrap();
        assert!(save_at(root.path(), &geometry()).is_err());
        assert!(!root.path().join(FILE).exists());
    }

    #[test]
    fn directory_at_state_path_is_not_moved_or_removed() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join(FILE)).unwrap();
        assert!(save_at(root.path(), &geometry()).is_err());
        assert!(root.path().join(FILE).is_dir());
    }
}
