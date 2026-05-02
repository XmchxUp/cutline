use std::collections::BTreeMap;

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

use crate::time::TimeValue;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProjectConfig {
    pub output: OutputConfig,

    #[serde(default)]
    pub input: BTreeMap<String, InputConfig>,

    #[serde(default)]
    pub render: RenderConfig,

    #[serde(default, rename = "clip")]
    pub clips: Vec<ClipConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OutputConfig {
    pub path: Utf8PathBuf,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InputConfig {
    pub path: Utf8PathBuf,

    #[serde(default)]
    pub chat: Option<Utf8PathBuf>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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
