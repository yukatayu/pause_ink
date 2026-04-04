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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiStatusModel {
    pub project_status: String,
    pub media_status: String,
    pub transport_status: String,
}

pub fn render_status_text(status: &UiStatusModel) -> String {
    [
        "PauseInk UI はまだ実装中です。",
        &status.project_status,
        &status.media_status,
        &status.transport_status,
    ]
    .join("\n")
}

pub fn run(bootstrap: &UiBootstrap, status: &UiStatusModel) -> anyhow::Result<()> {
    tracing::info!(
        title = %bootstrap.title,
        locale = %bootstrap.locale,
        "PauseInk UI bootstrap initialized"
    );
    println!("{}", render_status_text(status));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_text_is_rendered_in_japanese() {
        let text = render_status_text(&UiStatusModel {
            project_status: "オブジェクト数: 0".into(),
            media_status: "読込済みメディア: なし".into(),
            transport_status: "メディア未読み込み".into(),
        });

        assert!(text.contains("PauseInk UI はまだ実装中です。"));
        assert!(text.contains("読込済みメディア: なし"));
        assert!(text.contains("メディア未読み込み"));
    }
}
