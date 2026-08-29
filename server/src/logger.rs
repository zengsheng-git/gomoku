use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::OnceLock;

use tracing::Level;
use tracing_appender::non_blocking;
use tracing_appender::rolling;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry;
use tracing_subscriber::EnvFilter;

static APPENDER_GUARD: OnceLock<Mutex<Option<tracing_appender::non_blocking::WorkerGuard>>> =
    OnceLock::new();

/// 初始化tracing库，设置全局订阅者
pub fn init_tracer(level: Level, app_data_dir: &Path) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        let filter_str = format!(
            "{},wzqlink={},wzqlink_lib={},xcap=warn",
            level.as_str(),
            level.as_str(),
            level.as_str()
        );
        EnvFilter::new(filter_str)
    });

    let log_dir: PathBuf = app_data_dir.join("logs");

    if !log_dir.exists() {
        fs::create_dir_all(&log_dir).expect("无法创建日志目录");
    }

    let file_appender = rolling::daily(&log_dir, "runtime.log");
    let (non_blocking_file, _guard) = non_blocking(file_appender);

    let _unused = APPENDER_GUARD
        .get_or_init(|| Mutex::new(Some(_guard)))
        .lock()
        .unwrap();

    let console_layer =
        tracing_subscriber::fmt::layer().with_span_events(FmtSpan::CLOSE).with_ansi(true).compact();

    let file_layer = tracing_subscriber::fmt::layer()
        .with_span_events(FmtSpan::CLOSE)
        .with_ansi(false)
        .with_writer(non_blocking_file)
        .compact();

    registry().with(filter).with(console_layer).with(file_layer).init();
}
