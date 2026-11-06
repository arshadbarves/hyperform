//! The main window, created programmatically and always dark: macOS overlay
//! title bar with dark HUD vibrancy, Windows 11 frameless with dark Mica,
//! Linux with a native shadow. The window is transparent where a native
//! material exists, and `Material` tells the webview whether one is showing
//! so it can stop painting over it; Windows 10 and Linux have none, and the
//! webview paints its solid backdrop instead.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{App, Manager, Theme, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// Whether the native window material (vibrancy or Mica) was applied.
#[derive(Debug, Default)]
pub(crate) struct Material(AtomicBool);

impl Material {
    pub(crate) fn translucent(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

pub(crate) fn create_main(app: &App) -> tauri::Result<()> {
    let mut builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("HyperForm AI")
        .inner_size(1100.0, 760.0)
        .min_inner_size(720.0, 520.0)
        .theme(Some(Theme::Dark))
        .maximized(true);
    #[cfg(target_os = "macos")]
    {
        // Inside the sidebar card's head row (shell.css: the card is 8px in
        // from the window, its head --titlebar-h = 44px tall). tao's inset
        // keeps the buttons near the bottom of a container of height
        // button_height + y, so y is about the vertical centre: 8 + 44 / 2.
        builder = builder
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .hidden_title(true)
            .traffic_light_position(tauri::LogicalPosition::new(20.0, 30.0))
            .shadow(true)
            .transparent(true);
    }
    #[cfg(target_os = "windows")]
    {
        builder = builder.decorations(false).transparent(true);
    }
    #[cfg(target_os = "linux")]
    {
        builder = builder.shadow(true);
    }
    let window = builder.build()?;
    let translucent = apply_material(&window);
    app.state::<Material>()
        .0
        .store(translucent, Ordering::Relaxed);
    Ok(())
}

/// The dark HUD material, kept active so the window looks the same focused or not.
#[cfg(target_os = "macos")]
fn apply_material(window: &WebviewWindow) -> bool {
    use window_vibrancy::{NSVisualEffectMaterial, NSVisualEffectState, apply_vibrancy};
    apply_vibrancy(
        window,
        NSVisualEffectMaterial::HudWindow,
        Some(NSVisualEffectState::Active),
        None,
    )
    .is_ok()
}

/// Dark Mica needs Windows 11; on Windows 10 it fails and the webview paints its solid backdrop.
#[cfg(target_os = "windows")]
fn apply_material(window: &WebviewWindow) -> bool {
    window_vibrancy::apply_mica(window, Some(true)).is_ok()
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn apply_material(_window: &WebviewWindow) -> bool {
    false
}
