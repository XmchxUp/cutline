use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ffmpeg::{FfmpegCommand, clip_render_command, concat_command};
use crate::model::{Clip, Input, NormalizedProject};

pub const SCHEMA_VERSION: &str = "cutline-plan-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub output_path: Utf8PathBuf,
    pub concat_list_path: Utf8PathBuf,
    pub inputs: Vec<PlannedInput>,
    pub clips: Vec<PlannedClip>,
    pub chapters: Vec<PlannedChapter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedInput {
    pub name: String,
    pub path: Utf8PathBuf,
    pub duration: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedClip {
    pub clip: Clip,
    pub cache_key: String,
    pub cache_path: Utf8PathBuf,
    pub cache_exists: bool,
    pub ffmpeg: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedChapter {
    pub time: String,
    pub title: String,
}

pub fn build_plan(project: &NormalizedProject) -> crate::Result<Plan> {
    let cache_dir = crate::cache::cache_dir(project);
    let clips = project
        .clips
        .iter()
        .cloned()
        .map(|clip| -> crate::Result<PlannedClip> {
            let input = project.input(&clip.input).ok_or_else(|| {
                crate::CutlineError::InvalidProject(format!("unknown input {:?}", clip.input))
            })?;
            let cache_key = clip_cache_key(project, input, &clip);
            let cache_path = cache_dir.join(format!("{cache_key}.mp4"));
            let stub = planned_clip_stub(clip.clone(), &cache_key, &cache_path);
            let command = clip_render_command(project, &stub, &cache_path)?;

            Ok(PlannedClip {
                clip,
                cache_key,
                cache_exists: cache_path.exists(),
                cache_path,
                ffmpeg: command.display(),
            })
        })
        .collect::<crate::Result<Vec<_>>>()?;

    let inputs = project
        .inputs
        .iter()
        .map(|input| PlannedInput {
            name: input.name.clone(),
            path: input.path.clone(),
            duration: input
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.duration)
                .map(|duration| duration.display()),
        })
        .collect();

    let chapters = project
        .clips
        .iter()
        .filter_map(|clip| {
            clip.chapter.as_ref().map(|title| PlannedChapter {
                time: clip.output_start.display(),
                title: title.clone(),
            })
        })
        .collect();

    Ok(Plan {
        output_path: project.output_path.clone(),
        concat_list_path: crate::cache::concat_list_path(project),
        inputs,
        clips,
        chapters,
    })
}

pub fn render_commands(
    project: &NormalizedProject,
    plan: &Plan,
) -> crate::Result<Vec<FfmpegCommand>> {
    plan.clips
        .iter()
        .map(|planned| {
            let temp_path = crate::ffmpeg::temp_clip_path(&planned.cache_path);
            clip_render_command(project, planned, &temp_path)
        })
        .collect()
}

pub fn final_concat_command(plan: &Plan) -> FfmpegCommand {
    concat_command(plan, &plan.concat_list_path)
}

fn planned_clip_stub(clip: Clip, cache_key: &str, cache_path: &Utf8Path) -> PlannedClip {
    PlannedClip {
        clip,
        cache_key: cache_key.to_owned(),
        cache_path: cache_path.to_path_buf(),
        cache_exists: false,
        ffmpeg: String::new(),
    }
}

fn clip_cache_key(project: &NormalizedProject, input: &Input, clip: &Clip) -> String {
    let mut hasher = Sha256::new();
    hasher.update(SCHEMA_VERSION);
    hash_str(&mut hasher, &project.render.video_codec);
    hash_str(&mut hasher, &project.render.audio_codec);
    hash_str(&mut hasher, &project.render.video_bitrate);
    hash_str(&mut hasher, &project.render.audio_bitrate);
    hash_str(&mut hasher, &project.render.pixel_format);
    hash_str(&mut hasher, &project.render.container);
    hash_opt_str(&mut hasher, project.render.preset.as_deref());
    hash_opt_u8(&mut hasher, project.render.crf);
    for arg in &project.render.extra.input_args {
        hash_str(&mut hasher, arg);
    }
    for arg in &project.render.extra.output_args {
        hash_str(&mut hasher, arg);
    }
    hasher.update(clip.index.to_string());
    hash_str(&mut hasher, &clip.input);
    hash_str(&mut hasher, input.path.as_str());
    if let Some(metadata) = &input.metadata {
        hasher.update(metadata.size_bytes.to_string());
        if let Some(modified) = metadata.modified_unix_millis {
            hasher.update(modified.to_string());
        }
    }
    hasher.update(clip.start.millis().to_string());
    hasher.update(clip.end.millis().to_string());
    hasher.update(if clip.blur { "blur=1" } else { "blur=0" });
    hasher.update(if clip.mute { "mute=1" } else { "mute=0" });
    format!("{:x}", hasher.finalize())
}

fn hash_str(hasher: &mut Sha256, value: &str) {
    hasher.update(value.len().to_string());
    hasher.update(":");
    hasher.update(value);
    hasher.update(";");
}

fn hash_opt_str(hasher: &mut Sha256, value: Option<&str>) {
    match value {
        Some(value) => {
            hasher.update("some:");
            hash_str(hasher, value);
        }
        None => hasher.update("none;"),
    }
}

fn hash_opt_u8(hasher: &mut Sha256, value: Option<u8>) {
    match value {
        Some(value) => {
            hasher.update("some:");
            hasher.update(value.to_string());
        }
        None => hasher.update("none;"),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use camino::Utf8PathBuf;

    use crate::config::{ClipConfig, InputConfig, OutputConfig, ProjectConfig, RenderConfig};
    use crate::time::TimeValue;
    use crate::validate::{ValidationOptions, normalize_project_with_options};

    use super::build_plan;

    fn config_with_blur(blur: bool) -> ProjectConfig {
        let mut input = BTreeMap::new();
        input.insert(
            "main".to_owned(),
            InputConfig {
                path: Utf8PathBuf::from("Cargo.toml"),
                chat: None,
            },
        );

        ProjectConfig {
            output: OutputConfig {
                path: Utf8PathBuf::from("out.mp4"),
            },
            input,
            render: RenderConfig::default(),
            clips: vec![ClipConfig {
                input: "main".to_owned(),
                start: TimeValue::from_millis(0),
                end: TimeValue::from_millis(1_000),
                chapter: Some("Opening".to_owned()),
                blur,
                mute: false,
            }],
            auto_cuts: vec![],
            story_videos: vec![],
        }
    }

    #[test]
    fn builds_plan_with_chapters_and_cache_paths() {
        let project = normalize_project_with_options(
            "project.toml".into(),
            config_with_blur(false),
            ValidationOptions {
                require_inputs: true,
                probe_media: false,
            },
        )
        .unwrap();
        let plan = build_plan(&project).unwrap();

        assert_eq!(plan.clips.len(), 1);
        assert_eq!(plan.chapters[0].time, "00:00:00.000");
        assert_eq!(plan.chapters[0].title, "Opening");
        assert!(plan.clips[0].cache_path.as_str().contains(".cutline/cache"));
        assert!(plan.clips[0].ffmpeg.contains("ffmpeg"));
    }

    #[test]
    fn cache_key_changes_when_effect_changes() {
        let project_a = normalize_project_with_options(
            "project.toml".into(),
            config_with_blur(false),
            ValidationOptions {
                require_inputs: true,
                probe_media: false,
            },
        )
        .unwrap();
        let project_b = normalize_project_with_options(
            "project.toml".into(),
            config_with_blur(true),
            ValidationOptions {
                require_inputs: true,
                probe_media: false,
            },
        )
        .unwrap();

        let plan_a = build_plan(&project_a).unwrap();
        let plan_b = build_plan(&project_b).unwrap();

        assert_ne!(plan_a.clips[0].cache_key, plan_b.clips[0].cache_key);
    }
}
