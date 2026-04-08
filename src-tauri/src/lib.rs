mod keychain;
pub mod usage;
mod icon;

use std::sync::Arc;
use chrono::Timelike;
use tauri::{
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, State, WebviewWindow,
};
use tokio::sync::RwLock;
use usage::UsageData;

/// Shared app state: cached usage, OAuth token, and HTTP client.
pub struct AppState {
    pub token: RwLock<Option<String>>,
    /// Token expiry as millisecond Unix timestamp (None = unknown / token-file source).
    pub token_expires_at_ms: RwLock<Option<i64>>,
    pub cached: RwLock<Option<UsageData>>,
    pub client: reqwest::Client,
    /// Set to true when an update has been downloaded and is ready to install on restart.
    pub update_pending: RwLock<bool>,
}

/// Poll interval in seconds.
const POLL_SECS: u64 = 180; // 3 minutes

/// Maximum backoff ceiling in seconds (30 minutes).
const MAX_BACKOFF_SECS: u64 = 1800;

/// Update check interval in seconds (30 minutes).
const UPDATE_CHECK_SECS: u64 = 1800;

/// Seconds after startup before the watchdog begins checking heartbeats.
const HEARTBEAT_GRACE_SECS: u64 = 15;

/// Seconds without a heartbeat before the frontend is considered unhealthy.
const HEARTBEAT_THRESHOLD_SECS: u64 = 90;

/// Watchdog tick interval in seconds.
const WATCHDOG_TICK_SECS: u64 = 60;

// Compile-time sanity check: threshold must exceed tick interval or the
// watchdog can miss a failure that recovers between ticks.
const _: () = assert!(
    HEARTBEAT_THRESHOLD_SECS > WATCHDOG_TICK_SECS,
    "HEARTBEAT_THRESHOLD_SECS must exceed WATCHDOG_TICK_SECS"
);

// ── Tauri commands (called from Svelte) ──────────────────────

#[tauri::command]
async fn get_usage(state: State<'_, Arc<AppState>>) -> Result<UsageData, String> {
    if let Some(ref data) = *state.cached.read().await {
        return Ok(data.clone());
    }
    Err("No cached data yet — waiting for first poll".into())
}

#[tauri::command]
async fn refresh_usage(state: State<'_, Arc<AppState>>) -> Result<UsageData, String> {
    log::info!("refresh_usage called from frontend");
    let token = ensure_token(&state).await?;
    match usage::fetch_usage(&state.client, &token).await {
        Ok(data) => {
            *state.cached.write().await = Some(data.clone());
            Ok(data)
        }
        Err(e) if e == "token_expired" => {
            log::warn!("refresh_usage: token expired — clearing cache");
            *state.token.write().await = None;
            *state.token_expires_at_ms.write().await = None;
            Err("Token expired — will re-read from Keychain on next attempt".into())
        }
        Err(e) if e.starts_with("rate_limited:") => {
            // Return cached data on 429 — don't surface as error
            if let Some(ref data) = *state.cached.read().await {
                Ok(data.clone())
            } else {
                Err("Rate limited and no cached data available".into())
            }
        }
        Err(e) => Err(e),
    }
}

/// Check whether the cached token is expired (with 60s buffer).
async fn is_token_expired(state: &AppState) -> bool {
    let guard = state.token_expires_at_ms.read().await;
    match *guard {
        Some(expires_ms) => {
            let now_ms = chrono::Utc::now().timestamp_millis();
            // Treat as expired if within 60s of expiry — no point sending a token
            // that'll die before the response comes back
            expires_ms - now_ms < 60_000
        }
        None => false, // unknown expiry (token file) — let the API decide
    }
}

async fn ensure_token(state: &AppState) -> Result<String, String> {
    // If we have a cached token, check its expiry first
    {
        let guard = state.token.read().await;
        if guard.is_some() {
            if is_token_expired(state).await {
                log::warn!("Cached token expired — clearing, will re-read from source");
                drop(guard);
                *state.token.write().await = None;
                *state.token_expires_at_ms.write().await = None;
            } else {
                return Ok(guard.as_ref().unwrap().clone());
            }
        }
    }

    let info = keychain::get_oauth_token()?;

    // Check if the freshly-read token is already expired
    if let Some(expires_ms) = info.expires_at_ms {
        let now_ms = chrono::Utc::now().timestamp_millis();
        if expires_ms - now_ms < 60_000 {
            let expires_ago = (now_ms - expires_ms) / 1000;
            return Err(format!(
                "Token from Keychain is already expired ({expires_ago}s ago) — open Claude Code to refresh"
            ));
        }
    }

    *state.token.write().await = Some(info.token.clone());
    *state.token_expires_at_ms.write().await = info.expires_at_ms;
    Ok(info.token)
}

