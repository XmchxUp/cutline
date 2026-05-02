use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::model::{Clip, NormalizedProject};

pub const SCHEMA_VERSION: &str = "cutline-plan-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub output_path: Utf8PathBuf,
    pub clips: Vec<PlannedClip>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedClip {
    pub clip: Clip,
    pub cache_key: String,
    pub cache_path: Utf8PathBuf,
}

pub fn build_plan(project: &NormalizedProject) -> Plan {
    let cache_dir = project.project_dir.join(".cutline").join("cache");
    let clips = project
        .clips
        .iter()
        .cloned()
        .map(|clip| {
            let cache_key = clip_cache_key(&clip);
            let cache_path = cache_dir.join(format!("{cache_key}.mp4"));
            PlannedClip {
                clip,
                cache_key,
                cache_path,
            }
        })
        .collect();

    Plan {
        output_path: project.output_path.clone(),
        clips,
    }
}

fn clip_cache_key(clip: &Clip) -> String {
    let mut hasher = Sha256::new();
    hasher.update(SCHEMA_VERSION);
    hasher.update(clip.index.to_string());
    hasher.update(&clip.input);
    hasher.update(clip.start.millis().to_string());
    hasher.update(clip.end.millis().to_string());
    hasher.update(if clip.blur { "blur=1" } else { "blur=0" });
    hasher.update(if clip.mute { "mute=1" } else { "mute=0" });
    format!("{:x}", hasher.finalize())
}
