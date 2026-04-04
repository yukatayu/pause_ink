#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiBootstrap {
    pub title: String,
    pub locale: String,
}

impl Default for UiBootstrap {
    fn default() -> Self {
        Self {
            title: "PauseInk".to_owned(),
            locale: "ja-JP".to_owned(),
        }
    }
}

pub fn run(bootstrap: &UiBootstrap) -> anyhow::Result<()> {
    tracing::info!(
        title = %bootstrap.title,
        locale = %bootstrap.locale,
        "PauseInk UI bootstrap initialized"
    );
    println!("PauseInk UI はまだ実装中です。");
    Ok(())
}
