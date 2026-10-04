//! Opt-in Windows close-to-tray; explicit Quit always exits.
use crate::app_state::{AppSettings, AppState};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{AppHandle, Manager};
static VISIBLE: AtomicBool = AtomicBool::new(false);

#[cfg(any(windows, test))]
fn labels(locale: &str) -> (&'static str, &'static str) {
    match locale {
        "zh-CN" => ("显示窗口", "退出"),
        "zh-TW" => ("顯示視窗", "結束"),
        "ja" => ("ウィンドウを表示", "終了"),
        "ko" => ("창 표시", "종료"),
        "es" => ("Mostrar ventana", "Salir"),
        "fr" => ("Afficher la fenêtre", "Quitter"),
        "de" => ("Fenster anzeigen", "Beenden"),
        "pt-BR" => ("Mostrar janela", "Sair"),
        "ru" => ("Показать окно", "Выйти"),
        _ => ("Show window", "Quit"),
    }
}
#[cfg(windows)]
fn show(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}
pub fn apply(app: &AppHandle, settings: &AppSettings) -> tauri::Result<()> {
    #[cfg(windows)]
    {
        use tauri::{
            menu::{Menu, MenuItem},
            tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
        };
        let (show_text, quit_text) = labels(&settings.language);
        if !settings.close_to_tray && app.tray_by_id("usage").is_none() {
            return Ok(());
        }
        let show_item = MenuItem::with_id(app, "show", show_text, true, None::<&str>)?;
        let quit_item = MenuItem::with_id(app, "quit", quit_text, true, None::<&str>)?;
        let menu = Menu::with_items(app, &[&show_item, &quit_item])?;
        let tray = if let Some(tray) = app.tray_by_id("usage") {
            tray.set_menu(Some(menu))?;
            tray
        } else {
            let mut builder = TrayIconBuilder::with_id("usage")
                .menu(&menu)
                .tooltip("LLM Usage")
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if matches!(
                        event,
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        }
                    ) {
                        show(tray.app_handle());
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                builder = builder.icon(icon.clone());
            }
            builder.build(app)?
        };
        tray.set_visible(settings.close_to_tray)?;
        VISIBLE.store(settings.close_to_tray, Ordering::SeqCst);
        if !settings.close_to_tray {
            show(app);
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (app, settings);
        Ok(())
    }
}
pub fn close_requested(window: &tauri::Window, event: &tauri::WindowEvent) {
    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
        let enabled = window
            .state::<Arc<AppState>>()
            .settings
            .lock()
            .unwrap()
            .close_to_tray;
        if enabled && VISIBLE.load(Ordering::SeqCst) && window.hide().is_ok() {
            api.prevent_close();
        }
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn menu_labels_cover_every_supported_locale() {
        for locale in [
            "zh-CN", "zh-TW", "en", "ja", "ko", "es", "fr", "de", "pt-BR", "ru",
        ] {
            let (show, quit) = super::labels(locale);
            assert!(!show.is_empty() && !quit.is_empty());
        }
    }
}
