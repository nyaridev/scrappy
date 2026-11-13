//! Window: builds the main window with the frame core picked (ADR-0011), so it never shows with the wrong one,
//! minimized or hidden in the tray when the system started Scrappy (ADR-0036, ADR-0038), and brings it forward when
//! Scrappy is started a second time or the tray asks.

use scrappy_core::app::WindowFrame;
use tauri::{App, AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

const MAIN: &str = "main";

/// How the window first shows.
pub enum Shown {
    Normal,
    Minimized,
    /// Only the tray icon shows.
    Hidden,
}

/// Opens the main window with `frame`, shown as `shown` says.
pub fn open(app: &App, frame: WindowFrame, shown: Shown) -> tauri::Result<()> {
    let builder = WebviewWindowBuilder::new(app, MAIN, WebviewUrl::default())
        .title("Scrappy")
        .visible(!matches!(shown, Shown::Hidden))
        .inner_size(1200.0, 780.0)
        .min_inner_size(820.0, 560.0)
        .center();
    let builder = match frame {
        WindowFrame::Custom => builder.decorations(false),
        #[cfg(target_os = "macos")]
        WindowFrame::Overlay => builder.title_bar_style(tauri::TitleBarStyle::Overlay).hidden_title(true),
        _ => builder,
    };
    let window = builder.build()?;
    if matches!(shown, Shown::Minimized) {
        window.minimize()?;
    }
    Ok(())
}

/// Shows the main window in front: restored, unminimized and focused.
pub fn bring_forward(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN) {
        // Each step is best effort: a window that can't unminimize still gets focus.
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}
