use std::collections::BTreeSet;

use camino::{Utf8Path, Utf8PathBuf};

use crate::config::ProjectConfig;
use crate::error::{CutlineError, Result};
use crate::model::{Clip, Input, NormalizedProject};
use crate::time::TimeValue;

pub fn normalize_project(
    project_path: &Utf8Path,
    config: ProjectConfig,
) -> Result<NormalizedProject> {
    let project_dir = project_path
        .parent()
        .map(Utf8Path::to_path_buf)
        .unwrap_or_else(|| Utf8PathBuf::from("."));

    let output_path = resolve_project_path(&project_dir, &config.output.path);

    let mut input_names = BTreeSet::new();
    let inputs = config
        .input
        .into_iter()
        .map(|(name, input)| {
            input_names.insert(name.clone());
            Input {
                name,
                path: resolve_project_path(&project_dir, &input.path),
                chat: input
                    .chat
                    .map(|path| resolve_project_path(&project_dir, &path)),
            }
        })
        .collect::<Vec<_>>();

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

    Ok(NormalizedProject {
        project_dir,
        output_path,
        inputs,
        clips,
    })
}

fn resolve_project_path(project_dir: &Utf8Path, path: &Utf8Path) -> Utf8PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_dir.join(path)
    }
}
