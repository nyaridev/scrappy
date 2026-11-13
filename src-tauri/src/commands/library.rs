//! Library commands: where the library stands, picking or checking its folder, setting it up or switching to another,
//! moving it as a job (ADR-0039), its numbers and recent downloads, the gallery's pages, thumbnails and pictures, opening a file or showing it in the file manager, and
//! deleting a file or downloaded folder. Core checks every path, typed or picked.

use scrappy_core::library::{self, Disk, FolderCheck, GalleryPage, GalleryQuery, LibraryStatus, RecentDownload, Stats};
use scrappy_core::jobs::{JobId, Jobs};
use scrappy_core::media::{Downloads, Thumbnails};
use scrappy_core::settings::Store;
use std::sync::Arc;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

/// Where the library from the settings stands.
#[tauri::command(async)]
pub fn library_status(settings: State<Store>) -> LibraryStatus {
    library::status(settings.get().library_root.as_deref())
}

/// Opens the system folder picker and checks the folder; `None` when the user cancels.
#[tauri::command]
pub async fn pick_folder(app: AppHandle) -> Result<Option<FolderCheck>, String> {
    // Blocking is fine here: async commands run off the main thread, which the dialog needs.
    let Some(choice) = app.dialog().file().set_title("Choose a library folder").blocking_pick_folder() else {
        return Ok(None);
    };
    let path = choice.into_path().map_err(|e| format!("Can't use this folder: {e}"))?;
    library::check(path.to_str().ok_or("Can't use this folder's name.")?).map(Some)
}

/// Checks a folder path the user typed.
#[tauri::command(async)]
pub fn check_folder(path: String) -> Result<FolderCheck, String> {
    library::check(&path)
}

/// Sets up the library at `path`, at first run or to switch to another folder; the Gallery lists the new one.
#[tauri::command(async)]
pub fn setup_library(app: AppHandle, settings: State<Store>, path: String) -> Result<LibraryStatus, String> {
    let status = library::setup(&path, &settings)?;
    crate::events::library_watcher(app)();
    Ok(status)
}

/// Starts moving the library to `path` as a job; refused while downloads run. Returns the job's id.
#[tauri::command(async)]
pub fn move_library(
    app: AppHandle,
    settings: State<Store>,
    jobs: State<Arc<Jobs>>,
    downloads: State<Downloads>,
    path: String,
) -> Result<JobId, String> {
    let on_change = crate::events::library_watcher(app);
    library::start_move(&jobs, settings.inner().clone(), &path, downloads.busy(), on_change)
}

/// The library's totals and the last 30 days.
#[tauri::command(async)]
pub fn library_stats(settings: State<Store>) -> Result<Stats, String> {
    library::stats(&settings.library_root()?)
}

/// The latest downloads, newest first.
#[tauri::command(async)]
pub fn recent_downloads(settings: State<Store>, limit: u32) -> Result<Vec<RecentDownload>, String> {
    library::recent(&settings.library_root()?, limit)
}

/// Opens the system file manager with a library file selected. Core checks the path stays inside the library.
#[tauri::command(async)]
pub fn show_in_folder(settings: State<Store>, path: String) -> Result<(), String> {
    let file = library::locate(&settings.library_root()?, &path)?;
    tauri_plugin_opener::reveal_item_in_dir(file).map_err(|e| format!("Can't open the folder: {e}"))
}

/// Deletes a library file, or a downloaded folder with everything in it, for good. Core checks the path stays inside
/// the library and isn't one of its own folders.
#[tauri::command(async)]
pub fn delete_from_library(settings: State<Store>, path: String) -> Result<(), String> {
    library::remove(&settings.library_root()?, &path)
}

/// Opens a library file in the system's default app for it. Core checks the path stays inside the library.
#[tauri::command(async)]
pub fn open_file(settings: State<Store>, path: String) -> Result<(), String> {
    let file = library::locate(&settings.library_root()?, &path)?;
    tauri_plugin_opener::open_path(file, None::<&str>).map_err(|e| format!("Can't open the file: {e}"))
}

/// One page of the gallery, from the library folder as it is now.
#[tauri::command(async)]
pub fn gallery_page(settings: State<Store>, disk: State<Disk>, query: GalleryQuery) -> Result<GalleryPage, String> {
    library::page(&settings.library_root()?, &disk, &query)
}

/// A library file's thumbnail as a `data:` URL, made the first time it's asked for; `None` when it has no picture.
#[tauri::command]
pub async fn thumbnail(
    settings: State<'_, Store>,
    thumbnails: State<'_, Thumbnails>,
    path: String,
) -> Result<Option<String>, String> {
    thumbnails.get(&settings.library_root()?, &path).await
}

/// A library picture whole, as a `data:` URL; `None` when the window can't show it.
#[tauri::command(async)]
pub fn picture(settings: State<Store>, path: String) -> Result<Option<String>, String> {
    library::picture(&settings.library_root()?, &path)
}
