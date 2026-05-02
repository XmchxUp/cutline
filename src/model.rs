use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

use crate::config::RenderConfig;
use crate::time::TimeValue;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedProject {
    pub project_path: Utf8PathBuf,
    pub project_dir: Utf8PathBuf,
    pub output_path: Utf8PathBuf,
    pub render: RenderConfig,
    pub inputs: Vec<Input>,
    pub clips: Vec<Clip>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Input {
    pub name: String,
    pub path: Utf8PathBuf,
    pub chat: Option<Utf8PathBuf>,
    pub metadata: Option<InputMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputMetadata {
    pub size_bytes: u64,
    pub modified_unix_millis: Option<i128>,
    pub duration: Option<TimeValue>,
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

impl NormalizedProject {
    pub fn input(&self, name: &str) -> Option<&Input> {
        self.inputs.iter().find(|input| input.name == name)
    }

    pub fn input_mut(&mut self, name: &str) -> Option<&mut Input> {
        self.inputs.iter_mut().find(|input| input.name == name)
    }
}

impl Clip {
    pub fn duration(&self) -> TimeValue {
        TimeValue::from_millis(self.end.millis() - self.start.millis())
    }
}
