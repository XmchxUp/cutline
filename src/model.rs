use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

use crate::time::TimeValue;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedProject {
    pub project_dir: Utf8PathBuf,
    pub output_path: Utf8PathBuf,
    pub inputs: Vec<Input>,
    pub clips: Vec<Clip>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Input {
    pub name: String,
    pub path: Utf8PathBuf,
    pub chat: Option<Utf8PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clip {
    pub index: usize,
    pub input: String,
    pub start: TimeValue,
    pub end: TimeValue,
    pub output_start: TimeValue,
    pub output_end: TimeValue,
    pub chapter: Option<String>,
    pub blur: bool,
    pub mute: bool,
}
