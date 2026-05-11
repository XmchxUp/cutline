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

    let has_creative_workflow = !config.auto_cuts.is_empty() || !config.story_videos.is_empty();

    if inputs.is_empty() && !has_creative_workflow {
        return Err(CutlineError::InvalidProject(
            "at least one [input.<name>] section is required".to_owned(),
        ));
    }

    if config.clips.is_empty() && !has_creative_workflow {
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

    validate_story_video_configs(&project_dir, &config.story_videos)?;

    // Process auto_cuts
    let mut auto_cuts = Vec::with_capacity(config.auto_cuts.len());
    for autocut_config in &config.auto_cuts {
        auto_cuts.push(crate::autocut::generate_autocut(autocut_config));
    }

    // Process story_videos
    let mut story_videos = Vec::with_capacity(config.story_videos.len());
    for story_config in &config.story_videos {
        story_videos.push(crate::story::generate_story_video(story_config));
    }

    Ok(NormalizedProject {
        project_path: project_path.to_path_buf(),
        project_dir,
        output_path,
        render: config.render,
        inputs,
        clips,
        auto_cuts,
        story_videos,
    })
}

fn validate_story_video_configs(
    project_dir: &Utf8Path,
    story_videos: &[crate::config::StoryVideoConfig],
) -> Result<()> {
    for (index, story) in story_videos.iter().enumerate() {
        if story.start_line == 0 || story.end_line < story.start_line {
            return Err(CutlineError::InvalidProject(format!(
                "story[{index}].start_line must be at least 1 and story[{index}].end_line must be greater than or equal to start_line"
            )));
        }
        if story.platform != "douyin" {
            return Err(CutlineError::InvalidProject(format!(
                "story[{index}].platform must be \"douyin\" in V1, got {:?}",
                story.platform
            )));
        }
        if let Some(provider) = &story.voice_provider {
            if provider != "diamoetts" {
                return Err(CutlineError::InvalidProject(format!(
                    "story[{index}].voice_provider must be \"diamoetts\" when set in V1, got {:?}",
                    provider
                )));
            }
        }

        let source = resolve_project_path(project_dir, &story.source);
        if source.extension() != Some("txt") {
            return Err(CutlineError::InvalidProject(format!(
                "story[{index}].source must be a local UTF-8 .txt file: {source}"
            )));
        }
        if !source.is_file() {
            return Err(CutlineError::InvalidProject(format!(
                "story[{index}].source does not exist or is not a file: {source}"
            )));
        }
        let source_content = fs::read_to_string(&source).map_err(|err| {
            CutlineError::InvalidProject(format!(
                "story[{index}].source could not be read as UTF-8 text: {source}: {err}"
            ))
        })?;
        let line_count = source_content.lines().count();
        if story.end_line > line_count {
            return Err(CutlineError::InvalidProject(format!(
                "story[{index}].end_line {} exceeds story[{index}].source line count {line_count}",
                story.end_line
            )));
        }

        let background = resolve_project_path(project_dir, &story.background);
        if !background.is_file() {
            return Err(CutlineError::InvalidProject(format!(
                "story[{index}].background does not exist or is not a file: {background}"
            )));
        }
    }

    Ok(())
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
    use std::fs;

    use camino::Utf8PathBuf;

    use crate::config::{
        ClipConfig, InputConfig, OutputConfig, ProjectConfig, RenderConfig, StoryVideoConfig,
    };
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
            auto_cuts: vec![],
            story_videos: vec![],
        }
    }

    struct StoryFixture {
        root: std::path::PathBuf,
        project_path: Utf8PathBuf,
    }

    impl StoryFixture {
        fn new(name: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("cutline-story-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(root.join("stories")).unwrap();
            fs::create_dir_all(root.join("assets")).unwrap();
            let project_path = Utf8PathBuf::from_path_buf(root.join("project.toml")).unwrap();

            Self { root, project_path }
        }

        fn write_story(&self, path: &str, content: &str) {
            fs::write(self.root.join(path), content).unwrap();
        }

        fn write_background(&self, path: &str) {
            fs::write(self.root.join(path), "not real media").unwrap();
        }
    }

    impl Drop for StoryFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn story_config(story: StoryVideoConfig) -> ProjectConfig {
        ProjectConfig {
            output: OutputConfig {
                path: Utf8PathBuf::from("dist/story.mp4"),
            },
            input: BTreeMap::new(),
            render: RenderConfig::default(),
            clips: vec![],
            auto_cuts: vec![],
            story_videos: vec![story],
        }
    }

    fn valid_story_video_config() -> StoryVideoConfig {
        StoryVideoConfig {
            name: "demo".to_owned(),
            source: Utf8PathBuf::from("stories/demo.txt"),
            start_line: 1,
            end_line: 2,
            engagement_angle: "reversal".to_owned(),
            background: Utf8PathBuf::from("assets/bg.mp4"),
            voice_provider: None,
            platform: "douyin".to_owned(),
        }
    }

    fn normalize_story_fixture(
        fixture: &StoryFixture,
        story: StoryVideoConfig,
    ) -> crate::Result<crate::model::NormalizedProject> {
        normalize_project_with_options(
            &fixture.project_path,
            story_config(story),
            ValidationOptions {
                require_inputs: true,
                probe_media: false,
            },
        )
    }

    #[test]
    fn story_highlight_video_can_be_valid_without_manual_inputs_or_clips() {
        let fixture = StoryFixture::new("valid");
        fixture.write_story(
            "stories/demo.txt",
            "第一行\n第二行\n第三行\n第四行\n第五行\n",
        );
        fixture.write_background("assets/bg.mp4");
        let mut story = valid_story_video_config();
        story.end_line = 4;

        let project = normalize_story_fixture(&fixture, story).unwrap();

        assert!(project.inputs.is_empty());
        assert!(project.clips.is_empty());
        assert_eq!(project.story_videos.len(), 1);
        assert_eq!(project.story_videos[0].name, "demo");
    }

    #[test]
    fn story_highlight_video_rejects_missing_text_source() {
        let fixture = StoryFixture::new("missing-source");
        fixture.write_background("assets/bg.mp4");
        let mut story = valid_story_video_config();
        story.source = Utf8PathBuf::from("stories/missing.txt");
        story.end_line = 4;

        let err = normalize_story_fixture(&fixture, story)
            .unwrap_err()
            .to_string();

        assert!(err.contains("story[0].source"));
        assert!(err.contains("missing.txt"));
    }

    #[test]
    fn story_highlight_video_rejects_missing_background_asset() {
        let fixture = StoryFixture::new("missing-background");
        fixture.write_story("stories/demo.txt", "第一行\n第二行\n");
        let mut story = valid_story_video_config();
        story.background = Utf8PathBuf::from("assets/missing.mp4");

        let err = normalize_story_fixture(&fixture, story)
            .unwrap_err()
            .to_string();

        assert!(err.contains("story[0].background"));
        assert!(err.contains("missing.mp4"));
    }

    #[test]
    fn story_highlight_video_rejects_non_txt_source() {
        let fixture = StoryFixture::new("non-txt-source");
        fixture.write_story("stories/demo.epub", "not supported");
        fixture.write_background("assets/bg.mp4");
        let mut story = valid_story_video_config();
        story.source = Utf8PathBuf::from("stories/demo.epub");

        let err = normalize_story_fixture(&fixture, story)
            .unwrap_err()
            .to_string();

        assert!(err.contains("story[0].source"));
        assert!(err.contains(".txt"));
    }

    #[test]
    fn story_highlight_video_rejects_reversed_line_range() {
        let fixture = StoryFixture::new("reversed-range");
        fixture.write_story("stories/demo.txt", "第一行\n第二行\n第三行\n");
        fixture.write_background("assets/bg.mp4");
        let mut story = valid_story_video_config();
        story.start_line = 3;
        story.end_line = 2;

        let err = normalize_story_fixture(&fixture, story)
            .unwrap_err()
            .to_string();

        assert!(err.contains("story[0]"));
        assert!(err.contains("start_line"));
        assert!(err.contains("end_line"));
    }

    #[test]
    fn story_highlight_video_rejects_zero_start_line() {
        let fixture = StoryFixture::new("zero-start-line");
        fixture.write_story("stories/demo.txt", "第一行\n第二行\n");
        fixture.write_background("assets/bg.mp4");
        let mut story = valid_story_video_config();
        story.start_line = 0;

        let err = normalize_story_fixture(&fixture, story)
            .unwrap_err()
            .to_string();

        assert!(err.contains("story[0].start_line"));
        assert!(err.contains("at least 1"));
    }

    #[test]
    fn story_highlight_video_rejects_out_of_bounds_line_range() {
        let fixture = StoryFixture::new("out-of-bounds-range");
        fixture.write_story("stories/demo.txt", "第一行\n第二行\n");
        fixture.write_background("assets/bg.mp4");
        let mut story = valid_story_video_config();
        story.end_line = 3;

        let err = normalize_story_fixture(&fixture, story)
            .unwrap_err()
            .to_string();

        assert!(err.contains("story[0].end_line"));
        assert!(err.contains("exceeds"));
        assert!(err.contains("2"));
    }

    #[test]
    fn story_highlight_video_rejects_unsupported_platform_profile() {
        let fixture = StoryFixture::new("unsupported-platform");
        fixture.write_story("stories/demo.txt", "第一行\n第二行\n");
        fixture.write_background("assets/bg.mp4");
        let mut story = valid_story_video_config();
        story.platform = "unknown-platform".to_owned();

        let err = normalize_story_fixture(&fixture, story)
            .unwrap_err()
            .to_string();

        assert!(err.contains("story[0].platform"));
        assert!(err.contains("douyin"));
    }

    #[test]
    fn story_highlight_video_rejects_unknown_voice_provider() {
        let fixture = StoryFixture::new("unsupported-voice-provider");
        fixture.write_story("stories/demo.txt", "第一行\n第二行\n");
        fixture.write_background("assets/bg.mp4");
        let mut story = valid_story_video_config();
        story.voice_provider = Some("unknown-tts".to_owned());

        let err = normalize_story_fixture(&fixture, story)
            .unwrap_err()
            .to_string();

        assert!(err.contains("story[0].voice_provider"));
        assert!(err.contains("diamoetts"));
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
