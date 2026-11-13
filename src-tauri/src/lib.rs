//! The Tauri shell: opens the window and the tray, starts logging, the extension's listener and the update checks, and
//! registers the commands. All logic is in scrappy-core.

use std::sync::Arc;

mod autostart;
mod commands;
mod events;
mod protocol;
mod tray;
mod update;
mod window;

use commands::{app, dedupe, extension, grab, hunt, jobs, library, media, scrape, settings, tools};
use scrappy_core::bridge::{self, Bridge};
use scrappy_core::crawl::Mirrors;
use scrappy_core::dedupe::Duplicates;
use scrappy_core::grab::{Inbox, Scans};
use scrappy_core::hunt::Hunts;
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
        .plugin(autostart::plugin())
        .plugin(tauri_plugin_updater::Builder::new().build())
        // The Gallery player's videos and audio, a range at a time, off the window's thread (ADR-0023).
        .register_asynchronous_uri_scheme_protocol("scrappy", |context, request, responder| {
            let app = context.app_handle().clone();
            tauri::async_runtime::spawn_blocking(move || responder.respond(protocol::media(&app, &request)));
        })
        .setup(|app| {
            scrappy_core::logging::init(&app.path().app_log_dir()?)?;
            let store = Store::load(&app.path().app_config_dir()?);
            scrappy_core::app::apply(&store.get());
            let frame = scrappy_core::app::window_frame(store.get().look.native_frame);
            let toolbox = Toolbox::new(app.path().app_data_dir()?.join("tools"));
            let (warm, update) = (toolbox.clone(), store.get().preferences.update_yt_dlp);
            // Read the versions now, so the tool list shows at once; then let yt-dlp update itself, unless the user
            // turned that off (ADR-0034).
            tauri::async_runtime::spawn(async move {
                warm.status().await;
                if update {
                    warm.update_yt_dlp().await;
                }
            });
            let vault = Vault::open(&app.path().app_data_dir()?, KeySource::Keychain);
            let downloads = Downloads::new(toolbox.clone(), vault.clone())
                .kept_in(&app.path().app_data_dir()?)
                .limited(&store.get().downloads);
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
            app.manage(Scans::default());
            app.manage(Mirrors::default());
            app.manage(Hunts::default());
            app.manage(Disk::new(events::library_watcher(app.handle().clone())));
            app.manage(Thumbnails::new(toolbox.clone()));
            app.manage(toolbox);
            app.manage(update::Updates::default());
            autostart::restore(app.handle(), &app.state::<Store>());
            update::start_checks(app.handle().clone());
            tray::add(app)?;
            let shown = match (autostart::started_minimized(), tray::closes_to_tray(app.handle())) {
                (false, _) => window::Shown::Normal,
                (true, true) => window::Shown::Hidden,
                (true, false) => window::Shown::Minimized,
            };
            window::open(app, frame, shown)?;
            Ok(())
        })
        .on_window_event(tray::on_window_event)
        .invoke_handler(tauri::generate_handler![
            app::app_info,
            app::update_status,
            app::check_for_update,
            app::install_update,
            library::library_status,
            library::pick_folder,
            library::check_folder,
            library::setup_library,
            library::move_library,
            library::library_stats,
            library::recent_downloads,
            library::show_in_folder,
            library::open_file,
            library::delete_from_library,
            library::gallery_page,
            library::thumbnail,
            library::picture,
            settings::get_settings,
            settings::set_look,
            settings::set_download_limits,
            settings::set_preferences,
            settings::name_example,
            settings::set_player,
            settings::set_hidden_packs,
            settings::signed_in_sites,
            settings::forget_site,
            tools::tools_status,
            tools::install_tools,
            jobs::cancel_job,
            media::list_link,
            media::listed_link,
            media::entry_thumbnail,
            media::download_size,
            media::presets,
            media::start_download,
            media::downloads,
            media::remove_download,
            media::clear_downloads,
            grab::start_scan,
            grab::scanned_page,
            grab::find_thumbnail,
            grab::grab_finds,
            grab::handed_page,
            dedupe::find_duplicates,
            dedupe::duplicates_found,
            dedupe::trash_files,
            scrape::crawl_site,
            scrape::mirror_media,
            scrape::save_mirror_media,
            hunt::hunt_packs,
            hunt::save_pack,
            hunt::create_pack,
            hunt::remove_pack,
            hunt::start_hunt,
            hunt::hunt_status,
            hunt::open_packs_folder,
            extension::host_status,
            extension::connect_browsers,
            extension::disconnect_browsers,
            extension::extension_files,
            extension::open_extension_files,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Scrappy");
}
