mod keychain;
mod usage;
mod icon;

use std::sync::Arc;
use tauri::{
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, State, WebviewWindow,
};
use tokio::sync::RwLock;
use usage::UsageData;

/// Shared app state: cached usage + OAuth token.
pub struct AppState {
    pub token: RwLock<Option<String>>,
    pub cached: RwLock<Option<UsageData>>,
}

/// Poll interval in seconds.
const POLL_SECS: u64 = 180; // 3 minutes

// ── Tauri commands (called from Svelte) ──────────────────────

#[tauri::command]
async fn get_usage(state: State<'_, Arc<AppState>>) -> Result<UsageData, String> {
    if let Some(ref data) = *state.cached.read().await {
        return Ok(data.clone());
    }
    refresh_usage(state).await
}

#[tauri::command]
async fn refresh_usage(state: State<'_, Arc<AppState>>) -> Result<UsageData, String> {
    let token = ensure_token(&state).await?;
    let data = usage::fetch_usage(&token).await?;
    *state.cached.write().await = Some(data.clone());
    Ok(data)
}

async fn ensure_token(state: &AppState) -> Result<String, String> {
    {
        let guard = state.token.read().await;
        if let Some(ref t) = *guard {
            return Ok(t.clone());
        }
    }
    let t = keychain::get_oauth_token()?;
    *state.token.write().await = Some(t.clone());
    Ok(t)
}

// ── Popover positioning ──────────────────────────────────────

fn toggle_popover(window: &WebviewWindow, x: f64, y: f64) {
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }

    let w = 290.0_f64;
    let left = (x - w / 2.0).max(0.0);
    let top = y + 4.0;

    let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition {
        x: left,
        y: top,
    }));
    let _ = window.show();
    let _ = window.set_focus();
}

// ── Background polling loop ──────────────────────────────────

fn start_polling(app_handle: tauri::AppHandle, state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(POLL_SECS));

        loop {
            interval.tick().await;

            let token = match ensure_token(&state).await {
                Ok(t) => t,
                Err(e) => {
                    log::error!("Token error: {e}");
                    let _ = app_handle.emit("usage-error", &e);
                    continue;
                }
            };

            match usage::fetch_usage(&token).await {
                Ok(data) => {
                    // Regenerate tray icon based on utilisation
                    if let Some(bucket) = &data.five_hour {
                        let new_icon = icon::generate_usage_icon(bucket.utilisation);
                        if let Some(tray) = app_handle.tray_by_id("cspy-tray") {
                            let _ = tray.set_icon(Some(new_icon));
                        }
                    }

                    update_tray_tooltip(&app_handle, &data);
                    *state.cached.write().await = Some(data.clone());
                    let _ = app_handle.emit("usage-updated", &data);
                }
                Err(e) => {
                    log::error!("Poll failed: {e}");
                    let _ = app_handle.emit("usage-error", &e);
                }
            }
        }
    });
}

fn update_tray_tooltip(app: &tauri::AppHandle, data: &UsageData) {
    let h5 = data
        .five_hour
        .as_ref()
        .map(|b| format!("{}%", (b.utilisation * 100.0) as u32))
        .unwrap_or_else(|| "—".into());
    let d7 = data
        .seven_day
        .as_ref()
        .map(|b| format!("{}%", (b.utilisation * 100.0) as u32))
        .unwrap_or_else(|| "—".into());

    let tip = format!("CSpy — 5h: {h5} | 7d: {d7}");

    if let Some(tray) = app.tray_by_id("cspy-tray") {
        let _ = tray.set_tooltip(Some(&tip));
    }
}

// ── App entry ────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = Arc::new(AppState {
        token: RwLock::new(None),
        cached: RwLock::new(None),
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![get_usage, refresh_usage])
        .setup(move |app| {
            // Hide from Dock — menu bar only app
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let handle = app.handle().clone();

            // Build system tray
            TrayIconBuilder::with_id("cspy-tray")
                .tooltip("CSpy — loading…")
                .icon(icon::generate_usage_icon(0.0))
                .icon_as_template(false)
                .on_tray_icon_event(move |tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        position,
                        ..
                    } = event
                    {
                        if let Some(win) = tray.app_handle().get_webview_window("popover") {
                            toggle_popover(&win, position.x, position.y);
                        }
                    }
                })
                .build(app)?;

            // Start background polling
            start_polling(handle, state.clone());

            // Immediate first fetch
            let s = state.clone();
            let h = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                match ensure_token(&s).await {
                    Ok(token) => match usage::fetch_usage(&token).await {
                        Ok(data) => {
                            // Regenerate tray icon
                            if let Some(bucket) = &data.five_hour {
                                let new_icon = icon::generate_usage_icon(bucket.utilisation);
                                if let Some(tray) = h.tray_by_id("cspy-tray") {
                                    let _ = tray.set_icon(Some(new_icon));
                                }
                            }

                            update_tray_tooltip(&h, &data);
                            *s.cached.write().await = Some(data.clone());
                            let _ = h.emit("usage-updated", &data);
                        }
                        Err(e) => {
                            log::error!("Initial fetch failed: {e}");
                            let _ = h.emit("usage-error", &e);
                        }
                    },
                    Err(e) => {
                        log::error!("Keychain error on startup: {e}");
                        let _ = h.emit("usage-error", &e);
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running CSpy");
}
