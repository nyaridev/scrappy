//! The Tauri shell: opens the window, starts logging and the extension's listener, and registers the commands. All
//! logic is in scrappy-core.

use std::sync::Arc;

mod commands;
mod events;
mod window;

use commands::{app, dedupe, extension, grab, hunt, jobs, library, media, scrape, settings, tools};
use scrappy_core::bridge::{self, Bridge};
use scrappy_core::dedupe::Duplicates;
use scrappy_core::grab::Inbox;
use scrappy_core::jobs::Jobs;
use scrappy_core::library::Disk;
use scrappy_core::media::{Downloads, Thumbnails};
use scrappy_core::settings::Store;
use scrappy_core::tools::Toolbox;
use scrappy_core::vault::{KeySource, Vault};
use tauri::Manager;

/// Starts the app.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // First, so a second start hands over to the running app before anything else opens.
        .plugin(tauri_plugin_single_instance::init(|app, _, _| window::bring_forward(app)))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            scrappy_core::logging::init(&app.path().app_log_dir()?)?;
            let store = Store::load(&app.path().app_config_dir()?);
            let frame = scrappy_core::app::window_frame(store.get().look.native_frame);
            let toolbox = Toolbox::new(app.path().app_data_dir()?.join("tools"));
            let warm = toolbox.clone();
            // Read the versions now, so the tool list shows at once; then let yt-dlp update itself.
            tauri::async_runtime::spawn(async move {
                warm.status().await;
                warm.update_yt_dlp().await;
            });
            let vault = Vault::open(&app.path().app_data_dir()?, KeySource::Keychain);
            let downloads = Downloads::new(toolbox.clone(), vault.clone()).kept_in(&app.path().app_data_dir()?);
            let jobs = Jobs::new(events::job_reporter(app.handle().clone()));
            jobs.continue_after(downloads.last_id());
            let handle = app.handle().clone();
            // The window takes a handed page with `handed_page` when it starts and whenever it gains focus.
            let inbox = Inbox::new(Arc::new(move || window::bring_forward(&handle)));
            let extension = Bridge {
                downloads: downloads.clone(),
                jobs: jobs.clone(),
                settings: store.clone(),
                vault: vault.clone(),
                inbox: inbox.clone(),
            };
            tauri::async_runtime::spawn(bridge::serve(extension));
            app.manage(store);
            app.manage(frame);
            app.manage(downloads);
            app.manage(jobs);
            app.manage(vault);
            app.manage(inbox);
            app.manage(Duplicates::default());
            app.manage(Disk::new(events::library_watcher(app.handle().clone())));
            app.manage(Thumbnails::new(toolbox.clone()));
            app.manage(toolbox);
            window::open(app, frame)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app::app_info,
            library::library_status,
            library::pick_folder,
            library::check_folder,
            library::setup_library,
            library::library_stats,
            library::recent_downloads,
            library::show_in_folder,
            library::open_file,
            library::gallery_page,
            library::thumbnail,
            library::picture,
            settings::get_settings,
            settings::set_look,
            settings::signed_in_sites,
            settings::forget_site,
            tools::tools_status,
            tools::install_tools,
            jobs::cancel_job,
            media::probe_url,
            media::entry_thumbnail,
            media::presets,
            media::start_download,
            media::downloads,
            media::remove_download,
            media::clear_downloads,
            grab::scan_page,
            grab::grab_finds,
            grab::handed_page,
            dedupe::find_duplicates,
            dedupe::duplicates_found,
            dedupe::trash_files,
            scrape::crawl_site,
            hunt::hunt_packs,
            hunt::start_hunt,
            hunt::open_packs_folder,
            extension::host_status,
            extension::connect_browsers,
            extension::disconnect_browsers,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Scrappy");
}