// ── Popover positioning ──────────────────────────────────────

fn toggle_popover(window: &WebviewWindow, x: f64, y: f64) {
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }

    // Tray click coords are physical — convert to logical once and stay logical
    let scale = window.current_monitor()
        .ok()
        .flatten()
        .map(|m| m.scale_factor())
        .unwrap_or(2.0);

    let lx = x / scale;
    let ly = y / scale;
    let w = 290.0_f64;

    let left = (lx - w / 2.0).max(0.0);
    let top = ly + 4.0;

    let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition {
        x: left,
        y: top,
    }));
    let _ = window.show();
    let _ = window.set_focus();
}

// ── Background polling loop ──────────────────────────────────

/// Returns true if the given hour (0–23) falls within quiet hours (23:00–08:00).
fn is_quiet_hours_at(hour: u32) -> bool {
    !(8..23).contains(&hour)
}

/// Returns true if current local time is within quiet hours (23:00–08:00).
fn is_quiet_hours() -> bool {
    is_quiet_hours_at(chrono::Local::now().hour())
}

/// Compute backoff sleep: after 2+ consecutive errors, double each time up to MAX_BACKOFF_SECS.
/// 0–1 errors → POLL_SECS, 2 → 360s, 3 → 720s, 4 → 1440s, 5 → 1800s (capped).
fn backoff_sleep(consecutive_errors: u32) -> u64 {
    if consecutive_errors < 2 {
        return POLL_SECS;
    }
    let multiplier = 1u64 << (consecutive_errors - 1).min(5);
    (POLL_SECS * multiplier).min(MAX_BACKOFF_SECS)
}

/// Returns true if the frontend is healthy.
/// Pure function — no I/O, easy to test.
///
/// `last_heartbeat`: when the last heartbeat was received (`None` = never)
/// `startup`: when the app started
/// `now`: current time (pass `Instant::now()` in production)
/// `grace_secs`: seconds after startup before checking
/// `threshold_secs`: seconds without a heartbeat before unhealthy
fn is_frontend_healthy(
    last_heartbeat: Option<std::time::Instant>,
    startup: std::time::Instant,
    now: std::time::Instant,
    grace_secs: u64,
    threshold_secs: u64,
) -> bool {
    // Still in startup grace period — don't alert on slow WebView loads
    if now.duration_since(startup).as_secs() < grace_secs {
        return true;
    }
    match last_heartbeat {
        None => false, // past grace period, no heartbeat ever received
        Some(last) => now.duration_since(last).as_secs() < threshold_secs,
    }
}

