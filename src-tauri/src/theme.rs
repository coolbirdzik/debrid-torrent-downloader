//! Keeps the native title bar in the app's colors, so it reads as part of the window instead of a separate strip.
//!
//! On Windows the caption is painted through DWM: dark caption buttons, the exact caption and text color (Windows 11;
//! Windows 10 only gets the light/dark switch) and no 1px window border. Tauri's `set_theme` is not used there,
//! because forcing a window theme also forces the webview's `prefers-color-scheme`, which the "system" appearance
//! relies on. Other systems get `set_theme`, with `None` for "system".

use tauri::WebviewWindow;

/// Must match `--titlebar` in src/app.css (light and dark).
#[cfg(windows)]
const LIGHT_CAPTION: (u8, u8, u8) = (0xf5, 0xf6, 0xf8);
#[cfg(windows)]
const DARK_CAPTION: (u8, u8, u8) = (0x08, 0x09, 0x0c);
#[cfg(windows)]
const LIGHT_TEXT: (u8, u8, u8) = (0x0b, 0x0d, 0x12);
#[cfg(windows)]
const DARK_TEXT: (u8, u8, u8) = (0xee, 0xf0, 0xf5);

/// `appearance` is the saved setting: "light", "dark" or "system".
pub fn apply(window: &WebviewWindow, appearance: &str) {
    let dark = match appearance {
        "dark" => Some(true),
        "light" => Some(false),
        _ => None,
    };
    #[cfg(windows)]
    {
        // "system": the OS theme, which the window still reports because it is never forced.
        let dark = dark.unwrap_or_else(|| matches!(window.theme(), Ok(tauri::Theme::Dark)));
        paint(window, dark);
    }
    #[cfg(not(windows))]
    {
        let _ = window.set_theme(dark.map(|dark| if dark { tauri::Theme::Dark } else { tauri::Theme::Light }));
    }
}

#[cfg(windows)]
fn paint(window: &WebviewWindow, dark: bool) {
    use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE};
    /// DWMWA_COLOR_NONE: no window border at all.
    const NO_COLOR: u32 = 0xFFFF_FFFE;
    let colorref = |(r, g, b): (u8, u8, u8)| u32::from(r) | (u32::from(g) << 8) | (u32::from(b) << 16);
    let Ok(hwnd) = window.hwnd() else { return };
    let hwnd = hwnd.0 as _;
    let set = |attribute, value: u32| unsafe {
        // Failures (older Windows without the attribute) leave the default caption, which is fine.
        DwmSetWindowAttribute(hwnd, attribute as _, (&value as *const u32).cast(), std::mem::size_of::<u32>() as u32);
    };
    set(DWMWA_USE_IMMERSIVE_DARK_MODE, u32::from(dark));
    set(DWMWA_CAPTION_COLOR, colorref(if dark { DARK_CAPTION } else { LIGHT_CAPTION }));
    set(DWMWA_TEXT_COLOR, colorref(if dark { DARK_TEXT } else { LIGHT_TEXT }));
    set(DWMWA_BORDER_COLOR, NO_COLOR);
}
