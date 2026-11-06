mod app;

use app::{AddResult, App, DeleteOutcome, LibraryPatch, QueueItem, RemoteList, SettingsPatch, Snapshot};
use rdbatch_core::error::Error;
use rdbatch_core::models::{Msg, Settings, User};
use serde::Serialize;
use std::sync::Arc;
use tauri::{Manager, State};
use tauri_plugin_opener::OpenerExt;

/// Errors cross the IPC boundary as `{ key, args }` so the UI can translate them.
#[derive(Serialize)]
pub struct ApiError(Msg);

impl From<Error> for ApiError {
    fn from(error: Error) -> Self {
        ApiError(error.friendly())
    }
}

type Cmd<T> = Result<T, ApiError>;
type Shared<'a> = State<'a, Arc<App>>;

#[tauri::command]
fn bootstrap(app: Shared) -> Snapshot {
    app.snapshot()
}

#[tauri::command]
fn update_settings(app: Shared, patch: SettingsPatch) -> Settings {
    app.update_settings(patch)
}

#[tauri::command]
fn save_token(app: Shared, token: String) -> Option<Msg> {
    app.save_token(&token)
}

#[tauri::command]
async fn connect(app: Shared<'_>) -> Cmd<User> {
    Ok(app.connect().await?)
}

#[tauri::command]
fn library_add(app: Shared, text: String) -> AddResult {
    app.inner().library_add(&text)
}

#[tauri::command]
fn library_update(app: Shared, patches: Vec<LibraryPatch>) {
    app.library_update(patches)
}

#[tauri::command]
fn library_reset(app: Shared) {
    app.library_reset()
}

#[tauri::command]
fn library_cancel_inspect(app: Shared) {
    app.library_cancel_inspect()
}

#[tauri::command]
fn library_retry(app: Shared) {
    app.inner().library_retry()
}

#[tauri::command]
fn queue_add(app: Shared, items: Vec<QueueItem>) -> Vec<String> {
    app.queue_add(items)
}

#[tauri::command]
async fn queue_start(app: Shared<'_>, ids: Option<Vec<String>>) -> Cmd<()> {
    Ok(app.inner().queue_start(ids).await?)
}

#[tauri::command]
async fn queue_pause(app: Shared<'_>) -> Cmd<()> {
    app.queue_pause().await;
    Ok(())
}

#[tauri::command]
async fn queue_stop(app: Shared<'_>, ids: Option<Vec<String>>) -> Cmd<()> {
    app.queue_stop(ids).await;
    Ok(())
}

#[tauri::command]
async fn queue_remove(app: Shared<'_>, ids: Vec<String>) -> Cmd<Vec<String>> {
    Ok(app.queue_remove(ids).await)
}

#[tauri::command]
async fn queue_clear(app: Shared<'_>, only_completed: bool) -> Cmd<Vec<String>> {
    Ok(app.queue_clear(only_completed).await)
}

#[tauri::command]
async fn remote_list(app: Shared<'_>) -> Cmd<RemoteList> {
    Ok(app.remote_list().await?)
}

#[tauri::command]
async fn remote_delete(app: Shared<'_>, ids: Vec<String>, list_account: String) -> Cmd<Vec<DeleteOutcome>> {
    Ok(app.remote_delete(ids, list_account).await?)
}

#[tauri::command]
fn open_folder(handle: tauri::AppHandle, path: String) -> Cmd<()> {
    let target = std::path::PathBuf::from(&path);
    if !target.is_dir() {
        return Err(ApiError(Msg::plain("Thư mục không tồn tại.")));
    }
    handle.opener().open_path(path, None::<&str>).map_err(|e| ApiError(Msg::plain(&e.to_string())))
}

/// Only the three provider token pages can be opened from the UI.
#[tauri::command]
fn open_token_page(handle: tauri::AppHandle, provider: rdbatch_core::models::Provider) -> Cmd<()> {
    handle.opener().open_url(provider.token_page(), None::<&str>).map_err(|e| ApiError(Msg::plain(&e.to_string())))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // RDBATCH_DATA_DIR keeps all data in one folder (tests, screenshots). A file named `portable` next to the
            // executable does the same with `<exe folder>/data`, which is how the portable zip is laid out.
            let portable = std::env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf))
                .filter(|folder| folder.join("portable").is_file())
                .map(|folder| folder.join("data"));
            let directory = match (std::env::var_os("RDBATCH_DATA_DIR"), portable) {
                (Some(custom), _) => std::path::PathBuf::from(custom),
                (None, Some(folder)) => folder,
                (None, None) => app.path().app_data_dir()?,
            };
            let downloads = app.path().download_dir().map(|d| d.join("DebridDock")).unwrap_or_else(|_| directory.join("downloads"));
            let state = App::new(app.handle().clone(), directory, downloads.to_string_lossy().to_string());
            tauri::async_runtime::spawn(state.clone().background());
            state.clone().start_inspection();
            app.manage(state);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                if let Some(app) = window.try_state::<Arc<App>>() {
                    app.save_now();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            update_settings,
            save_token,
            connect,
            library_add,
            library_update,
            library_reset,
            library_cancel_inspect,
            library_retry,
            queue_add,
            queue_start,
            queue_pause,
            queue_stop,
            queue_remove,
            queue_clear,
            remote_list,
            remote_delete,
            open_folder,
            open_token_page,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the application");
}