fn start_polling(app_handle: tauri::AppHandle, state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        // Sleep first — the immediate fetch in setup handles the first request
        tokio::time::sleep(std::time::Duration::from_secs(POLL_SECS)).await;

        let mut consecutive_errors: u32 = 0;

        loop {
            let mut next_sleep = POLL_SECS;

            if is_quiet_hours() {
                // If an update was downloaded, restart now (user is asleep)
                if *state.update_pending.read().await {
                    log::info!("Quiet hours + update pending — restarting to apply update");
                    app_handle.restart();
                }
                log::info!("Quiet hours (23:00–08:00) — skipping poll");
            } else {
                let token = match ensure_token(&state).await {
                    Ok(t) => t,
                    Err(e) => {
                        consecutive_errors += 1;
                        next_sleep = backoff_sleep(consecutive_errors);
                        log::error!("Token error (attempt {}): {e} — next poll in {next_sleep}s",
                            consecutive_errors);
                        let _ = app_handle.emit("usage-error", &e);
                        tokio::time::sleep(std::time::Duration::from_secs(next_sleep)).await;
                        continue;
                    }
                };

                match usage::fetch_usage(&state.client, &token).await {
                    Ok(data) => {
                        if consecutive_errors > 0 {
                            log::info!("Poll succeeded after {} consecutive error(s) — backoff reset",
                                consecutive_errors);
                        }
                        consecutive_errors = 0;

                        // Regenerate tray icon based on utilisation
                        if let Some(bucket) = &data.five_hour {
                            let new_icon = icon::generate_usage_icon(bucket.utilisation);
                            if let Some(tray) = app_handle.tray_by_id("cspy-tray") {
                                let _ = tray.set_icon(Some(new_icon));
                            }
                        }

                        update_tray_tooltip(&app_handle, &data);
                        update_tray_title(&app_handle, &data);
                        *state.cached.write().await = Some(data.clone());
                        let _ = app_handle.emit("usage-updated", &data);
                    }
                    Err(e) if e == "token_expired" => {
                        consecutive_errors += 1;
                        *state.token.write().await = None;
                        *state.token_expires_at_ms.write().await = None;
                        next_sleep = backoff_sleep(consecutive_errors);
                        log::warn!("Token expired (attempt {}) — cleared cache, next poll in {next_sleep}s",
                            consecutive_errors);

                        if consecutive_errors >= 2 {
                            let _ = app_handle.emit("usage-error",
                                "Token expired — open Claude Code to refresh it");
                        }
                    }
                    Err(e) if e.starts_with("rate_limited:") => {
                        consecutive_errors += 1;
                        let retry_after: u64 = e.trim_start_matches("rate_limited:")
                            .parse()
                            .unwrap_or(0);
                        // Respect Retry-After OR backoff, whichever is longer
                        next_sleep = retry_after
                            .max(backoff_sleep(consecutive_errors))
                            .min(MAX_BACKOFF_SECS);
                        log::info!("Rate limited (attempt {}) — will retry in {next_sleep}s",
                            consecutive_errors);
                    }
                    Err(e) => {
                        consecutive_errors += 1;
                        next_sleep = backoff_sleep(consecutive_errors);
                        log::error!("Poll failed (attempt {}): {e} — next poll in {next_sleep}s",
                            consecutive_errors);
                        let _ = app_handle.emit("usage-error", &e);
                    }
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(next_sleep)).await;
        }
    });
}

/// Tick every 60 seconds to update the tray countdown without a fresh API fetch.
/// Also resets the icon to 0% when the five-hour window has expired (e.g. during quiet hours).
fn start_countdown_ticker(app_handle: tauri::AppHandle, state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            if let Some(data) = state.cached.read().await.as_ref() {
                update_tray_title(&app_handle, data);

                // If the five-hour window has expired, reset the icon to 0%
                if let Some(bucket) = &data.five_hour {
                    if is_window_expired(bucket) {
                        let empty_icon = icon::generate_usage_icon(0.0);
                        if let Some(tray) = app_handle.tray_by_id("cspy-tray") {
                            let _ = tray.set_icon(Some(empty_icon));
                        }
                    }
                }
            }
        }
    });
}

