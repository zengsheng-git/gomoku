use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::RwLock;
use std::thread;

use engine::Engine;
use tauri::Manager as _;

mod board;
mod config;
mod engine;
mod listen;
mod logger;
mod vision;
mod worker;

// 全局共享状态，用Arc和Mutex包装以实现线程安全共享
struct SharedState {
    config: Arc<RwLock<config::Config>>,
    engine: Arc<Mutex<Engine>>,
    listen_thread: Mutex<Option<thread::JoinHandle<()>>>,
}

static SHARED_STATE: OnceLock<SharedState> = OnceLock::new();

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            logger::init_tracer(tracing::Level::DEBUG, &app.path().app_data_dir().unwrap());

            let _ = SHARED_STATE.get_or_init(|| {
                let config = config::Config::load(&app.path().app_data_dir().unwrap());
                let lib_path = app.path().resolve("../libs/rapfi", tauri::path::BaseDirectory::Resource).unwrap();
                let mut engine = Engine::new(&lib_path);
                engine.apply_static_config(&config.engine);

                SharedState {
                    config: Arc::new(RwLock::new(config)),
                    engine: Arc::new(Mutex::new(engine)),
                    listen_thread: Mutex::new(None),
                }
            });

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            reload_engine,
            listen::list_windows,
            listen::capture_window_image,
            worker::start_listen,
            worker::stop_listen,
            config::get_engine_config,
            config::set_engine_depth,
            config::set_engine_time,
            config::set_engine_threads,
            config::set_engine_hash,
            config::set_engine_multipv,
            config::set_engine_alt_score_gap,
            config::set_engine_rule,
            config::set_calibration,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
fn reload_engine(app: tauri::AppHandle) {
    let lib_path = app.path().resolve("../libs/rapfi", tauri::path::BaseDirectory::Resource).unwrap();
    let state = SHARED_STATE.get().unwrap();
    let engine_config = state.config.read().unwrap().engine;
    state.engine.lock().unwrap().reload(&lib_path, &engine_config);
}

/// 校准用截图响应
#[derive(serde::Serialize)]
pub struct CaptureImage {
    pub base64: String,
    pub width: u32,
    pub height: u32,
}
