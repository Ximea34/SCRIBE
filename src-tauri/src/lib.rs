pub mod airports;
pub mod aurora;
pub mod domain;
pub mod engine;
pub mod error;
pub mod ipc;
pub mod printing;
pub mod settings;
pub mod templates;

use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Manager, RunEvent};
use tracing::{error, info, warn};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

use airports::Airport;
use aurora::AuroraClient;
use domain::Store;
use engine::{EngineHandle, EngineOptions};
use ipc::events::TauriSink;
use ipc::{Ipc, Templates};
use settings::Settings;

const SETTINGS_FILE: &str = "settings.json";
const ACTIVATIONS_FILE: &str = "activations.json";
const LOG_FILE: &str = "scribe.log";
const LOG_ENV: &str = "SCRIBE_LOG";
const DEFAULT_LOG_FILTER: &str = "info,scribe_lib=debug";
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);
const WORKER_THREADS: usize = 2;

struct LogGuard(Mutex<Option<WorkerGuard>>);

struct Shutdown(Mutex<Option<ShutdownParts>>);

struct ShutdownParts {
    runtime: tokio::runtime::Runtime,
    engine: EngineHandle,
    task: tokio::task::JoinHandle<()>,
}

/// Where the generated TypeScript types land, relative to `src-tauri`.
pub const BINDINGS_PATH: &str = "../src/types/bindings.ts";

fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .commands(tauri_specta::collect_commands![
            ipc::commands::board_snapshot,
            ipc::commands::activate_flight,
            ipc::commands::flight_detail,
            ipc::commands::get_field_catalogue,
            ipc::commands::list_templates,
            ipc::commands::load_template,
            ipc::commands::save_template,
            ipc::commands::delete_template,
            ipc::commands::import_logo,
        ])
        .events(tauri_specta::collect_events![ipc::events::BoardUpdated])
}

/// Regenerates `src/types/bindings.ts`; a test calls this so the bindings can never drift.
/// The header suppresses type checking of the generated file itself, not of its consumers.
pub fn export_bindings(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let typescript = specta_typescript::Typescript::default().header("// @ts-nocheck");
    specta_builder().export(typescript, path)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = specta_builder();

    #[cfg(debug_assertions)]
    if let Err(error) = export_bindings(BINDINGS_PATH) {
        warn!(%error, "cannot regenerate the TypeScript bindings");
    }

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            app.manage(LogGuard(Mutex::new(init_logging(app.handle()))));
            manage_templates(app.handle());
            start(app.handle());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building the application");

    app.run(|handle, event| {
        if matches!(event, RunEvent::Exit) {
            if let Some(shutdown) = handle.try_state::<Shutdown>() {
                shutdown.close();
            }
            if let Some(logs) = handle.try_state::<LogGuard>() {
                logs.flush();
            }
        }
    });
}

impl LogGuard {
    /// Dropping the appender guard is what flushes the last lines to the log file.
    fn flush(&self) {
        if let Ok(mut slot) = self.0.lock() {
            drop(slot.take());
        }
    }
}

fn init_logging(app: &AppHandle) -> Option<WorkerGuard> {
    let filter =
        EnvFilter::try_from_env(LOG_ENV).unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG_FILTER));
    let console = fmt::layer().with_writer(std::io::stderr).with_target(false);

    let file = app.path().app_log_dir().ok().and_then(|dir| {
        std::fs::create_dir_all(&dir).ok()?;
        let appender = tracing_appender::rolling::daily(dir, LOG_FILE);
        Some(tracing_appender::non_blocking(appender))
    });

    match file {
        Some((writer, guard)) => {
            tracing_subscriber::registry()
                .with(filter)
                .with(console)
                .with(fmt::layer().with_writer(writer).with_ansi(false))
                .init();
            Some(guard)
        }
        None => {
            tracing_subscriber::registry()
                .with(filter)
                .with(console)
                .init();
            None
        }
    }
}

/// Registered before the board and independently of it: the editor works with Aurora
/// disconnected and with no airport configured.
fn manage_templates(app: &AppHandle) {
    let directory = match app.path().app_data_dir() {
        Ok(data_dir) => templates::storage::directory(&data_dir),
        Err(error) => {
            error!(%error, "no application data directory; templates fall back to the temp dir");
            std::env::temp_dir().join("scribe-strips")
        }
    };
    info!(?directory, "strip templates directory");
    app.manage(Templates(directory));
}

/// Brings the board up. A missing or unusable airport configuration leaves the UI running with
/// an empty board rather than refusing to start, since OPTIONS does not exist yet to fix it.
fn start(app: &AppHandle) {
    let Ok(config_dir) = app.path().app_config_dir() else {
        error!("no application config directory; the board cannot start");
        app.manage(Ipc(None));
        return;
    };

    let mut settings = Settings::load(&config_dir.join(SETTINGS_FILE)).unwrap_or_else(|error| {
        warn!(%error, "falling back to default settings");
        Settings::default()
    });
    settings.apply_env_overrides();

    let Some(airport) = resolve_airport(&settings) else {
        app.manage(Ipc(None));
        return;
    };
    info!(icao = %airport.icao, name = %airport.name, "controlled airport");

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(WORKER_THREADS)
        .thread_name("scribe")
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            error!(%error, "cannot start the async runtime");
            app.manage(Ipc(None));
            return;
        }
    };

    let store = Store::new(airport, settings.domain_config());
    let options = EngineOptions {
        settings: settings.clone(),
        activations_path: config_dir.join(ACTIVATIONS_FILE),
    };

    let (engine, task) = {
        let _guard = runtime.enter();
        let (client, _client_task) = AuroraClient::spawn(settings.client_config());
        engine::spawn(client, store, options, Box::new(TauriSink(app.clone())))
    };

    app.manage(Ipc(Some(engine.clone())));
    app.manage(Shutdown(Mutex::new(Some(ShutdownParts {
        runtime,
        engine,
        task,
    }))));
}

fn resolve_airport(settings: &Settings) -> Option<Airport> {
    let (Some(file), Some(icao)) = (&settings.airports_file, &settings.selected_icao) else {
        warn!(
            "no airport configured; set {} and {} or edit {SETTINGS_FILE}",
            settings::ENV_AIRPORTS_FILE,
            settings::ENV_ICAO
        );
        return None;
    };
    match airports::load_selected(file, icao) {
        Ok(airport) => Some(airport),
        Err(error) => {
            error!(%error, "cannot load the controlled airport; the board stays empty");
            None
        }
    }
}

impl Shutdown {
    /// Stops the engine, which drops the Aurora client and closes the socket.
    fn close(&self) {
        let Ok(mut slot) = self.0.lock() else {
            return;
        };
        let Some(parts) = slot.take() else {
            return;
        };
        parts.runtime.block_on(async {
            parts.engine.shutdown().await;
            let _ = tokio::time::timeout(SHUTDOWN_TIMEOUT, parts.task).await;
        });
        info!("board stopped and the Aurora socket closed");
    }
}
