use std::path::Path;

use pauseink_domain::{AnnotationProject, MediaTime};
use pauseink_media::{import_media, ImportedMedia, MediaError, MediaProvider, PlaybackState};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AppSession {
    pub project: AnnotationProject,
    pub imported_media: Option<ImportedMedia>,
    pub playback: Option<PlaybackState>,
}

impl AppSession {
    pub fn import_media(
        &mut self,
        provider: &dyn MediaProvider,
        source_path: &Path,
    ) -> Result<(), MediaError> {
        let imported = import_media(provider, source_path)?;
        self.playback = Some(PlaybackState::new(imported.clone()));
        self.imported_media = Some(imported);
        Ok(())
    }

    pub fn play(&mut self) -> bool {
        let Some(playback) = &mut self.playback else {
            return false;
        };
        playback.play();
        true
    }

    pub fn pause(&mut self) -> bool {
        let Some(playback) = &mut self.playback else {
            return false;
        };
        playback.pause();
        true
    }

    pub fn seek(&mut self, time: MediaTime) -> bool {
        let Some(playback) = &mut self.playback else {
            return false;
        };
        playback.seek(time);
        true
    }

    pub fn transport_summary(&self) -> String {
        let Some(playback) = &self.playback else {
            return "メディア未読み込み".to_owned();
        };

        format!(
            "{} / 現在位置 {} ticks",
            if playback.is_playing {
                "再生中"
            } else {
                "一時停止"
            },
            playback.current_time.ticks
        )
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use pauseink_media::{
        MediaProbe, MediaRuntime, MediaSupport, MediaProvider, RuntimeCapabilities,
    };

    use super::*;

    struct MockMediaProvider {
        probe: MediaProbe,
    }

    impl MediaProvider for MockMediaProvider {
        fn probe(&self, _source_path: &Path) -> Result<MediaProbe, MediaError> {
            Ok(self.probe.clone())
        }

        fn capabilities(&self) -> Result<RuntimeCapabilities, MediaError> {
            Ok(RuntimeCapabilities::default())
        }

        fn diagnostics(&self) -> MediaRuntime {
            MediaRuntime::from_system_path()
        }
    }

    #[test]
    fn import_media_initializes_playback_state() {
        let mut session = AppSession::default();
        let provider = MockMediaProvider {
            probe: MediaProbe {
                format_name: Some("mp4".into()),
                duration_seconds: Some(8.0),
                duration_raw: Some("8.000000".into()),
                width: Some(1280),
                height: Some(720),
                frame_rate: Some(30.0),
                avg_frame_rate_raw: Some("30/1".into()),
                r_frame_rate_raw: Some("30/1".into()),
                pix_fmt: Some("yuv420p".into()),
                has_alpha: false,
                has_audio: true,
                video_codec: Some("h264".into()),
                audio_codec: Some("aac".into()),
                support: MediaSupport::Supported,
            },
        };

        session
            .import_media(&provider, Path::new("sample.mp4"))
            .expect("import should succeed");

        assert_eq!(
            session
                .imported_media
                .as_ref()
                .map(|media| media.source_path.clone()),
            Some(PathBuf::from("sample.mp4"))
        );
        assert_eq!(
            session.playback.as_ref().map(|playback| playback.current_time),
            Some(MediaTime::from_millis(0))
        );
    }

    #[test]
    fn play_pause_seek_update_transport_summary() {
        let mut session = AppSession {
            playback: Some(PlaybackState::new(ImportedMedia {
                source_path: PathBuf::from("sample.mp4"),
                probe: MediaProbe {
                    format_name: Some("mp4".into()),
                    duration_seconds: Some(5.0),
                    duration_raw: Some("5.000000".into()),
                    width: Some(1280),
                    height: Some(720),
                    frame_rate: Some(30.0),
                    avg_frame_rate_raw: Some("30/1".into()),
                    r_frame_rate_raw: Some("30/1".into()),
                    pix_fmt: Some("yuv420p".into()),
                    has_alpha: false,
                    has_audio: true,
                    video_codec: Some("h264".into()),
                    audio_codec: Some("aac".into()),
                    support: MediaSupport::Supported,
                },
            })),
            ..AppSession::default()
        };

        assert!(session.play());
        assert!(session.transport_summary().contains("再生中"));

        assert!(session.seek(MediaTime::from_millis(2_000)));
        assert!(session.transport_summary().contains("2000"));

        assert!(session.pause());
        assert!(session.transport_summary().contains("一時停止"));
    }
}
