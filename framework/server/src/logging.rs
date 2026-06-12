use tracing_subscriber::{fmt, EnvFilter};

/// Initialize structured logging. Respects `RUST_LOG` (e.g. `info,server=debug`).
pub fn init() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,server=info"));
    let _ = fmt()
        .with_env_filter(filter)
        .with_target(true)
        .try_init();
}