fn start_update_checker(app_handle: tauri::AppHandle, state: Arc<AppState>) {
    use tauri_plugin_updater::UpdaterExt;

    tauri::async_runtime::spawn(async move {
        // Wait 60s before first check — let the app settle after launch
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;

        loop {
            log::debug!("Checking for updates...");

            match app_handle.updater_builder().build() {
                Ok(updater) => match updater.check().await {
                    Ok(Some(update)) => {
                        log::info!("Update available: v{}", update.version);
                        match update.download_and_install(|_, _| {}, || {}).await {
                            Ok(()) => {
                                log::info!("Update v{} downloaded and staged", update.version);
                                *state.update_pending.write().await = true;

                                // Notify user via macOS notification
                                let msg = format!(
                                    "CSpy v{} downloaded — will update tonight",
                                    update.version
                                );
                                use tauri_plugin_notification::NotificationExt;
                                let _ = app_handle
                                    .notification()
                                    .builder()
                                    .title("CSpy Update Ready")
                                    .body(&msg)
                                    .show();
                                log::info!("Notification sent: {msg}");

                                // If already in quiet hours, restart immediately
                                if is_quiet_hours() {
                                    log::info!(
                                        "Already in quiet hours — restarting to apply update"
                                    );
                                    app_handle.restart();
                                }
                            }
                            Err(e) => {
                                log::error!("Failed to download/install update: {e}");
                            }
                        }
                    }
                    Ok(None) => {
                        log::debug!("No updates available");
                    }
                    Err(e) => {
                        log::warn!("Update check failed: {e}");
                    }
                },
                Err(e) => {
                    log::error!("Failed to build updater: {e}");
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(UPDATE_CHECK_SECS)).await;
        }
    });
}

/// Returns true if the bucket's five-hour window has expired (resets_at is in the past).
fn is_window_expired(bucket: &usage::UsageBucket) -> bool {
    bucket.resets_at.as_deref()
        .and_then(|r| chrono::DateTime::parse_from_rfc3339(r).ok())
        .map(|reset| reset < chrono::Utc::now())
        .unwrap_or(false)
}

/// Format remaining time until `resets_at` as "Xh Ym", "Ym", or "0m".
fn format_countdown(resets_at: &str) -> String {
    let Ok(reset) = chrono::DateTime::parse_from_rfc3339(resets_at) else {
        return "—".into();
    };
    let total_mins = reset.signed_duration_since(chrono::Utc::now()).num_minutes();
    if total_mins <= 0 {
        return "—".into();
    }
    let hours = total_mins / 60;
    let mins = total_mins % 60;
    if hours > 0 {
        format!("{hours}h {mins}m")
    } else {
        format!("{mins}m")
    }
}

/// Set the tray icon title (text to the right of the icon) to the 5-hour countdown.
fn update_tray_title(app: &tauri::AppHandle, data: &UsageData) {
    let label = data.five_hour.as_ref()
        .and_then(|b| b.resets_at.as_deref())
        .map(format_countdown)
        .unwrap_or_else(|| "—".into());
    if let Some(tray) = app.tray_by_id("cspy-tray") {
        let _ = tray.set_title(Some(label.as_str()));
    }
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
    let client = usage::build_client().expect("Failed to build HTTP client");

    let state = Arc::new(AppState {
        token: RwLock::new(None),
        token_expires_at_ms: RwLock::new(None),
        cached: RwLock::new(None),
        client,
        update_pending: RwLock::new(false),
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
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
                        log::info!("Tray clicked at ({}, {})", position.x, position.y);
                        match tray.app_handle().get_webview_window("popover") {
                            Some(win) => {
                                log::info!("Found popover window, toggling");
                                toggle_popover(&win, position.x, position.y);
                            }
                            None => {
                                log::error!("Popover window not found!");
                            }
                        }
                    }
                })
                .build(app)?;

            // Start background polling
            start_polling(handle, state.clone());

            // Start 1-minute countdown ticker for tray title
            start_countdown_ticker(app.handle().clone(), state.clone());

            // Start background update checker
            start_update_checker(app.handle().clone(), state.clone());

            // Immediate first fetch
            let s = state.clone();
            let h = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                match ensure_token(&s).await {
                    Ok(token) => match usage::fetch_usage(&s.client, &token).await {
                        Ok(data) => {
                            // Regenerate tray icon
                            if let Some(bucket) = &data.five_hour {
                                let new_icon = icon::generate_usage_icon(bucket.utilisation);
                                if let Some(tray) = h.tray_by_id("cspy-tray") {
                                    let _ = tray.set_icon(Some(new_icon));
                                }
                            }

                            update_tray_tooltip(&h, &data);
                            update_tray_title(&h, &data);
                            *s.cached.write().await = Some(data.clone());
                            let _ = h.emit("usage-updated", &data);
                        }
                        Err(e) if e == "token_expired" => {
                            log::warn!("Initial fetch: token expired — clearing cache");
                            *s.token.write().await = None;
                            *s.token_expires_at_ms.write().await = None;
                            let _ = h.emit("usage-error", "Token expired — will retry on next poll");
                        }
                        Err(e) if e.starts_with("rate_limited:") => {
                            log::info!("Initial fetch rate limited — will retry on next poll");
                        }
                        Err(e) => {
                            log::error!("Initial fetch failed: {e}");
                            let _ = h.emit("usage-error", &e);
                        }
                    },
                    Err(e) => {
                        log::error!("Token error on startup: {e}");
                        let _ = h.emit("usage-error", &e);
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running CSpy");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn countdown_future_hours_and_mins() {
        let reset = (chrono::Utc::now() + chrono::Duration::minutes(150)).to_rfc3339();
        let result = format_countdown(&reset);
        assert!(result.starts_with("2h "), "expected '2h Xm', got '{result}'");
    }

    #[test]
    fn countdown_future_mins_only() {
        let reset = (chrono::Utc::now() + chrono::Duration::minutes(42)).to_rfc3339();
        let result = format_countdown(&reset);
        // Allow 1m tolerance due to timing variations
        assert!(result == "42m" || result == "41m", "expected '41m' or '42m', got '{result}'");
    }

    #[test]
    fn countdown_past_returns_dash() {
        let reset = (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        assert_eq!(format_countdown(&reset), "\u{2014}");
    }

    #[test]
    fn countdown_unparseable_returns_dash() {
        assert_eq!(format_countdown("not-a-date"), "\u{2014}");
    }

    #[test]
    fn window_expired_past_resets_at() {
        let bucket = usage::UsageBucket {
            utilisation: 0.5,
            resets_at: Some((chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339()),
        };
        assert!(is_window_expired(&bucket));
    }

    #[test]
    fn window_not_expired_future_resets_at() {
        let bucket = usage::UsageBucket {
            utilisation: 0.5,
            resets_at: Some((chrono::Utc::now() + chrono::Duration::hours(3)).to_rfc3339()),
        };
        assert!(!is_window_expired(&bucket));
    }

    #[test]
    fn window_not_expired_none_resets_at() {
        let bucket = usage::UsageBucket {
            utilisation: 0.5,
            resets_at: None,
        };
        assert!(!is_window_expired(&bucket));
    }

    #[test]
    fn quiet_hours_boundaries() {
        assert!(!is_quiet_hours_at(22), "22:00 should NOT be quiet");
        assert!(is_quiet_hours_at(23),  "23:00 should be quiet");
        assert!(is_quiet_hours_at(0),   "00:00 should be quiet");
        assert!(is_quiet_hours_at(7),   "07:00 should be quiet");
        assert!(!is_quiet_hours_at(8),  "08:00 should NOT be quiet");
        assert!(!is_quiet_hours_at(12), "12:00 should NOT be quiet");
    }

    #[test]
    fn backoff_zero_errors_returns_poll_secs() {
        assert_eq!(backoff_sleep(0), POLL_SECS);
    }

    #[test]
    fn backoff_one_error_returns_poll_secs() {
        assert_eq!(backoff_sleep(1), POLL_SECS);
    }

    #[test]
    fn backoff_two_errors_doubles() {
        assert_eq!(backoff_sleep(2), POLL_SECS * 2);
    }

    #[test]
    fn backoff_three_errors_quadruples() {
        assert_eq!(backoff_sleep(3), POLL_SECS * 4);
    }

    #[test]
    fn backoff_capped_at_max() {
        assert_eq!(backoff_sleep(10), MAX_BACKOFF_SECS);
    }

    #[test]
    fn heartbeat_healthy_within_threshold() {
        let now = std::time::Instant::now();
        let startup = now - std::time::Duration::from_secs(60);
        let last_beat = Some(now - std::time::Duration::from_secs(60));
        assert!(is_frontend_healthy(last_beat, startup, now, 15, 90));
    }

    #[test]
    fn heartbeat_unhealthy_beyond_threshold() {
        let now = std::time::Instant::now();
        let startup = now - std::time::Duration::from_secs(200);
        let last_beat = Some(now - std::time::Duration::from_secs(100));
        assert!(!is_frontend_healthy(last_beat, startup, now, 15, 90));
    }

    #[test]
    fn heartbeat_healthy_during_grace_period() {
        let now = std::time::Instant::now();
        let startup = now - std::time::Duration::from_secs(10); // 10s < 15s grace
        let last_beat = None; // no heartbeat yet
        assert!(is_frontend_healthy(last_beat, startup, now, 15, 90));
    }

    #[test]
    fn heartbeat_none_after_grace_period() {
        let now = std::time::Instant::now();
        let startup = now - std::time::Duration::from_secs(20); // 20s > 15s grace
        let last_beat = None; // no heartbeat ever received
        assert!(!is_frontend_healthy(last_beat, startup, now, 15, 90));
    }
}
