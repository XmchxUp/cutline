use std::collections::BTreeMap;

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

use crate::time::TimeValue;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    pub output: OutputConfig,

    #[serde(default)]
    pub input: BTreeMap<String, InputConfig>,

    #[serde(default)]
    pub render: RenderConfig,

    #[serde(default, rename = "clip")]
    pub clips: Vec<ClipConfig>,

    #[serde(default, rename = "auto_cut")]
    pub auto_cuts: Vec<AutoCutConfig>,

    #[serde(default, rename = "story")]
    pub story_videos: Vec<StoryVideoConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OutputConfig {
    pub path: Utf8PathBuf,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InputConfig {
    pub path: Utf8PathBuf,

    #[serde(default)]
    pub chat: Option<Utf8PathBuf>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClipConfig {
    pub input: String,
    pub start: TimeValue,
    pub end: TimeValue,

    #[serde(default)]
    pub chapter: Option<String>,

    #[serde(default)]
    pub blur: bool,

    #[serde(default)]
    pub mute: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AutoCutConfig {
    pub name: String,
    pub input: String,
    pub target_duration: TimeValue,
    pub clip_duration: TimeValue,

    #[serde(default = "default_min_clip_duration")]
    pub min_clip_duration: TimeValue,

    #[serde(default)]
    pub rules: Vec<AutoCutRuleConfig>,

    #[serde(default = "default_single_output")]
    pub output_mode: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub enum AutoCutRuleConfig {
    #[serde(rename = "scene_change")]
    SceneChange {
        #[serde(default = "default_scene_threshold")]
        threshold: f64,
    },

    #[serde(rename = "audio_activity")]
    AudioActivity {
        #[serde(default = "default_audio_threshold")]
        threshold: f64,
    },

    #[serde(rename = "motion")]
    Motion {
        #[serde(default = "default_motion_threshold")]
        threshold: f64,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryVideoConfig {
    pub name: String,
    pub source: Utf8PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub engagement_angle: String,
    pub background: Utf8PathBuf,

    #[serde(default = "default_platform")]
    pub platform: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StorySegmentConfig {
    pub start_line: usize,
    pub end_line: usize,

    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RenderConfig {
    #[serde(default = "default_video_codec")]
    pub video_codec: String,

    #[serde(default = "default_audio_codec")]
    pub audio_codec: String,

    #[serde(default = "default_video_bitrate")]
    pub video_bitrate: String,

    #[serde(default = "default_audio_bitrate")]
    pub audio_bitrate: String,

    #[serde(default = "default_pixel_format")]
    pub pixel_format: String,

    #[serde(default = "default_container")]
    pub container: String,

    #[serde(default)]
    pub preset: Option<String>,

    #[serde(default)]
    pub crf: Option<u8>,

    #[serde(default)]
    pub extra: RenderExtraConfig,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            video_codec: default_video_codec(),
            audio_codec: default_audio_codec(),
            video_bitrate: default_video_bitrate(),
            audio_bitrate: default_audio_bitrate(),
            pixel_format: default_pixel_format(),
            container: default_container(),
            preset: None,
            crf: None,
            extra: RenderExtraConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RenderExtraConfig {
    #[serde(default)]
    pub input_args: Vec<String>,

    #[serde(default)]
    pub output_args: Vec<String>,
}

fn default_video_codec() -> String {
    "libx264".to_owned()
}

fn default_audio_codec() -> String {
    "aac".to_owned()
}

fn default_video_bitrate() -> String {
    "4000k".to_owned()
}

fn default_audio_bitrate() -> String {
    "300k".to_owned()
}

fn default_pixel_format() -> String {
    "yuv420p".to_owned()
}

fn default_container() -> String {
    "mp4".to_owned()
}

fn default_min_clip_duration() -> TimeValue {
    TimeValue::from_millis(15000) // 15 seconds
}

fn default_scene_threshold() -> f64 {
    0.3
}

fn default_audio_threshold() -> f64 {
    0.01
}

fn default_motion_threshold() -> f64 {
    0.1
}

fn default_single_output() -> String {
    "single".to_owned()
}

fn default_platform() -> String {
    "douyin".to_owned()
}

#[cfg(test)]
mod tests {
    use super::ProjectConfig;

    #[test]
    fn parses_smoke_example() {
        let config: ProjectConfig = toml::from_str(include_str!("../examples/smoke.toml")).unwrap();

        assert_eq!(config.input.len(), 1);
        assert_eq!(config.clips.len(), 1);
        assert_eq!(config.render.video_codec, "libx264");
    }

    #[test]
    fn parses_story_highlight_video_example() {
        let config: ProjectConfig = toml::from_str(include_str!("../examples/story.toml")).unwrap();

        assert_eq!(config.input.len(), 0);
        assert_eq!(config.clips.len(), 0);
        assert_eq!(config.story_videos.len(), 1);
        assert_eq!(config.story_videos[0].source, "stories/chapter1.txt");
        assert_eq!(config.story_videos[0].platform, "douyin");
    }

    #[test]
    fn rejects_unknown_fields() {
        let err = toml::from_str::<ProjectConfig>(
            r#"
            [output]
            path = "out.mp4"
            surprise = true
            "#,
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("unknown field"));
    }

    #[test]
    fn rejects_unknown_story_fields() {
        let err = toml::from_str::<ProjectConfig>(
            r#"
            [output]
            path = "out.mp4"

            [[story]]
            name = "demo"
            source = "stories/demo.txt"
            start_line = 1
            end_line = 2
            engagement_angle = "reversal"
            background = "assets/bg.mp4"
            platform = "douyin"
            surprise = true
            "#,
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("unknown field"));
        assert!(err.contains("surprise"));
    }
}
