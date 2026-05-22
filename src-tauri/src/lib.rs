//! Captain Agent — AI Agent behavior audit tool.
//!
//! User-mode Tauri app. Module map (matches design spec):
//!   - bus:           ③ broadcast channel (local to this process)
//!   - helper_client: connects to captain-helper UDS, drains events into bus
//!   - target:        ① Target Manager (CRUD + monitored PID set + filter)
//!   - rule:          ④ Rule engine (Slice 2)
//!   - store:         ⑤ SQLite
//!   - api:           ⑥ Tauri commands + event push
//!   - notify:        ⑧ OS notifications (Slice 4)

pub mod api;
pub mod bus;
pub mod helper_client;
pub mod notify;
pub mod rule;
pub mod store;
pub mod target;

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::Manager;

use crate::bus::Bus;
use crate::helper_client::HelperClient;
use crate::target::TargetManager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(
                    "captain_agent_lib=info",
                )),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_handle = app.handle().clone();
            let bus = Bus::new();
            let helper_connected = Arc::new(AtomicBool::new(false));

            app.manage(bus.clone());
            app.manage(helper_connected.clone());

            let data_dir = app
                .path()
                .app_data_dir()
                .context("could not resolve app data dir")?;
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("captain.sqlite");

            let bus_for_boot = bus.clone();
            let app_handle_for_boot = app_handle.clone();
            let helper_connected_boot = helper_connected.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = boot_pipeline(
                    db_path,
                    bus_for_boot,
                    app_handle_for_boot,
                    helper_connected_boot,
                )
                .await
                {
                    tracing::error!(error = ?e, "pipeline boot failed");
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            api::recent_events,
            api::status,
            api::list_findings,
            api::update_finding_status,
            api::list_targets,
            api::add_target,
            api::remove_target,
            api::toggle_target,
            api::dashboard_summary,
            api::export_report,
            api::get_data_stats,
            api::clear_old_events,
            api::clear_all_data,
            api::list_all_rules,
            api::upsert_user_rule,
            api::delete_user_rule,
            api::set_rule_enabled,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn boot_pipeline(
    db_path: PathBuf,
    bus: Bus,
    app_handle: tauri::AppHandle,
    helper_connected: Arc<AtomicBool>,
) -> Result<()> {
    // ⑤ Store
    let pool = store::open(&db_path).await?;
    app_handle.manage(pool.clone());
    tracing::info!(db = %db_path.display(), "opened captain.sqlite");

    // ① Target Manager (loads existing targets from SQLite).
    let target_manager = Arc::new(TargetManager::new(pool.clone()).await?);
    app_handle.manage(target_manager.clone());

    // Batch writer (persists bus events, filtered by TM).
    {
        let pool = pool.clone();
        let bus = bus.clone();
        let tm = target_manager.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = store::batch_writer::run(pool, bus, tm).await {
                tracing::error!(error = %e, "batch writer crashed");
            }
        });
    }

    // UI pusher (forwards bus events to the webview, filtered by TM).
    {
        let bus = bus.clone();
        let app_handle = app_handle.clone();
        let tm = target_manager.clone();
        tauri::async_runtime::spawn(async move {
            api::push_events_to_ui(bus, app_handle, tm).await;
        });
    }

    // Helper client (drains captain-helper's event stream into our bus).
    let helper_socket = HelperClient::resolve_socket();
    let helper = HelperClient {
        socket_path: helper_socket.clone(),
        bus: bus.clone(),
        connected: helper_connected,
    };
    tracing::info!(
        socket = %helper.socket_path.display(),
        "starting helper client"
    );
    tauri::async_runtime::spawn(async move {
        helper.run().await;
    });

    // Periodic GC task: ask helper for alive PIDs every 30s, prune
    // Target Manager's monitored set of any dead PIDs.
    {
        let tm = target_manager.clone();
        let socket = helper_socket.clone();
        tauri::async_runtime::spawn(async move {
            use std::collections::HashSet;
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(30));
            tick.tick().await; // first tick is immediate; skip to align with 30s cadence
            loop {
                tick.tick().await;
                match helper_client::fetch_alive_pids(&socket).await {
                    Ok(pids) => {
                        let alive: HashSet<i64> = pids.into_iter().collect();
                        let pruned = tm.prune_dead(&alive);
                        if pruned > 0 {
                            tracing::info!(pruned, "TargetManager GC pruned dead PIDs");
                        }
                    }
                    Err(e) => tracing::debug!(error = %e, "alive-pid GC fetch failed"),
                }
            }
        });
    }

    // ④ Rule engine — load rules + subscribe to bus + emit findings.
    let initial_rules = rule::loader::load_all(&pool).await;
    tracing::info!(n_rules = initial_rules.len(), "loaded rules");
    let (rules_tx, rules_rx) = tokio::sync::watch::channel(initial_rules);
    // Stash the sender so Tauri commands (Rules CRUD) can push reloads.
    app_handle.manage(Arc::new(rules_tx));
    {
        let pool = pool.clone();
        let bus = bus.clone();
        let app_handle = app_handle.clone();
        let tm = target_manager.clone();
        tauri::async_runtime::spawn(async move {
            rule::run(pool, bus, app_handle, rules_rx, tm).await;
        });
    }

    Ok(())
}
