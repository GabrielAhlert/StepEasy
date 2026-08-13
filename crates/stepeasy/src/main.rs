// Sem console no Windows em release; em debug o console fica, que é onde os
// logs de `tracing` aparecem durante o desenvolvimento.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use tracing_subscriber::EnvFilter;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("STEPEASY_LOG").unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    stepeasy_ui::run()
}
