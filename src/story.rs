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
    pub references: Vec<serde_json::Value>,
    pub pipeline_step_runs: Vec<PipelineStepRun>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStepRun {
    pub step: String,
    pub status: String,
    pub provider: String,
    pub outputs: Vec<String>,
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
    };
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    fs::write(
        package_path.join("draft.json"),
        format!("{manifest_json}\n"),
    )?;
    let reference_map = ReferenceMap {
        schema_version: DRAFT_SCHEMA_VERSION.to_owned(),
        draft_id: story.name.clone(),
        references: vec![],
        pipeline_step_runs: vec![
            PipelineStepRun {
                step: "ingest".to_owned(),
                status: "completed".to_owned(),
                provider: "local".to_owned(),
                outputs: vec![],
            },
            PipelineStepRun {
                step: "draft_package".to_owned(),
                status: "completed".to_owned(),
                provider: "local".to_owned(),
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
            let root = std::env::temp_dir()
                .join(format!("cutline-story-draft-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(root.join("stories")).unwrap();
            fs::create_dir_all(root.join("assets")).unwrap();
            fs::write(root.join("stories/demo.txt"), "第一行\n第二行\n第三行\n").unwrap();
            fs::write(root.join("assets/bg.mp4"), "not real media").unwrap();

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
                        end_line: 3,
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
    fn reference_map_records_initial_pipeline_step_runs() {
        let fixture = DraftFixture::new("step-runs");

        let summary = generate_reviewable_draft_package(&fixture.project).unwrap();
        let reference_map: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(summary.package_path.join("references.json")).unwrap(),
        )
        .unwrap();

        assert_eq!(reference_map["schema_version"], DRAFT_SCHEMA_VERSION);
        assert_eq!(reference_map["draft_id"], "demo");
        assert!(reference_map["references"].as_array().unwrap().is_empty());
        let steps = reference_map["pipeline_step_runs"].as_array().unwrap();
        assert_eq!(steps[0]["step"], "ingest");
        assert_eq!(steps[0]["status"], "completed");
        assert_eq!(steps[0]["provider"], "local");
        assert_eq!(steps[1]["step"], "draft_package");
        assert_eq!(steps[1]["status"], "completed");
        assert_eq!(steps[1]["outputs"][0], "draft.json");
        assert_eq!(steps[1]["outputs"][1], "references.json");
    }
}
