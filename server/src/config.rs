use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;
use tracing::debug;

use crate::engine::EngineConfig;
use crate::vision::Calibration;
use crate::SHARED_STATE;

#[derive(Serialize, Deserialize)]
pub struct Config {
    #[serde(skip)]
    config_path: Option<PathBuf>,
    // trace, debug, info, warn, silent
    pub loglevel: String,
    pub timer_interval: u64,
    pub confirm_interval: u64,

    // 棋盘区域校准（归一化坐标）
    pub calibration: Option<Calibration>,

    pub engine: EngineConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            config_path: None,
            loglevel: "INFO".to_string(),
            timer_interval: 200,
            confirm_interval: 200,
            calibration: None,
            engine: Default::default(),
        }
    }
}

impl Config {
    pub fn load(base: &Path) -> Self {
        let dir = base.join("wzqlink");
        if !dir.exists() {
            let _ = fs::create_dir(&dir);
        };

        let config_path = dir.join("config.json");
        debug!("load config from '{}'", config_path.display());

        if config_path.exists() {
            let reader = BufReader::new(File::open(&config_path).unwrap());
            if let Ok(mut config) = serde_json::from_reader::<_, Config>(reader) {
                config.config_path = Some(config_path);
                config.save();
                return config;
            };

            // 解析失败代表配置不兼容, 直接删除后重新使用默认配置
            let _ = std::fs::remove_file(&config_path);
            debug!("remove old config '{}'", config_path.display())
        }

        let config = Config { config_path: Some(config_path), ..Default::default() };
        config.save();
        config
    }

    pub fn save(&self) {
        let Some(path) = &self.config_path else { return };
        debug!("save config to '{}'", path.display());
        let json_string = serde_json::to_string_pretty(self).unwrap();

        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(mut file) = File::create(path) {
            let _ = file.write_all(json_string.as_bytes());
        }
    }
}

#[tauri::command]
pub async fn get_engine_config() -> EngineConfig { SHARED_STATE.get().unwrap().config.read().unwrap().engine }

#[tauri::command]
pub async fn set_engine_depth(depth: usize) {
    let state = SHARED_STATE.get().unwrap();
    let mut config = state.config.write().unwrap();
    config.engine.depth = depth;
    config.save();
    debug!("set_engine_depth: {}", depth);
}

#[tauri::command]
pub async fn set_engine_time(time: f32) {
    let state = SHARED_STATE.get().unwrap();
    let mut config = state.config.write().unwrap();
    config.engine.time = (time * 1000.0) as usize;
    config.save();
    debug!("set_engine_time: {}", time);
}

#[tauri::command]
pub async fn set_engine_threads(num: usize) {
    let state = SHARED_STATE.get().unwrap();
    let mut config = state.config.write().unwrap();
    config.engine.threads = num;
    config.save();
    debug!("set_engine_threads: {}", num);
}

#[tauri::command]
pub async fn set_engine_hash(size: usize) {
    let state = SHARED_STATE.get().unwrap();
    let mut config = state.config.write().unwrap();
    config.engine.hash = size;
    config.save();
    debug!("set_engine_hash: {}", size);
}

#[tauri::command]
pub async fn set_engine_multipv(num: usize) {
    let state = SHARED_STATE.get().unwrap();
    let mut config = state.config.write().unwrap();
    config.engine.multipv = num;
    config.save();
    debug!("set_engine_multipv: {}", num);
}

#[tauri::command]
pub async fn set_engine_alt_score_gap(gap: isize) {
    let state = SHARED_STATE.get().unwrap();
    let mut config = state.config.write().unwrap();
    config.engine.alt_score_gap = gap;
    config.save();
    debug!("set_engine_alt_score_gap: {}", gap);
}

#[tauri::command]
pub async fn set_engine_rule(rule: u8) {
    let state = SHARED_STATE.get().unwrap();
    let mut config = state.config.write().unwrap();
    config.engine.rule = rule.min(2);
    config.save();
    debug!("set_engine_rule: {}", rule);
}

#[tauri::command]
pub async fn set_calibration(x0: f64, y0: f64, x1: f64, y1: f64) {
    let state = SHARED_STATE.get().unwrap();
    let mut config = state.config.write().unwrap();
    config.calibration = Some(Calibration { x0, y0, x1, y1 });
    config.save();
    debug!("set_calibration: ({x0},{y0}) ({x1},{y1})");
}
