//! Library commands: where the library stands, picking or checking its folder, setting it up, its numbers and recent
//! downloads, the gallery's pages, thumbnails and pictures, and opening a file or showing it in the file manager. Core
//! checks every path, typed or picked.

use scrappy_core::library::{self, FolderCheck, GalleryPage, GalleryQuery, LibraryStatus, RecentDownload, Stats};
use scrappy_core::media::Thumbnails;
use scrappy_core::settings::Store;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

/// Where the library from the settings stands.
#[tauri::command]
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
#[tauri::command]
pub fn check_folder(path: String) -> Result<FolderCheck, String> {
    library::check(&path)
}

/// Sets up the library at `path`.
#[tauri::command]
pub fn setup_library(settings: State<Store>, path: String) -> Result<LibraryStatus, String> {
    library::setup(&path, &settings)
}

/// The library's totals and the last 30 days.
#[tauri::command]
pub fn library_stats(settings: State<Store>) -> Result<Stats, String> {
    library::stats(&settings.library_root()?)
}

/// The latest downloads, newest first.
#[tauri::command]
pub fn recent_downloads(settings: State<Store>, limit: u32) -> Result<Vec<RecentDownload>, String> {
    library::recent(&settings.library_root()?, limit)
}

/// Opens the system file manager with a library file selected. Core checks the path stays inside the library.
#[tauri::command]
pub fn show_in_folder(settings: State<Store>, path: String) -> Result<(), String> {
    let file = library::locate(&settings.library_root()?, &path)?;
    tauri_plugin_opener::reveal_item_in_dir(file).map_err(|e| format!("Can't open the folder: {e}"))
}

/// Opens a library file in the system's default app for it. Core checks the path stays inside the library.
#[tauri::command]
pub fn open_file(settings: State<Store>, path: String) -> Result<(), String> {
    let file = library::locate(&settings.library_root()?, &path)?;
    tauri_plugin_opener::open_path(file, None::<&str>).map_err(|e| format!("Can't open the file: {e}"))
}

/// One page of the gallery.
#[tauri::command]
pub fn gallery_page(settings: State<Store>, query: GalleryQuery) -> Result<GalleryPage, String> {
    library::page(&settings.library_root()?, &query)
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
#[tauri::command]
pub fn picture(settings: State<Store>, path: String) -> Result<Option<String>, String> {
    library::picture(&settings.library_root()?, &path)
}
