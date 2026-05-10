use std::fs;

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::StoryVideoConfig;
use crate::error::{CutlineError, Result};
use crate::model::{NormalizedProject, StoryVideo};

pub const DRAFT_SCHEMA_VERSION: &str = "cutline-draft-v1";

#[derive(Debug, Clone)]
pub struct DraftPackageSummary {
    pub draft_id: String,
    pub package_path: Utf8PathBuf,
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
        platform: config.platform.clone(),
    }
}

pub fn generate_reviewable_draft_package(
    project: &NormalizedProject,
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
        references: script_reference_entries(&short_video_drafts[0]),
        pipeline_step_runs: vec![
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
                step: "draft_package".to_owned(),
                status: "completed".to_owned(),
                provider: "local".to_owned(),
                parameters: serde_json::json!({}),
                outputs: vec!["draft.json".to_owned(), "references.json".to_owned()],
            },
        ],
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

fn script_reference_entries(draft: &ShortVideoDraft) -> Vec<ReferenceEntry> {
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

fn project_reference_path(project_dir: &camino::Utf8Path, path: &camino::Utf8Path) -> Utf8PathBuf {
    path.strip_prefix(project_dir)
        .map(camino::Utf8Path::to_path_buf)
        .unwrap_or_else(|_| path.to_path_buf())
}

fn file_fingerprint(path: &camino::Utf8Path) -> Result<String> {
    let bytes = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

/// Parse a text file into segments based on line numbers.
pub fn parse_text_segments(_file_path: &str, _segments: &[(usize, usize)]) -> Vec<TextSegment> {
    // TODO: Implement text file parsing
    // This would read the file and extract the specified line ranges
    vec![]
}

/// Extract engaging content from text using keyword matching.
pub fn extract_engaging_content(_text: &str) -> Vec<EngagingSegment> {
    // TODO: Implement keyword-based content extraction
    // This would look for action, emotion, and suspense keywords
    vec![]
}

/// Generate voiceover audio from text using TTS.
pub async fn generate_voiceover(_text: &str, _voice: &str, _speed: f32) -> Vec<u8> {
    // TODO: Implement TTS integration
    // This would call a TTS service (Azure, Alibaba Cloud, etc.) and return audio data
    vec![]
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

    #[test]
    fn generates_story_video_from_config() {
        let config = StoryVideoConfig {
            name: "test".to_string(),
            source: "stories/chapter1.txt".into(),
            start_line: 10,
            end_line: 50,
            engagement_angle: "reversal".to_string(),
            background: "assets/bg.mp4".into(),
            platform: "douyin".to_string(),
        };

        let story_video = generate_story_video(&config);

        assert_eq!(story_video.name, "test");
        assert_eq!(story_video.engagement_angle, "reversal");
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
}
