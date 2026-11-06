//! Logs go to `<data dir>/hyperform.log`. The directory may not exist on
//! first run, and a logging failure must never block launch.

use std::path::Path;

pub(crate) const LOG_FILE: &str = "hyperform.log";

pub(crate) fn init(data_dir: &Path) {
    let path = data_dir.join(LOG_FILE);
    let _ = std::fs::create_dir_all(data_dir);
    let file = match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        Ok(file) => file,
        Err(error) => {
            eprintln!("hyperform: cannot open log {}: {error}", path.display());
            return;
        }
    };
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_writer(std::sync::Mutex::new(file))
        .with_env_filter(filter)
        .with_ansi(false)
        .try_init()
        .ok();
}
