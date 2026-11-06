//! Desktop shell (Tauri 2) over the v2 engine: builder wiring only.
//!
//! One tokio runtime serves Tauri's async commands and the engine's tasks.
//! The main window is created programmatically (see `window.rs`), and every
//! engine event reaches the webview on the `engineEvent` channel.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod events;
mod guard;
mod ipc;
mod layers;
mod logging;
mod pet;
mod state;
mod window;
mod workspaces;

use std::time::Duration;

use anyhow::Context;
use tauri::{Manager, RunEvent};
use z_engine_config::{EnvOverrides, Paths};
use z_engine_engine::{Engine, EngineOptions};

use commands::{access, app, catalog, extensions, session, settings, update, work, workspace};
use state::AppState;
use workspaces::Workspaces;

/// Closing sessions on quit kills their shells and MCP servers.
const SHUTDOWN_LIMIT: Duration = Duration::from_secs(5);

fn main() -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("tokio runtime")?;
    let handle = runtime.handle().clone();
    tauri::async_runtime::set(handle.clone());
    let paths = Paths::discover().context("hyperform directories")?;
    logging::init(&paths.data_dir);
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "hyperform starting");

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .on_page_load(|webview, payload| {
            if matches!(payload.event(), tauri::webview::PageLoadEvent::Finished) {
                tracing::info!(webview = webview.label(), url = %payload.url(), "page loaded");
            }
        })
        .invoke_handler(tauri::generate_handler![
            session::open_session,
            session::send_command,
            session::list_sessions,
            session::delete_session,
            session::agent_transcript,
            session::export_session,
            session::inspect_request,
            session::context_breakdown,
            session::session_decisions,
            session::session_changed_files,
            session::session_diff_for_file,
            catalog::list_commands,
            catalog::list_agents,
            catalog::list_files,
            catalog::fetch_model_catalog,
            workspace::list_workspaces,
            workspace::add_workspace,
            workspace::remove_workspace,
            workspace::git_summary,
            workspace::list_changed_files,
            workspace::diff_for_file,
            workspace::create_worktree,
            workspace::open_path,
            workspace::reveal_path,
            app::app_info,
            settings::get_settings,
            settings::get_layer,
            settings::set_setting,
            settings::remove_setting,
            settings::add_permission_rule,
            settings::remove_permission_rule,
            settings::set_mcp_server,
            settings::remove_mcp_server,
            settings::test_mcp_server,
            settings::feature_catalog,
            settings::test_decision_model,
            settings::decision_model_status,
            settings::download_decision_model,
            settings::cancel_decision_model_download,
            settings::remove_decision_model,
            settings::set_hooks,
            access::credential_status,
            access::save_api_key,
            access::save_search_key,
            access::copilot_login_start,
            access::copilot_login_poll,
            access::trust_status,
            access::set_trust,
            extensions::list_extensions,
            extensions::read_extension_file,
            extensions::write_extension_file,
            extensions::delete_extension_file,
            extensions::list_instruction_files,
            extensions::write_instruction_file,
            update::check_for_update,
            update::open_release_url,
            update::install_update,
            commands::pet::pet_load,
            commands::pet::pet_save,
            work::work_plans,
            work::work_packet,
            work::work_why,
            work::work_parallel,
        ])
        .setup(move |app| {
            let _guard = handle.enter();
            let engine = Engine::new(EngineOptions {
                paths: paths.clone(),
                event_sink: events::sink(app.handle().clone()),
                client_factory: None,
                env: EnvOverrides::from_process(),
            })?;
            let workspaces = Workspaces::new(&paths.data_dir);
            let active = workspaces.initial_root(paths.home_dir.as_deref());
            tracing::info!(project = %active.display(), "engine initialized");
            let pet = pet::PetStore::new(&paths.data_dir);
            app.manage(AppState::new(engine, workspaces, pet, active));
            app.manage(window::Material::default());
            window::create_main(app)?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .context("building the HyperForm AI window")?;

    app.run(|handle, event| {
        if matches!(event, RunEvent::ExitRequested { .. }) {
            if let Some(state) = handle.try_state::<AppState>() {
                let engine = state.engine.clone();
                tauri::async_runtime::block_on(async move {
                    if tokio::time::timeout(SHUTDOWN_LIMIT, engine.shutdown())
                        .await
                        .is_err()
                    {
                        tracing::warn!("sessions did not close in time");
                    }
                });
                tracing::info!("engine shut down");
            }
        }
    });
    Ok(())
}
