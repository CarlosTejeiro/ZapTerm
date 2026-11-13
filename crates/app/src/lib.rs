mod commands;
mod data_dir;
mod error;
mod events;
mod hint_watcher;
mod importers;
mod login_script;
mod menu;
mod scripting;
mod secrets;
mod state;
mod triggers;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use tauri::Manager;

pub use error::AppError;
pub use state::AppState;

#[cfg(target_os = "macos")]
fn apply_vibrancy(window: &tauri::WebviewWindow) {
    use window_vibrancy::{
        apply_vibrancy as ns_vibrancy, NSVisualEffectMaterial, NSVisualEffectState,
    };
    if let Err(e) = ns_vibrancy(
        window,
        NSVisualEffectMaterial::HudWindow,
        Some(NSVisualEffectState::Active),
        Some(8.0),
    ) {
        tracing::warn!("vibrancy failed: {e}");
    }
}

#[cfg(target_os = "windows")]
fn apply_vibrancy(window: &tauri::WebviewWindow) {
    if let Err(e) = window_vibrancy::apply_acrylic(window, Some((18, 18, 18, 180))) {
        tracing::warn!("acrylic failed: {e}");
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn apply_vibrancy(_window: &tauri::WebviewWindow) {}

/// Filename tauri-plugin-window-state persists to (in the OS config dir).
/// Its absence is how we detect the very first launch.
const WINDOW_STATE_FILE: &str = "window-state.json";

/// On first launch, size the main window to ~75% of the monitor work area
/// (clamped to the configured 900×600 minimum) and center it. No-op when a
/// saved window state exists — the plugin restores it instead.
fn size_window_on_first_run(app: &tauri::App) {
    let has_saved_state = app
        .path()
        .app_config_dir()
        .map(|d| d.join(WINDOW_STATE_FILE).exists())
        .unwrap_or(true);
    if has_saved_state {
        return;
    }
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    let Ok(Some(monitor)) = win.current_monitor() else {
        return;
    };
    let scale = monitor.scale_factor();
    let logical_w = monitor.size().width as f64 / scale;
    let logical_h = monitor.size().height as f64 / scale;
    let w = (logical_w * 0.75).max(900.0).min(logical_w);
    let h = (logical_h * 0.75).max(600.0).min(logical_h);
    let _ = win.set_size(tauri::LogicalSize::new(w, h));
    let _ = win.center();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // NOTE: the WebKitGTK DMABUF-renderer workaround that fixes high CPU on
    // Linux/NVIDIA lives in `main()` — it has to set the env var *before* the
    // webview initializes (via a one-time re-exec), which an in-process set_var
    // here is too late to do.

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("app=debug".parse().expect("valid directive")),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_filename(WINDOW_STATE_FILE)
                .build(),
        )
        .setup(|app| {
            // Native window vibrancy (translucent chrome) — best effort, and
            // purely cosmetic. It reaches into private macOS APIs (and Windows
            // DWM) that can misbehave or panic on brand-new OS builds; a panic
            // here runs inside the `did_finish_launching` ObjC callback, which
            // can't unwind and would abort the whole app. Contain it so a
            // failed effect just means opaque chrome, never a crash.
            if let Some(win) = app.get_webview_window("main") {
                let r =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| apply_vibrancy(&win)));
                if r.is_err() {
                    tracing::warn!("window vibrancy panicked; continuing without it");
                }
            }

            // First launch (no saved window state yet): open centered at
            // ~75% of the monitor work area instead of the fixed config
            // size, like SecureCRT/MobaXterm do. Subsequent launches are
            // restored by tauri-plugin-window-state. Also contained — a
            // monitor-geometry hiccup must not abort launch.
            {
                let app_ref: &tauri::App = app;
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    size_window_on_first_run(app_ref)
                }));
            }

            let resolved = data_dir::resolve(app.handle())?;
            tracing::info!(dir = %resolved.dir.display(), source = ?resolved.source, "data dir");
            let data_dir = resolved.dir.clone();
            std::fs::create_dir_all(&data_dir)?;

            // A notebook connection never outlives the app: drop a file left
            // behind by a crash so notebooks don't chase a dead port.
            let _ = std::fs::remove_file(data_dir.join("scripting").join("connection.json"));

            // First run: bring over ZAPX's sessions, settings and secrets.
            data_dir::import_from_zapx(app.handle(), &resolved);

            let db_path = data_dir.join(data_dir::DB_FILE);
            let log_dir = data_dir.join("session_logs");
            std::fs::create_dir_all(&log_dir)?;

            // Hint catalogs: bundled by default, optionally overridden by
            // `<platform>.json` files dropped into `<data_dir>/catalogs/`.
            // We pre-create the directory here so the filesystem watcher
            // (below) has something concrete to attach to from second zero;
            // the existing `hint_catalogs_dir` command still seeds it with
            // `README.md` + `*.example.json` on first user-triggered open.
            let catalogs_dir = data_dir.join("catalogs");
            core_hints::catalog::init(Some(catalogs_dir.clone()));
            let hint_watcher = hint_watcher::spawn(app.handle().clone(), catalogs_dir);

            let db = core_persistence::Database::open(&db_path)
                .map_err(|e| format!("DB init failed: {e}"))?;

            // Push the persisted SSH keepalive interval into core-transport
            // before any session can be opened.
            commands::settings::apply_persisted_keepalive(&db);

            let rules = db.list_highlight_rules().unwrap_or_default();
            let highlighter = Arc::new(RwLock::new(core_highlight::Highlighter::new(
                rules
                    .into_iter()
                    .map(|r| core_highlight::HighlightRule {
                        id: r.id,
                        name: r.name,
                        pattern: r.pattern,
                        is_regex: r.is_regex,
                        fg_color: r.fg_color,
                        bg_color: r.bg_color,
                        bold: r.bold,
                        underline: r.underline,
                        enabled: r.enabled,
                        sort_order: r.sort_order,
                    })
                    .collect(),
            )));

            // Local-fallback vault seed: a random per-install key in
            // `vault.key` (mode 0600), not the predictable data-dir path the
            // older builds used. If the keyfile didn't exist yet, this is the
            // first launch on the new scheme — migrate any secrets written
            // under the legacy seed so the user isn't re-prompted. If the
            // keyfile can't be created (read-only dir), degrade to the legacy
            // seed so credentials still resolve.
            let legacy_seed = resolved.legacy_vault_seed();
            let vault_key_existed = data_dir.join("vault.key").exists();
            let vault_seed = core_vault::load_or_create_seed(&data_dir).unwrap_or_else(|e| {
                tracing::warn!("vault keyfile unavailable ({e}); falling back to legacy seed");
                legacy_seed.clone()
            });
            if !vault_key_existed && vault_seed != legacy_seed {
                data_dir::migrate_session_secrets(&db, &legacy_seed, &vault_seed);
            }

            // The SSH library used to keep `known_hosts` under
            // `%USERPROFILE%\ssh\` on Windows; it now uses the standard
            // `.ssh\`. Fold the old file in once so the user's trusted hosts
            // don't all come back as "unknown" after the update.
            core_transport::ssh::migrate_legacy_known_hosts();

            app.manage(AppState {
                data_dir: resolved,
                sessions: Mutex::new(HashMap::new()),
                db,
                highlighter,
                loggers: Arc::new(Mutex::new(HashMap::new())),
                log_dir,
                ki_pending: Arc::new(Mutex::new(HashMap::new())),
                forwards: Mutex::new(HashMap::new()),
                password_cache: Arc::new(Mutex::new(HashMap::new())),
                vault_seed,
                sftp_cancellations: Arc::new(Mutex::new(HashMap::new())),
                hint_watcher,
                scripts: Default::default(),
            });

            // Sync folder: periodic push of local changes + notification of
            // remote ones. No-op until the user configures a folder.
            commands::sync::spawn(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::sessions::open_local_session,
            commands::sessions::create_wsl_session,
            commands::sessions::list_wsl_distros,
            commands::sessions::open_ssh_session,
            commands::sessions::ssh_preflight_host_key,
            commands::sessions::ssh_trust_host_key,
            commands::sessions::ssh_overwrite_host_key,
            commands::sessions::respond_keyboard_interactive,
            commands::forwards::add_local_forward,
            commands::forwards::add_dynamic_forward,
            commands::forwards::add_remote_forward,
            commands::forwards::list_forwards,
            commands::forwards::list_all_forwards,
            commands::forwards::remove_forward,
            commands::forwards::get_session_forwards,
            commands::forwards::set_session_forwards,
            commands::sftp::sftp_list_dir,
            commands::sftp::sftp_stat,
            commands::sftp::sftp_canonicalize,
            commands::sftp::sftp_mkdir,
            commands::sftp::sftp_remove_dir,
            commands::sftp::sftp_remove_file,
            commands::sftp::sftp_rename,
            commands::sftp::sftp_download_file,
            commands::sftp::sftp_upload_file,
            commands::sftp::sftp_cancel_transfer,
            commands::sftp::sftp_edit_file,
            commands::snippets::list_snippets,
            commands::snippets::list_snippets_for_platform,
            commands::snippets::list_recent_command_snippets,
            commands::snippets::create_snippet,
            commands::snippets::update_snippet,
            commands::snippets::set_snippets_order,
            commands::snippets::delete_snippet,
            commands::snippets::set_snippet_steps,
            commands::snippets::set_snippet_folder,
            commands::snippets::set_snippet_shortcut,
            commands::scripts::script_start,
            commands::scripts::script_reply,
            commands::scripts::script_stop,
            commands::scripts::script_python_info,
            commands::scripts::notebook_enable,
            commands::scripts::script_examples_dir,
            commands::snippets::set_snippet_positions,
            commands::snippets::export_macros,
            commands::snippets::import_macros,
            commands::vault::create_vault_entry,
            commands::vault::list_vault_entries,
            commands::vault::update_vault_entry,
            commands::vault::delete_vault_entry,
            commands::vault::send_vault_secret,
            commands::vault::send_vault_field,
            commands::login_scripts::get_login_script,
            commands::login_scripts::set_login_script,
            commands::sessions::send_input,
            commands::sessions::get_session_tcp_mss,
            commands::sessions::resize_terminal,
            commands::sessions::close_session,
            commands::sessions::create_saved_session,
            commands::sessions::cache_session_password,
            commands::sessions::forget_session_password,
            commands::sessions::session_password_source,
            commands::sessions::ssh_direct_endpoint,
            commands::sessions::update_saved_session,
            commands::sessions::set_session_credential,
            commands::sessions::move_saved_session,
            commands::sessions::reorder_saved_session,
            commands::sessions::list_sessions,
            commands::sessions::delete_saved_session,
            commands::sessions::clone_saved_session,
            commands::sessions::open_saved_session,
            commands::sessions::open_telnet_session,
            commands::sessions::open_serial_session,
            commands::sessions::list_serial_ports,
            commands::sessions::create_telnet_session,
            commands::sessions::create_serial_session,
            commands::folders::list_folders,
            commands::folders::create_folder,
            commands::folders::rename_folder,
            commands::folders::delete_folder,
            commands::groups::list_broadcast_groups,
            commands::groups::create_broadcast_group,
            commands::groups::rename_broadcast_group,
            commands::groups::delete_broadcast_group,
            commands::groups::set_broadcast_group_members,
            commands::highlight::list_highlight_rules,
            commands::highlight::create_highlight_rule,
            commands::highlight::toggle_highlight_rule,
            commands::highlight::delete_highlight_rule,
            commands::highlight::update_highlight_rule,
            commands::highlight::export_highlight_rules,
            commands::highlight::import_highlight_rules,
            commands::logging::start_session_logging,
            commands::logging::stop_session_logging,
            commands::logging::list_session_logs,
            commands::logging::list_all_session_logs,
            commands::logging::open_logs_dir,
            commands::shell::open_external,
            commands::transfer::export_sessions,
            commands::transfer::import_sessions,
            commands::transfer::import_ssh_config,
            commands::transfer::import_putty,
            commands::transfer::import_mobaxterm,
            commands::transfer::import_securecrt,
            commands::transfer::save_text_file,
            commands::backup::backup_export,
            commands::backup::backup_inspect,
            commands::backup::backup_restore,
            commands::sync::backup_sync_status,
            commands::sync::backup_sync_configure,
            commands::sync::backup_sync_disable,
            commands::sync::backup_sync_now,
            commands::sync::backup_sync_push,
            commands::sync::backup_sync_pull,
            commands::triggers::get_session_triggers,
            commands::triggers::set_session_triggers,
            commands::data_dir::get_data_dir_info,
            commands::data_dir::set_data_dir,
            commands::data_dir::reset_data_dir,
            commands::data_dir::restart_app,
            commands::settings::get_settings,
            commands::settings::get_setting,
            commands::settings::set_setting,
            commands::settings::set_ssh_keepalive,
            commands::settings::list_color_schemes,
            commands::hints::get_hints,
            commands::hints::search_command_history,
            commands::hints::record_command,
            commands::hints::set_session_platform,
            commands::hints::clear_session_platform,
            commands::hints::get_session_platform,
            commands::hints::list_platforms,
            commands::hints::clear_command_history,
            commands::hints::reload_hint_catalogs,
            commands::hints::hint_catalogs_dir,
            commands::cmdlists::read_user_cmdlists,
            commands::cmdlists::write_user_cmdlist,
            commands::cmdlists::delete_user_cmdlist,
            commands::cmdlists::open_cmdlists_dir,
            commands::hints::prompt_returned,
        ])
        .run(tauri::generate_context!())
        .expect("error while running zapterm");
}
