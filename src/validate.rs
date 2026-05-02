use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::time::UNIX_EPOCH;

use camino::{Utf8Path, Utf8PathBuf};

use crate::config::ProjectConfig;
use crate::error::{CutlineError, Result};
use crate::ffmpeg::probe_duration;
use crate::model::{Clip, Input, InputMetadata, NormalizedProject};
use crate::time::TimeValue;

#[derive(Debug, Clone, Copy)]
pub struct ValidationOptions {
    pub require_inputs: bool,
    pub probe_media: bool,
}

impl Default for ValidationOptions {
    fn default() -> Self {
        Self {
            require_inputs: true,
            probe_media: true,
        }
    }
}

pub fn normalize_project(
    project_path: &Utf8Path,
    config: ProjectConfig,
) -> Result<NormalizedProject> {
    normalize_project_with_options(project_path, config, ValidationOptions::default())
}

pub fn normalize_project_with_options(
    project_path: &Utf8Path,
    config: ProjectConfig,
    options: ValidationOptions,
) -> Result<NormalizedProject> {
    if config.render.container != "mp4" {
        return Err(CutlineError::InvalidProject(format!(
            "render.container must be \"mp4\" in V1, got {:?}",
            config.render.container
        )));
    }

    let project_dir = project_path
        .parent()
        .map(Utf8Path::to_path_buf)
        .unwrap_or_else(|| Utf8PathBuf::from("."));

    let output_path = resolve_project_path(&project_dir, &config.output.path);

    let mut input_names = BTreeSet::new();
    let mut inputs = Vec::with_capacity(config.input.len());
    for (name, input) in config.input {
        input_names.insert(name.clone());
        let path = resolve_project_path(&project_dir, &input.path);
        let (path, metadata) = load_input_path(&path, options)?;
        inputs.push(Input {
            name,
            path,
            chat: input
                .chat
                .map(|path| resolve_project_path(&project_dir, &path)),
            metadata,
        });
    }

    if inputs.is_empty() {
        return Err(CutlineError::InvalidProject(
            "at least one [input.<name>] section is required".to_owned(),
        ));
    }

    if config.clips.is_empty() {
        return Err(CutlineError::InvalidProject(
            "at least one [[clip]] section is required".to_owned(),
        ));
    }

    let mut output_cursor = TimeValue::from_millis(0);
    let mut clips = Vec::with_capacity(config.clips.len());

    for (index, clip) in config.clips.into_iter().enumerate() {
        if !input_names.contains(&clip.input) {
            return Err(CutlineError::InvalidProject(format!(
                "unknown input {:?} at clip[{index}].input",
                clip.input
            )));
        }

        if clip.end <= clip.start {
            return Err(CutlineError::InvalidProject(format!(
                "clip[{index}].end must be greater than clip[{index}].start"
            )));
        }

        let duration = clip.end.millis() - clip.start.millis();
        let output_start = output_cursor;
        let output_end = TimeValue::from_millis(output_start.millis() + duration);
        output_cursor = output_end;

        clips.push(Clip {
            index,
            input: clip.input,
            start: clip.start,
            end: clip.end,
            output_start,
            output_end,
            chapter: clip.chapter,
            blur: clip.blur,
            mute: clip.mute,
        });
    }

    if options.probe_media {
        validate_clip_ranges_against_inputs(&clips, &inputs)?;
    }

    Ok(NormalizedProject {
        project_path: project_path.to_path_buf(),
        project_dir,
        output_path,
        render: config.render,
        inputs,
        clips,
    })
}

pub fn resolve_project_path(project_dir: &Utf8Path, path: &Utf8Path) -> Utf8PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_dir.join(path)
    }
}

fn load_input_path(
    path: &Utf8Path,
    options: ValidationOptions,
) -> Result<(Utf8PathBuf, Option<InputMetadata>)> {
    let canonical_path = match fs::canonicalize(path) {
        Ok(path) => Utf8PathBuf::from_path_buf(path)
            .map_err(|path| CutlineError::NonUtf8Path(path.display().to_string()))?,
        Err(err) if err.kind() == ErrorKind::NotFound && !options.require_inputs => {
            return Ok((path.to_path_buf(), None));
        }
        Err(err) => {
            return Err(CutlineError::InvalidProject(format!(
                "could not access input path {path}: {err}"
            )));
        }
    };

    let metadata = fs::metadata(&canonical_path).map_err(|err| {
        CutlineError::InvalidProject(format!(
            "could not access input path {canonical_path}: {err}"
        ))
    })?;
    if !metadata.is_file() {
        return Err(CutlineError::InvalidProject(format!(
            "input path is not a file: {canonical_path}"
        )));
    }

    let modified_unix_millis = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as i128);

    let duration = if options.probe_media {
        Some(probe_duration(&canonical_path)?)
    } else {
        None
    };

    Ok((
        canonical_path,
        Some(InputMetadata {
            size_bytes: metadata.len(),
            modified_unix_millis,
            duration,
        }),
    ))
}

fn validate_clip_ranges_against_inputs(clips: &[Clip], inputs: &[Input]) -> Result<()> {
    for clip in clips {
        let input = inputs
            .iter()
            .find(|input| input.name == clip.input)
            .ok_or_else(|| {
                CutlineError::InvalidProject(format!(
                    "unknown input {:?} at clip[{}].input",
                    clip.input, clip.index
                ))
            })?;

        let duration = input
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.duration)
            .ok_or_else(|| {
                CutlineError::InvalidProject(format!(
                    "missing probed duration for input {:?}",
                    input.name
                ))
            })?;

        if clip.end > duration {
            return Err(CutlineError::InvalidProject(format!(
                "clip[{}].end {} exceeds input {:?} duration {}",
                clip.index,
                clip.end.display(),
                clip.input,
                duration.display()
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use camino::Utf8PathBuf;

    use crate::config::{ClipConfig, InputConfig, OutputConfig, ProjectConfig, RenderConfig};
    use crate::time::TimeValue;

    use super::{ValidationOptions, normalize_project_with_options};

    fn project_config() -> ProjectConfig {
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
                start: TimeValue::from_millis(1_000),
                end: TimeValue::from_millis(2_500),
                chapter: Some("Start".to_owned()),
                blur: false,
                mute: true,
            }],
        }
    }

    #[test]
    fn derives_output_times_and_resolves_paths() {
        let project = normalize_project_with_options(
            "project.toml".into(),
            project_config(),
            ValidationOptions {
                require_inputs: true,
                probe_media: false,
            },
        )
        .unwrap();

        assert_eq!(project.output_path, Utf8PathBuf::from("out.mp4"));
        assert!(project.inputs[0].path.is_absolute());
        assert!(project.inputs[0].path.ends_with("Cargo.toml"));
        assert_eq!(project.clips[0].output_start.millis(), 0);
        assert_eq!(project.clips[0].output_end.millis(), 1_500);
        assert!(project.inputs[0].metadata.is_some());
    }

    #[test]
    fn rejects_unknown_clip_input() {
        let mut config = project_config();
        config.clips[0].input = "missing".to_owned();

        let err = normalize_project_with_options(
            "project.toml".into(),
            config,
            ValidationOptions {
                require_inputs: true,
                probe_media: false,
            },
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("unknown input"));
    }
}
