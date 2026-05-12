use std::fs;

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::StoryVideoConfig;
use crate::error::{CutlineError, Result};
use crate::ffmpeg::{FfmpegCommand, story_preview_render_args};
use crate::model::{NormalizedProject, StoryVideo};
use crate::time::TimeValue;

pub const DRAFT_SCHEMA_VERSION: &str = "cutline-draft-v1";
const NARRATION_FILE: &str = "narration.txt";
const SUBTITLES_FILE: &str = "subtitles.srt";
const PREVIEW_FILE: &str = "preview.mp4";
const VOICEOVER_FILE: &str = "voiceover.wav";
const SUBTITLE_MAX_CHARS_PER_LINE: usize = 18;
const SUBTITLE_CUE_DURATION_MILLIS: u64 = 2_000;

#[derive(Debug, Clone)]
pub struct DraftPackageSummary {
    pub draft_id: String,
    pub package_path: Utf8PathBuf,
}

#[derive(Debug, Clone)]
pub struct DraftPackageOptions {
    pub render_preview: bool,
    pub ffmpeg_program: String,
    pub voice_provider: VoiceProviderConfig,
}

impl Default for DraftPackageOptions {
    fn default() -> Self {
        Self {
            render_preview: false,
            ffmpeg_program: "ffmpeg".to_owned(),
            voice_provider: VoiceProviderConfig::None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum VoiceProviderConfig {
    None,
    DiamoETts,
    Test { audio_bytes: Vec<u8> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftManifest {
    pub schema_version: String,
    pub workflow: String,
    pub draft_id: String,
    pub platform_profile: String,
    pub story: ManifestStory,
    pub visual: ManifestVisual,
    pub output: ManifestOutput,
    pub generated_assets: GeneratedAssets,
    pub subtitle_style: SubtitleStyle,
    pub short_video_drafts: Vec<ShortVideoDraft>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestStory {
    pub source: ManifestStorySource,
    pub engagement_angle: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestStorySource {
    pub path: Utf8PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestVisual {
    pub background: ManifestAssetReference,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestAssetReference {
    pub path: Utf8PathBuf,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestOutput {
    pub path: Utf8PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedAssets {
    pub narration: ManifestAssetReference,
    pub subtitles: ManifestAssetReference,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<ManifestAssetReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voiceover: Option<ManifestAssetReference>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleStyle {
    pub platform: String,
    pub aspect_ratio: String,
    pub max_chars_per_line: usize,
    pub position: String,
    pub safe_area_bottom_percent: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceMap {
    pub schema_version: String,
    pub draft_id: String,
    pub references: Vec<ReferenceEntry>,
    pub pipeline_step_runs: Vec<PipelineStepRun>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceEntry {
    pub target: String,
    pub source: Utf8PathBuf,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStepRun {
    pub step: String,
    pub status: String,
    pub provider: String,
    pub parameters: serde_json::Value,
    pub outputs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortVideoDraft {
    pub id: String,
    pub hook: ScriptText,
    pub content_beats: Vec<ScriptText>,
    pub adapted_narration: ScriptText,
    pub ending: ScriptText,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptText {
    pub text: String,
    pub source_reference: SourceReference,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceReference {
    pub source: Utf8PathBuf,
    pub start_line: usize,
    pub end_line: usize,
}

/// Generate a StoryVideo from its configuration.
pub fn generate_story_video(config: &StoryVideoConfig) -> StoryVideo {
    StoryVideo {
        name: config.name.clone(),
        source: config.source.clone(),
        start_line: config.start_line,
        end_line: config.end_line,
        engagement_angle: config.engagement_angle.clone(),
        background: config.background.clone(),
        voice_provider: config.voice_provider.clone(),
        platform: config.platform.clone(),
    }
}

pub fn generate_reviewable_draft_package(
    project: &NormalizedProject,
) -> Result<DraftPackageSummary> {
    generate_reviewable_draft_package_with_options(project, DraftPackageOptions::default())
}

pub fn generate_reviewable_draft_package_with_options(
    project: &NormalizedProject,
    options: DraftPackageOptions,
) -> Result<DraftPackageSummary> {
    let story = project.story_videos.first().ok_or_else(|| {
        CutlineError::InvalidProject(
            "at least one [[story]] section is required to generate a draft package".to_owned(),
        )
    })?;

    let package_path = project
        .project_dir
        .join(".cutline")
        .join("drafts")
        .join(&story.name);
    fs::create_dir_all(package_path.join("assets"))?;
    let short_video_drafts = vec![generate_local_script_draft(&project.project_dir, story)?];
    let narration_text = format!("{}\n", short_video_drafts[0].adapted_narration.text);
    let subtitles = generate_subtitles(&short_video_drafts[0].adapted_narration.text);
    let subtitles_text = format_srt(&subtitles);
    fs::write(package_path.join(NARRATION_FILE), &narration_text)?;
    fs::write(package_path.join(SUBTITLES_FILE), &subtitles_text)?;
    let voice_provider = selected_voice_provider(story, options.voice_provider);
    let voice_step = run_voice_step(&package_path, &voice_provider)?;
    let preview_path = package_path.join(PREVIEW_FILE);
    let preview_command = if options.render_preview {
        let command = FfmpegCommand {
            program: options.ffmpeg_program.clone(),
            args: story_preview_render_args(
                &project.project_dir.join(&story.background),
                &package_path.join(SUBTITLES_FILE),
                preview_duration(&subtitles),
                &preview_path,
            ),
        };
        command.run()?;
        if !preview_path.is_file() {
            return Err(CutlineError::InvalidProject(format!(
                "preview render did not create expected output: {preview_path}"
            )));
        }
        Some(command)
    } else {
        None
    };
    let preview_asset = if options.render_preview {
        Some(ManifestAssetReference {
            path: Utf8PathBuf::from(PREVIEW_FILE),
            fingerprint: file_fingerprint(&preview_path)?,
        })
    } else {
        None
    };

    let manifest = DraftManifest {
        schema_version: DRAFT_SCHEMA_VERSION.to_owned(),
        workflow: "StoryHighlightVideo".to_owned(),
        draft_id: story.name.clone(),
        platform_profile: story.platform.clone(),
        story: ManifestStory {
            source: ManifestStorySource {
                path: story.source.clone(),
                start_line: story.start_line,
                end_line: story.end_line,
                fingerprint: file_fingerprint(&project.project_dir.join(&story.source))?,
            },
            engagement_angle: story.engagement_angle.clone(),
        },
        visual: ManifestVisual {
            background: ManifestAssetReference {
                path: story.background.clone(),
                fingerprint: file_fingerprint(&project.project_dir.join(&story.background))?,
            },
        },
        output: ManifestOutput {
            path: project_reference_path(&project.project_dir, &project.output_path),
        },
        generated_assets: GeneratedAssets {
            narration: ManifestAssetReference {
                path: Utf8PathBuf::from(NARRATION_FILE),
                fingerprint: bytes_fingerprint(narration_text.as_bytes()),
            },
            subtitles: ManifestAssetReference {
                path: Utf8PathBuf::from(SUBTITLES_FILE),
                fingerprint: bytes_fingerprint(subtitles_text.as_bytes()),
            },
            preview: preview_asset,
            voiceover: voice_step.asset.clone(),
        },
        subtitle_style: default_subtitle_style(&story.platform),
        short_video_drafts: short_video_drafts.clone(),
    };
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    fs::write(
        package_path.join("draft.json"),
        format!("{manifest_json}\n"),
    )?;
    let reference_map = ReferenceMap {
        schema_version: DRAFT_SCHEMA_VERSION.to_owned(),
        draft_id: story.name.clone(),
        references: generated_reference_entries(&short_video_drafts[0], &subtitles),
        pipeline_step_runs: pipeline_step_runs(
            story,
            &voice_step.step_run,
            preview_command.as_ref(),
        ),
    };
    let reference_json = serde_json::to_string_pretty(&reference_map)?;
    fs::write(
        package_path.join("references.json"),
        format!("{reference_json}\n"),
    )?;

    Ok(DraftPackageSummary {
        draft_id: story.name.clone(),
        package_path,
    })
}

fn pipeline_step_runs(
    story: &StoryVideo,
    voice_step: &PipelineStepRun,
    preview_command: Option<&FfmpegCommand>,
) -> Vec<PipelineStepRun> {
    let mut steps = vec![
        PipelineStepRun {
            step: "ingest".to_owned(),
            status: "completed".to_owned(),
            provider: "local".to_owned(),
            parameters: serde_json::json!({}),
            outputs: vec![],
        },
        PipelineStepRun {
            step: "script".to_owned(),
            status: "completed".to_owned(),
            provider: "local_script_provider".to_owned(),
            parameters: serde_json::json!({
                "engagement_angle": story.engagement_angle,
                "source": story.source,
                "start_line": story.start_line,
                "end_line": story.end_line,
            }),
            outputs: vec!["short_video_drafts[0]".to_owned()],
        },
        PipelineStepRun {
            step: "subtitle".to_owned(),
            status: "completed".to_owned(),
            provider: "local_subtitle_provider".to_owned(),
            parameters: serde_json::json!({
                "max_chars_per_line": SUBTITLE_MAX_CHARS_PER_LINE,
                "cue_duration_millis": SUBTITLE_CUE_DURATION_MILLIS,
                "subtitle_style": "douyin_default",
            }),
            outputs: vec![NARRATION_FILE.to_owned(), SUBTITLES_FILE.to_owned()],
        },
        voice_step.clone(),
    ];

    if let Some(command) = preview_command {
        steps.push(PipelineStepRun {
            step: "preview".to_owned(),
            status: "completed".to_owned(),
            provider: "ffmpeg".to_owned(),
            parameters: serde_json::json!({
                "command": command.display(),
                "program": command.program,
                "args": command.args,
                "output_path": PREVIEW_FILE,
            }),
            outputs: vec![PREVIEW_FILE.to_owned()],
        });
    }

    steps.push(PipelineStepRun {
        step: "draft_package".to_owned(),
        status: "completed".to_owned(),
        provider: "local".to_owned(),
        parameters: serde_json::json!({}),
        outputs: vec!["draft.json".to_owned(), "references.json".to_owned()],
    });
    steps
}

fn selected_voice_provider(
    story: &StoryVideo,
    requested: VoiceProviderConfig,
) -> VoiceProviderConfig {
    match requested {
        VoiceProviderConfig::None => match story.voice_provider.as_deref() {
            Some("diamoetts") => VoiceProviderConfig::DiamoETts,
            _ => VoiceProviderConfig::None,
        },
        provider => provider,
    }
}

struct VoiceStepResult {
    asset: Option<ManifestAssetReference>,
    step_run: PipelineStepRun,
}

fn run_voice_step(
    package_path: &camino::Utf8Path,
    provider: &VoiceProviderConfig,
) -> Result<VoiceStepResult> {
    match provider {
        VoiceProviderConfig::None => Ok(VoiceStepResult {
            asset: None,
            step_run: PipelineStepRun {
                step: "voice".to_owned(),
                status: "skipped".to_owned(),
                provider: "none".to_owned(),
                parameters: serde_json::json!({
                    "reason": "no voice provider configured",
                }),
                outputs: vec![],
            },
        }),
        VoiceProviderConfig::DiamoETts => Ok(VoiceStepResult {
            asset: None,
            step_run: PipelineStepRun {
                step: "voice".to_owned(),
                status: "skipped".to_owned(),
                provider: "diamoetts".to_owned(),
                parameters: serde_json::json!({
                    "reason": "diamoetts provider declared but no local model configured",
                    "optional": true,
                }),
                outputs: vec![],
            },
        }),
        VoiceProviderConfig::Test { audio_bytes } => {
            let path = package_path.join(VOICEOVER_FILE);
            fs::write(&path, audio_bytes)?;
            Ok(VoiceStepResult {
                asset: Some(ManifestAssetReference {
                    path: Utf8PathBuf::from(VOICEOVER_FILE),
                    fingerprint: file_fingerprint(&path)?,
                }),
                step_run: PipelineStepRun {
                    step: "voice".to_owned(),
                    status: "completed".to_owned(),
                    provider: "test_voice_provider".to_owned(),
                    parameters: serde_json::json!({
                        "source": NARRATION_FILE,
                    }),
                    outputs: vec![VOICEOVER_FILE.to_owned()],
                },
            })
        }
    }
}

fn generated_reference_entries(
    draft: &ShortVideoDraft,
    subtitles: &[SubtitleCue],
) -> Vec<ReferenceEntry> {
    let mut entries = Vec::new();
    entries.push(reference_entry(format!("{}.hook", draft.id), &draft.hook));
    for (index, beat) in draft.content_beats.iter().enumerate() {
        entries.push(reference_entry(
            format!("{}.content_beats[{index}]", draft.id),
            beat,
        ));
    }
    entries.push(reference_entry(
        format!("{}.adapted_narration", draft.id),
        &draft.adapted_narration,
    ));
    entries.push(reference_entry(
        format!("{NARRATION_FILE}:1"),
        &draft.adapted_narration,
    ));
    entries.extend(subtitles.iter().map(|cue| {
        reference_entry(
            format!("{SUBTITLES_FILE}:{}", cue.index),
            &draft.adapted_narration,
        )
    }));
    entries.push(reference_entry(
        format!("{}.ending", draft.id),
        &draft.ending,
    ));
    entries
}

fn reference_entry(target: String, text: &ScriptText) -> ReferenceEntry {
    ReferenceEntry {
        target,
        source: text.source_reference.source.clone(),
        start_line: text.source_reference.start_line,
        end_line: text.source_reference.end_line,
    }
}

fn generate_local_script_draft(
    project_dir: &camino::Utf8Path,
    story: &StoryVideo,
) -> Result<ShortVideoDraft> {
    let source_path = project_dir.join(&story.source);
    let source = fs::read_to_string(&source_path)?;
    let mut selected_lines = source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let line_number = index + 1;
            (story.start_line..=story.end_line)
                .contains(&line_number)
                .then(|| line.trim())
        })
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if selected_lines.is_empty() {
        selected_lines.push("选定文本暂未包含非空内容");
    }

    let first_line = selected_lines.first().copied().unwrap_or("");
    let source_reference = SourceReference {
        source: story.source.clone(),
        start_line: story.start_line,
        end_line: story.end_line,
    };
    let beat_count = selected_lines.len().clamp(3, 5);
    let beats = (0..beat_count)
        .map(|index| {
            let line = selected_lines[index % selected_lines.len()];
            ScriptText {
                text: format!(
                    "围绕{}推进第{}个看点：{line}",
                    story.engagement_angle,
                    index + 1
                ),
                source_reference: source_reference.clone(),
            }
        })
        .collect::<Vec<_>>();
    let beat_texts = beats
        .iter()
        .map(|beat| beat.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");

    Ok(ShortVideoDraft {
        id: format!("{}-001", story.name),
        hook: ScriptText {
            text: format!("{}反转来了：{first_line}", story.engagement_angle),
            source_reference: source_reference.clone(),
        },
        content_beats: beats,
        adapted_narration: ScriptText {
            text: beat_texts,
            source_reference: source_reference.clone(),
        },
        ending: ScriptText {
            text: "真正的爆点，才刚刚开始。".to_owned(),
            source_reference,
        },
    })
}

#[derive(Debug, Clone)]
struct SubtitleCue {
    index: usize,
    start_millis: u64,
    end_millis: u64,
    text: String,
}

fn generate_subtitles(narration: &str) -> Vec<SubtitleCue> {
    split_subtitle_text(narration, SUBTITLE_MAX_CHARS_PER_LINE)
        .into_iter()
        .enumerate()
        .map(|(index, text)| {
            let start_millis = (index as u64) * SUBTITLE_CUE_DURATION_MILLIS;
            SubtitleCue {
                index: index + 1,
                start_millis,
                end_millis: start_millis + SUBTITLE_CUE_DURATION_MILLIS,
                text,
            }
        })
        .collect()
}

fn preview_duration(subtitles: &[SubtitleCue]) -> TimeValue {
    TimeValue::from_millis(
        subtitles
            .last()
            .map(|subtitle| subtitle.end_millis)
            .unwrap_or(SUBTITLE_CUE_DURATION_MILLIS),
    )
}

fn split_subtitle_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for character in text.chars().filter(|character| !character.is_whitespace()) {
        current.push(character);
        if current.chars().count() >= max_chars || "，。！？；：".contains(character) {
            lines.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn format_srt(cues: &[SubtitleCue]) -> String {
    let mut output = String::new();
    for cue in cues {
        output.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            cue.index,
            format_srt_timestamp(cue.start_millis),
            format_srt_timestamp(cue.end_millis),
            cue.text
        ));
    }
    output
}

fn format_srt_timestamp(millis: u64) -> String {
    let hours = millis / 3_600_000;
    let minutes = (millis % 3_600_000) / 60_000;
    let seconds = (millis % 60_000) / 1_000;
    let millis = millis % 1_000;
    format!("{hours:02}:{minutes:02}:{seconds:02},{millis:03}")
}

fn default_subtitle_style(platform: &str) -> SubtitleStyle {
    SubtitleStyle {
        platform: platform.to_owned(),
        aspect_ratio: "9:16".to_owned(),
        max_chars_per_line: SUBTITLE_MAX_CHARS_PER_LINE,
        position: "bottom_safe_area".to_owned(),
        safe_area_bottom_percent: 12,
    }
}

fn project_reference_path(project_dir: &camino::Utf8Path, path: &camino::Utf8Path) -> Utf8PathBuf {
    path.strip_prefix(project_dir)
        .map(camino::Utf8Path::to_path_buf)
        .unwrap_or_else(|_| path.to_path_buf())
}

fn file_fingerprint(path: &camino::Utf8Path) -> Result<String> {
    let bytes = fs::read(path)?;
    Ok(bytes_fingerprint(&bytes))
}

fn bytes_fingerprint(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

/// Parse a text file into segments based on line numbers.
pub fn parse_text_segments(file_path: &str, segments: &[(usize, usize)]) -> Vec<TextSegment> {
    let Ok(content) = fs::read_to_string(file_path) else {
        return Vec::new();
    };
    let lines = content.lines().collect::<Vec<_>>();

    segments
        .iter()
        .filter_map(|(start_line, end_line)| {
            if *start_line == 0 || end_line < start_line || *start_line > lines.len() {
                return None;
            }

            let end_line = (*end_line).min(lines.len());
            let content = lines[*start_line - 1..end_line]
                .iter()
                .map(|line| line.trim_end())
                .collect::<Vec<_>>()
                .join("\n");
            (!content.trim().is_empty()).then(|| TextSegment {
                start_line: *start_line,
                end_line,
                content,
                description: None,
            })
        })
        .collect()
}

/// Extract engaging content from text using keyword matching.
pub fn extract_engaging_content(text: &str) -> Vec<EngagingSegment> {
    const KEYWORDS: &[(&str, f64)] = &[
        ("反转", 2.0),
        ("复仇", 1.8),
        ("真相", 1.7),
        ("秘密", 1.6),
        ("背叛", 1.6),
        ("危机", 1.5),
        ("震惊", 1.5),
        ("悬念", 1.4),
        ("冲突", 1.4),
        ("命运", 1.2),
        ("突然", 1.2),
        ("revealed", 1.5),
        ("secret", 1.4),
        ("betrayal", 1.4),
        ("revenge", 1.4),
        ("danger", 1.2),
    ];

    let mut segments = Vec::new();
    let mut offset = 0;
    for raw_sentence in text.split_inclusive(['。', '！', '？', '.', '!', '?']) {
        let sentence = raw_sentence.trim();
        let start_offset = offset + raw_sentence.find(sentence).unwrap_or(0);
        let end_offset = start_offset + sentence.len();
        offset += raw_sentence.len();
        if sentence.is_empty() {
            continue;
        }

        let mut matched_keywords = Vec::new();
        let mut score = 0.0;
        let lowercase = sentence.to_ascii_lowercase();
        for (keyword, weight) in KEYWORDS {
            let matched = if keyword.is_ascii() {
                lowercase.contains(keyword)
            } else {
                sentence.contains(keyword)
            };
            if matched {
                matched_keywords.push((*keyword).to_owned());
                score += weight;
            }
        }

        if score > 0.0 {
            segments.push(EngagingSegment {
                start_offset,
                end_offset,
                content: sentence.to_owned(),
                score,
                keywords: matched_keywords,
            });
        }
    }

    segments.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.start_offset.cmp(&right.start_offset))
    });
    segments
}

/// Generate deterministic placeholder voiceover bytes for local draft tests.
///
/// Real TTS provider discovery and synthesis are tracked separately; this
/// helper intentionally returns an auditable placeholder payload instead of
/// pretending to contact a remote service.
pub async fn generate_voiceover(text: &str, voice: &str, speed: f32) -> Vec<u8> {
    if text.trim().is_empty() || voice.trim().is_empty() || !speed.is_finite() || speed <= 0.0 {
        return Vec::new();
    }

    format!(
        "cutline-placeholder-voiceover\nvoice={}\nspeed={:.2}\ntext_sha256={}\n",
        voice.trim(),
        speed,
        bytes_fingerprint(text.as_bytes())
    )
    .into_bytes()
}

/// A segment of text extracted from a novel.
#[derive(Debug, Clone)]
pub struct TextSegment {
    pub start_line: usize,
    pub end_line: usize,
    pub content: String,
    pub description: Option<String>,
}

/// A segment identified as "engaging" based on content analysis.
#[derive(Debug, Clone)]
pub struct EngagingSegment {
    pub start_offset: usize,
    pub end_offset: usize,
    pub content: String,
    pub score: f64,
    pub keywords: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use camino::Utf8PathBuf;

    use crate::config::{OutputConfig, ProjectConfig, RenderConfig};
    use crate::validate::{ValidationOptions, normalize_project_with_options};

    struct DraftFixture {
        root: std::path::PathBuf,
        project: NormalizedProject,
    }

    impl DraftFixture {
        fn new(name: &str) -> Self {
            Self::with_source(name, "第一行\n第二行\n第三行\n")
        }

        fn with_source(name: &str, source: &str) -> Self {
            let root = std::env::temp_dir()
                .join(format!("cutline-story-draft-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(root.join("stories")).unwrap();
            fs::create_dir_all(root.join("assets")).unwrap();
            fs::write(root.join("stories/demo.txt"), source).unwrap();
            fs::write(root.join("assets/bg.mp4"), "not real media").unwrap();

            let end_line = source.lines().count();
            let project_path = Utf8PathBuf::from_path_buf(root.join("project.toml")).unwrap();
            let project = normalize_project_with_options(
                &project_path,
                ProjectConfig {
                    output: OutputConfig {
                        path: Utf8PathBuf::from("dist/story.mp4"),
                    },
                    input: BTreeMap::new(),
                    render: RenderConfig::default(),
                    clips: vec![],
                    auto_cuts: vec![],
                    story_videos: vec![StoryVideoConfig {
                        name: "demo".to_owned(),
                        source: Utf8PathBuf::from("stories/demo.txt"),
                        start_line: 1,
                        end_line,
                        engagement_angle: "reversal".to_owned(),
                        background: Utf8PathBuf::from("assets/bg.mp4"),
                        voice_provider: None,
                        platform: "douyin".to_owned(),
                    }],
                },
                ValidationOptions {
                    require_inputs: true,
                    probe_media: false,
                },
            )
            .unwrap();

            Self { root, project }
        }
    }

    impl Drop for DraftFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn parse_srt_test_timestamp(timestamp: &str) -> u64 {
        let (clock, millis) = timestamp.split_once(',').unwrap();
        let mut parts = clock.split(':').map(|part| part.parse::<u64>().unwrap());
        let hours = parts.next().unwrap();
        let minutes = parts.next().unwrap();
        let seconds = parts.next().unwrap();
        hours * 3_600_000 + minutes * 60_000 + seconds * 1_000 + millis.parse::<u64>().unwrap()
    }

    struct NoopWake;

    impl std::task::Wake for NoopWake {
        fn wake(self: std::sync::Arc<Self>) {}
    }

    fn futures_like_block_on<F: std::future::Future>(future: F) -> F::Output {
        let waker = std::task::Waker::from(std::sync::Arc::new(NoopWake));
        let mut context = std::task::Context::from_waker(&waker);
        let mut future = std::pin::pin!(future);

        loop {
            match future.as_mut().poll(&mut context) {
                std::task::Poll::Ready(output) => return output,
                std::task::Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    fn write_fake_ffmpeg(root: &std::path::Path) -> Utf8PathBuf {
        let path = root.join("ffmpeg");
        fs::write(
            &path,
            "#!/bin/sh\nout=\"\"\nfor arg do\n  out=\"$arg\"\ndone\nprintf preview > \"$out\"\n",
        )
        .unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).unwrap();
        Utf8PathBuf::from_path_buf(path).unwrap()
    }

    #[test]
    fn generates_story_video_from_config() {
        let config = StoryVideoConfig {
            name: "test".to_string(),
            source: "stories/chapter1.txt".into(),
            start_line: 10,
            end_line: 50,
            engagement_angle: "reversal".to_string(),
            background: "assets/bg.mp4".into(),
            voice_provider: None,
            platform: "douyin".to_string(),
        };

        let story_video = generate_story_video(&config);

        assert_eq!(story_video.name, "test");
        assert_eq!(story_video.engagement_angle, "reversal");
    }

    #[test]
    fn parses_text_segments_from_line_ranges() {
        let root =
            std::env::temp_dir().join(format!("cutline-story-segments-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let path = root.join("story.txt");
        fs::write(&path, "第一行\n第二行\n第三行\n第四行\n").unwrap();

        let segments = parse_text_segments(path.to_str().unwrap(), &[(2, 3), (4, 99), (0, 1)]);

        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].start_line, 2);
        assert_eq!(segments[0].end_line, 3);
        assert_eq!(segments[0].content, "第二行\n第三行");
        assert_eq!(segments[1].start_line, 4);
        assert_eq!(segments[1].end_line, 4);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn extracts_engaging_content_by_keyword_score() {
        let segments =
            extract_engaging_content("平静开场。反转来了！A secret betrayal was revealed.");

        assert_eq!(segments.len(), 2);
        assert!(segments[0].score >= segments[1].score);
        assert!(segments[0].content.contains("secret"));
        assert_eq!(
            segments[0].keywords,
            vec![
                "revealed".to_owned(),
                "secret".to_owned(),
                "betrayal".to_owned()
            ]
        );
        assert!(segments[1].keywords.contains(&"反转".to_owned()));
    }

    #[test]
    fn generate_voiceover_returns_deterministic_placeholder_payload() {
        let payload = futures_like_block_on(generate_voiceover("hello", "test-voice", 1.25));
        let text = String::from_utf8(payload).unwrap();

        assert!(text.contains("cutline-placeholder-voiceover"));
        assert!(text.contains("voice=test-voice"));
        assert!(text.contains("speed=1.25"));
        assert!(text.contains("text_sha256=sha256:"));

        assert!(futures_like_block_on(generate_voiceover("", "test-voice", 1.0)).is_empty());
    }

    #[test]
    fn generates_reviewable_draft_package_skeleton() {
        let fixture = DraftFixture::new("skeleton");

        let summary = generate_reviewable_draft_package(&fixture.project).unwrap();

        assert_eq!(summary.draft_id, "demo");
        assert!(summary.package_path.ends_with(".cutline/drafts/demo"));
        assert!(summary.package_path.join("draft.json").is_file());
        assert!(summary.package_path.join("references.json").is_file());
        assert!(summary.package_path.join("assets").is_dir());
    }

    #[test]
    fn draft_manifest_records_story_package_metadata() {
        let fixture = DraftFixture::new("manifest");

        let summary = generate_reviewable_draft_package(&fixture.project).unwrap();
        let manifest: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(summary.package_path.join("draft.json")).unwrap(),
        )
        .unwrap();

        assert_eq!(manifest["schema_version"], DRAFT_SCHEMA_VERSION);
        assert_eq!(manifest["workflow"], "StoryHighlightVideo");
        assert_eq!(manifest["draft_id"], "demo");
        assert_eq!(manifest["platform_profile"], "douyin");
        assert_eq!(manifest["story"]["source"]["path"], "stories/demo.txt");
        assert_eq!(manifest["story"]["source"]["start_line"], 1);
        assert_eq!(manifest["story"]["source"]["end_line"], 3);
        assert_eq!(manifest["story"]["engagement_angle"], "reversal");
        assert_eq!(manifest["visual"]["background"]["path"], "assets/bg.mp4");
        assert_eq!(manifest["output"]["path"], "dist/story.mp4");
    }

    #[test]
    fn draft_manifest_records_source_asset_fingerprints() {
        let fixture = DraftFixture::new("fingerprint");

        let summary = generate_reviewable_draft_package(&fixture.project).unwrap();
        let manifest: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(summary.package_path.join("draft.json")).unwrap(),
        )
        .unwrap();

        let source_fingerprint = manifest["story"]["source"]["fingerprint"].as_str().unwrap();
        let background_fingerprint = manifest["visual"]["background"]["fingerprint"]
            .as_str()
            .unwrap();

        assert!(source_fingerprint.starts_with("sha256:"));
        assert!(background_fingerprint.starts_with("sha256:"));
        assert_ne!(source_fingerprint, background_fingerprint);
        assert!(!summary.package_path.join("assets/demo.txt").exists());
        assert!(!summary.package_path.join("assets/bg.mp4").exists());
    }

    #[test]
    fn reference_map_records_draft_package_step_runs() {
        let fixture = DraftFixture::new("step-runs");

        let summary = generate_reviewable_draft_package(&fixture.project).unwrap();
        let reference_map: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(summary.package_path.join("references.json")).unwrap(),
        )
        .unwrap();

        assert_eq!(reference_map["schema_version"], DRAFT_SCHEMA_VERSION);
        assert_eq!(reference_map["draft_id"], "demo");
        let steps = reference_map["pipeline_step_runs"].as_array().unwrap();
        let ingest_step = steps
            .iter()
            .find(|step| step["step"] == "ingest")
            .expect("ingest step run");
        assert_eq!(ingest_step["status"], "completed");
        assert_eq!(ingest_step["provider"], "local");

        let package_step = steps
            .iter()
            .find(|step| step["step"] == "draft_package")
            .expect("draft package step run");
        assert_eq!(package_step["status"], "completed");
        assert_eq!(package_step["outputs"][0], "draft.json");
        assert_eq!(package_step["outputs"][1], "references.json");
    }

    #[test]
    fn draft_manifest_records_local_script_short_video_draft() {
        let fixture = DraftFixture::new("local-script");

        let summary = generate_reviewable_draft_package(&fixture.project).unwrap();
        let manifest: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(summary.package_path.join("draft.json")).unwrap(),
        )
        .unwrap();

        let drafts = manifest["short_video_drafts"].as_array().unwrap();
        assert_eq!(drafts.len(), 1);
        let draft = &drafts[0];
        assert_eq!(draft["id"], "demo-001");
        assert!(draft["hook"]["text"].as_str().unwrap().contains("reversal"));
        let beats = draft["content_beats"].as_array().unwrap();
        assert!((3..=5).contains(&beats.len()));
        assert!(
            !draft["adapted_narration"]["text"]
                .as_str()
                .unwrap()
                .is_empty()
        );
        assert!(!draft["ending"]["text"].as_str().unwrap().is_empty());
    }

    #[test]
    fn draft_package_writes_narration_text_from_adapted_narration() {
        let fixture = DraftFixture::new("narration-text");

        let summary = generate_reviewable_draft_package(&fixture.project).unwrap();
        let manifest: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(summary.package_path.join("draft.json")).unwrap(),
        )
        .unwrap();
        let narration = fs::read_to_string(summary.package_path.join("narration.txt")).unwrap();

        assert_eq!(
            narration.trim_end(),
            manifest["short_video_drafts"][0]["adapted_narration"]["text"]
                .as_str()
                .unwrap()
        );
    }

    #[test]
    fn draft_package_writes_readable_srt_subtitles() {
        let fixture = DraftFixture::with_source(
            "readable-srt",
            "第一行出现了很长很长的冲突信息需要拆短\n第二行继续推动悬念\n第三行给出新的反转\n",
        );

        let summary = generate_reviewable_draft_package(&fixture.project).unwrap();
        let srt = fs::read_to_string(summary.package_path.join("subtitles.srt")).unwrap();
        let blocks = srt.trim_end().split("\n\n").collect::<Vec<_>>();

        assert!(!blocks.is_empty());
        let first_block = blocks[0].lines().collect::<Vec<_>>();
        assert_eq!(first_block[0], "1");
        assert!(first_block[1].contains(" --> "));
        assert!(
            blocks
                .iter()
                .flat_map(|block| block.lines().skip(2))
                .all(|line| line.chars().count() <= 18)
        );
    }

    #[test]
    fn draft_package_writes_srt_with_monotonic_timings() {
        let fixture = DraftFixture::new("monotonic-srt");

        let summary = generate_reviewable_draft_package(&fixture.project).unwrap();
        let srt = fs::read_to_string(summary.package_path.join("subtitles.srt")).unwrap();
        let ranges = srt
            .trim_end()
            .split("\n\n")
            .map(|block| {
                let timing = block.lines().nth(1).unwrap();
                let (start, end) = timing.split_once(" --> ").unwrap();
                (
                    parse_srt_test_timestamp(start),
                    parse_srt_test_timestamp(end),
                )
            })
            .collect::<Vec<_>>();

        assert!(ranges.iter().all(|(start, end)| start < end));
        assert!(ranges.windows(2).all(|pair| pair[0].1 <= pair[1].0));
    }

    #[test]
    fn draft_manifest_records_generated_assets_and_subtitle_style() {
        let fixture = DraftFixture::new("subtitle-manifest");

        let summary = generate_reviewable_draft_package(&fixture.project).unwrap();
        let manifest: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(summary.package_path.join("draft.json")).unwrap(),
        )
        .unwrap();

        assert_eq!(
            manifest["generated_assets"]["narration"]["path"],
            "narration.txt"
        );
        assert_eq!(
            manifest["generated_assets"]["subtitles"]["path"],
            "subtitles.srt"
        );
        assert!(
            manifest["generated_assets"]["narration"]["fingerprint"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
        assert!(
            manifest["generated_assets"]["subtitles"]["fingerprint"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
        assert_eq!(manifest["subtitle_style"]["platform"], "douyin");
        assert_eq!(manifest["subtitle_style"]["aspect_ratio"], "9:16");
        assert_eq!(manifest["subtitle_style"]["max_chars_per_line"], 18);
        assert_eq!(manifest["subtitle_style"]["position"], "bottom_safe_area");
    }

    #[test]
    fn draft_manifest_records_preview_asset_when_preview_render_requested() {
        let fixture = DraftFixture::new("preview-manifest");
        let ffmpeg = write_fake_ffmpeg(&fixture.root);

        let summary = generate_reviewable_draft_package_with_options(
            &fixture.project,
            DraftPackageOptions {
                render_preview: true,
                ffmpeg_program: ffmpeg.to_string(),
                voice_provider: VoiceProviderConfig::None,
            },
        )
        .unwrap();
        let manifest: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(summary.package_path.join("draft.json")).unwrap(),
        )
        .unwrap();

        assert!(summary.package_path.join("preview.mp4").is_file());
        assert_eq!(
            manifest["generated_assets"]["preview"]["path"],
            "preview.mp4"
        );
        assert!(
            manifest["generated_assets"]["preview"]["fingerprint"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
    }

    #[test]
    fn reference_map_records_preview_step_run_when_preview_render_requested() {
        let fixture = DraftFixture::new("preview-step-run");
        let ffmpeg = write_fake_ffmpeg(&fixture.root);

        let summary = generate_reviewable_draft_package_with_options(
            &fixture.project,
            DraftPackageOptions {
                render_preview: true,
                ffmpeg_program: ffmpeg.to_string(),
                voice_provider: VoiceProviderConfig::None,
            },
        )
        .unwrap();
        let reference_map: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(summary.package_path.join("references.json")).unwrap(),
        )
        .unwrap();
        let steps = reference_map["pipeline_step_runs"].as_array().unwrap();
        let preview_step = steps
            .iter()
            .find(|step| step["step"] == "preview")
            .expect("preview step run");

        assert_eq!(preview_step["status"], "completed");
        assert_eq!(preview_step["provider"], "ffmpeg");
        assert_eq!(preview_step["parameters"]["output_path"], "preview.mp4");
        assert!(
            preview_step["parameters"]["command"]
                .as_str()
                .unwrap()
                .contains("subtitles.srt")
        );
        assert_eq!(preview_step["outputs"][0], "preview.mp4");
    }

    #[test]
    fn local_script_draft_keeps_three_to_five_beats_for_short_source_ranges() {
        let fixture = DraftFixture::with_source("short-script-range", "第一行\n第二行\n");

        let summary = generate_reviewable_draft_package(&fixture.project).unwrap();
        let manifest: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(summary.package_path.join("draft.json")).unwrap(),
        )
        .unwrap();

        let beats = manifest["short_video_drafts"][0]["content_beats"]
            .as_array()
            .unwrap();
        assert!((3..=5).contains(&beats.len()));
    }

    #[test]
    fn local_script_draft_records_source_references() {
        let fixture = DraftFixture::new("script-references");

        let summary = generate_reviewable_draft_package(&fixture.project).unwrap();
        let manifest: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(summary.package_path.join("draft.json")).unwrap(),
        )
        .unwrap();
        let draft = &manifest["short_video_drafts"][0];

        for path in [
            &draft["hook"],
            &draft["content_beats"][0],
            &draft["adapted_narration"],
            &draft["ending"],
        ] {
            assert_eq!(path["source_reference"]["source"], "stories/demo.txt");
            assert_eq!(path["source_reference"]["start_line"], 1);
            assert_eq!(path["source_reference"]["end_line"], 3);
        }
    }

    #[test]
    fn reference_map_records_script_source_references() {
        let fixture = DraftFixture::new("script-reference-map");

        let summary = generate_reviewable_draft_package(&fixture.project).unwrap();
        let reference_map: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(summary.package_path.join("references.json")).unwrap(),
        )
        .unwrap();

        let references = reference_map["references"].as_array().unwrap();
        assert!(
            references
                .iter()
                .any(|reference| reference["target"] == "demo-001.hook")
        );
        assert!(
            references
                .iter()
                .any(|reference| reference["target"] == "demo-001.content_beats[0]")
        );
        assert!(
            references
                .iter()
                .any(|reference| reference["target"] == "demo-001.adapted_narration")
        );
        assert!(
            references
                .iter()
                .any(|reference| reference["target"] == "demo-001.ending")
        );
        assert!(references.iter().all(|reference| {
            reference["source"] == "stories/demo.txt"
                && reference["start_line"] == 1
                && reference["end_line"] == 3
        }));
    }

    #[test]
    fn reference_map_records_narration_and_subtitle_source_references() {
        let fixture = DraftFixture::new("subtitle-reference-map");

        let summary = generate_reviewable_draft_package(&fixture.project).unwrap();
        let reference_map: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(summary.package_path.join("references.json")).unwrap(),
        )
        .unwrap();
        let references = reference_map["references"].as_array().unwrap();

        for target in ["narration.txt:1", "subtitles.srt:1"] {
            let reference = references
                .iter()
                .find(|reference| reference["target"] == target)
                .expect("generated text reference");
            assert_eq!(reference["source"], "stories/demo.txt");
            assert_eq!(reference["start_line"], 1);
            assert_eq!(reference["end_line"], 3);
        }
    }

    #[test]
    fn reference_map_records_local_script_step_run() {
        let fixture = DraftFixture::new("script-step-run");

        let summary = generate_reviewable_draft_package(&fixture.project).unwrap();
        let reference_map: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(summary.package_path.join("references.json")).unwrap(),
        )
        .unwrap();
        let steps = reference_map["pipeline_step_runs"].as_array().unwrap();
        let script_step = steps
            .iter()
            .find(|step| step["step"] == "script")
            .expect("script step run");

        assert_eq!(script_step["status"], "completed");
        assert_eq!(script_step["provider"], "local_script_provider");
        assert_eq!(script_step["parameters"]["engagement_angle"], "reversal");
        assert_eq!(script_step["parameters"]["source"], "stories/demo.txt");
        assert_eq!(script_step["outputs"][0], "short_video_drafts[0]");
    }

    #[test]
    fn reference_map_records_subtitle_step_run() {
        let fixture = DraftFixture::new("subtitle-step-run");

        let summary = generate_reviewable_draft_package(&fixture.project).unwrap();
        let reference_map: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(summary.package_path.join("references.json")).unwrap(),
        )
        .unwrap();
        let steps = reference_map["pipeline_step_runs"].as_array().unwrap();
        let subtitle_step = steps
            .iter()
            .find(|step| step["step"] == "subtitle")
            .expect("subtitle step run");

        assert_eq!(subtitle_step["status"], "completed");
        assert_eq!(subtitle_step["provider"], "local_subtitle_provider");
        assert_eq!(subtitle_step["parameters"]["max_chars_per_line"], 18);
        assert_eq!(subtitle_step["parameters"]["cue_duration_millis"], 2000);
        assert_eq!(subtitle_step["outputs"][0], "narration.txt");
        assert_eq!(subtitle_step["outputs"][1], "subtitles.srt");
    }

    #[test]
    fn reference_map_records_skipped_voice_step_when_no_voice_provider_configured() {
        let fixture = DraftFixture::new("voice-skipped");

        let summary = generate_reviewable_draft_package(&fixture.project).unwrap();
        let reference_map: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(summary.package_path.join("references.json")).unwrap(),
        )
        .unwrap();
        let steps = reference_map["pipeline_step_runs"].as_array().unwrap();
        let voice_step = steps
            .iter()
            .find(|step| step["step"] == "voice")
            .expect("voice step run");

        assert_eq!(voice_step["status"], "skipped");
        assert_eq!(voice_step["provider"], "none");
        assert_eq!(
            voice_step["parameters"]["reason"],
            "no voice provider configured"
        );
        assert!(voice_step["outputs"].as_array().unwrap().is_empty());
    }

    #[test]
    fn reference_map_records_diamoetts_voice_provider_as_skipped_without_local_model() {
        let mut fixture = DraftFixture::new("voice-diamoetts-skipped");
        fixture.project.story_videos[0].voice_provider = Some("diamoetts".to_owned());

        let summary = generate_reviewable_draft_package(&fixture.project).unwrap();
        let reference_map: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(summary.package_path.join("references.json")).unwrap(),
        )
        .unwrap();
        let steps = reference_map["pipeline_step_runs"].as_array().unwrap();
        let voice_step = steps
            .iter()
            .find(|step| step["step"] == "voice")
            .expect("voice step run");

        assert_eq!(voice_step["status"], "skipped");
        assert_eq!(voice_step["provider"], "diamoetts");
        assert_eq!(
            voice_step["parameters"]["reason"],
            "diamoetts provider declared but no local model configured"
        );
    }

    #[test]
    fn draft_manifest_records_voiceover_asset_when_test_voice_provider_completes() {
        let fixture = DraftFixture::new("voice-completed-manifest");

        let summary = generate_reviewable_draft_package_with_options(
            &fixture.project,
            DraftPackageOptions {
                render_preview: false,
                ffmpeg_program: "ffmpeg".to_owned(),
                voice_provider: VoiceProviderConfig::Test {
                    audio_bytes: b"fake wav".to_vec(),
                },
            },
        )
        .unwrap();
        let manifest: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(summary.package_path.join("draft.json")).unwrap(),
        )
        .unwrap();

        assert_eq!(
            fs::read(summary.package_path.join("voiceover.wav")).unwrap(),
            b"fake wav"
        );
        assert_eq!(
            manifest["generated_assets"]["voiceover"]["path"],
            "voiceover.wav"
        );
        assert!(
            manifest["generated_assets"]["voiceover"]["fingerprint"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
    }

    #[test]
    fn reference_map_records_completed_voice_step_when_test_voice_provider_completes() {
        let fixture = DraftFixture::new("voice-completed-step");

        let summary = generate_reviewable_draft_package_with_options(
            &fixture.project,
            DraftPackageOptions {
                render_preview: false,
                ffmpeg_program: "ffmpeg".to_owned(),
                voice_provider: VoiceProviderConfig::Test {
                    audio_bytes: b"fake wav".to_vec(),
                },
            },
        )
        .unwrap();
        let reference_map: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(summary.package_path.join("references.json")).unwrap(),
        )
        .unwrap();
        let steps = reference_map["pipeline_step_runs"].as_array().unwrap();
        let voice_step = steps
            .iter()
            .find(|step| step["step"] == "voice")
            .expect("voice step run");

        assert_eq!(voice_step["status"], "completed");
        assert_eq!(voice_step["provider"], "test_voice_provider");
        assert_eq!(voice_step["outputs"][0], "voiceover.wav");
    }
}
