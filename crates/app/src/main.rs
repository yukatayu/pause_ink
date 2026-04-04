use anyhow::Result;

fn main() -> Result<()> {
    init_tracing();
    let session = pauseink_app::AppSession::default();
    let status = pauseink_ui::UiStatusModel {
        project_status: format!("オブジェクト数: {}", session.project.glyph_objects.len()),
        media_status: session
            .imported_media
            .as_ref()
            .map(|media| format!("読込済みメディア: {}", media.source_path.display()))
            .unwrap_or_else(|| "読込済みメディア: なし".to_owned()),
        transport_status: session.transport_summary(),
    };
    pauseink_ui::run(&pauseink_ui::UiBootstrap::default(), &status)
}

fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .without_time()
        .try_init();
}
