use anyhow::Result;

fn main() -> Result<()> {
    init_tracing();
    pauseink_ui::run(&pauseink_ui::UiBootstrap::default())
}

fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .without_time()
        .try_init();
}
