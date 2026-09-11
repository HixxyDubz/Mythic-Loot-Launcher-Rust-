use serde::Deserialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LocalPathKind {
    Folder,
    Executable,
}

#[tauri::command]
pub async fn choose_local_path(
    app: AppHandle,
    kind: LocalPathKind,
) -> Result<Option<String>, String> {
    let guard = crate::operations::WorkGuard::begin()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let mut dialog = app.dialog().file();
        if let Some(window) = app.get_webview_window("main") {
            dialog = dialog.set_parent(&window);
        }
        let selected = match kind {
            LocalPathKind::Folder => dialog
                .set_title("Choose a local folder")
                .blocking_pick_folder(),
            LocalPathKind::Executable => dialog
                .set_title("Choose the game or launcher executable")
                .add_filter("Windows executable", &["exe"])
                .blocking_pick_file(),
        };
        let Some(selected) = selected else {
            return Ok(None);
        };
        let path = selected
            .into_path()
            .map_err(|error| format!("Choose a local path: {error}"))?;
        crate::safe_path::reject_link_path(&path)?;
        let valid = match kind {
            LocalPathKind::Folder => path.is_dir(),
            LocalPathKind::Executable => {
                path.is_file()
                    && path
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
            }
        };
        if !valid {
            return Err(
                "The selected path is not an existing folder or executable of the requested type"
                    .into(),
            );
        }
        Ok(Some(path.display().to_string()))
    })
    .await
    .map_err(|error| format!("Folder selection could not finish: {error}"))?
}
