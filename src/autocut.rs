use std::process::Command;

use camino::Utf8Path;

use crate::config::{AutoCutConfig, AutoCutRuleConfig};
use crate::error::{CutlineError, Result};
use crate::ffmpeg::probe_duration;
use crate::model::{AutoCut, AutoCutOutputMode, AutoCutRule, NormalizedProject};
use crate::time::TimeValue;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AutoCutPlan {
    pub auto_cuts: Vec<PlannedAutoCut>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlannedAutoCut {
    pub name: String,
    pub input: String,
    pub input_path: camino::Utf8PathBuf,
    pub input_duration: TimeValue,
    pub target_duration: TimeValue,
    pub clip_duration: TimeValue,
    pub min_clip_duration: TimeValue,
    pub output_mode: AutoCutOutputMode,
    pub rules: Vec<AutoCutRule>,
    pub analysis: AutoCutAnalysis,
    pub clips: Vec<AutoCutClip>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AutoCutAnalysis {
    pub scene_changes: Vec<TimeValue>,
    pub audio_regions: Vec<AutoCutAudioRegion>,
    pub fallback_used: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AutoCutAudioRegion {
    pub start: TimeValue,
    pub end: TimeValue,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AutoCutClip {
    pub index: usize,
    pub input: String,
    pub start: TimeValue,
    pub end: TimeValue,
    pub duration: TimeValue,
    pub output_start: TimeValue,
    pub output_end: TimeValue,
}

pub trait AutoCutAnalyzer {
    fn scene_changes(&self, video_path: &Utf8Path, threshold: f64) -> Vec<i64>;
    fn audio_activity(
        &self,
        video_path: &Utf8Path,
        threshold: f64,
        duration: TimeValue,
    ) -> Vec<(i64, i64)>;
    fn motion_anchors(&self, video_path: &Utf8Path, threshold: f64) -> Vec<i64>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FfmpegAutoCutAnalyzer;

impl AutoCutAnalyzer for FfmpegAutoCutAnalyzer {
    fn scene_changes(&self, video_path: &Utf8Path, threshold: f64) -> Vec<i64> {
        analyze_scene_changes_with_threshold(video_path, threshold)
    }

    fn audio_activity(
        &self,
        video_path: &Utf8Path,
        threshold: f64,
        duration: TimeValue,
    ) -> Vec<(i64, i64)> {
        analyze_audio_activity_with_threshold(video_path, threshold, Some(duration))
    }

    fn motion_anchors(&self, video_path: &Utf8Path, threshold: f64) -> Vec<i64> {
        // V1 uses ffmpeg's scene score as a visual activity proxy until a
        // dedicated motion metric is wired into the planning pipeline.
        analyze_scene_changes_with_threshold(video_path, threshold)
    }
}

/// Generate an AutoCut from its configuration.
pub fn generate_autocut(config: &AutoCutConfig) -> AutoCut {
    AutoCut {
        name: config.name.clone(),
        input: config.input.clone(),
        target_duration: config.target_duration,
        clip_duration: config.clip_duration,
        min_clip_duration: config.min_clip_duration,
        rules: config.rules.iter().map(auto_cut_rule).collect(),
        output_mode: match config.output_mode.as_str() {
            "multiple" => crate::model::AutoCutOutputMode::Multiple,
            _ => crate::model::AutoCutOutputMode::Single,
        },
    }
}

pub fn build_autocut_plan(project: &NormalizedProject) -> Result<AutoCutPlan> {
    build_autocut_plan_with_analyzer(project, &FfmpegAutoCutAnalyzer)
}

pub fn build_autocut_plan_with_analyzer(
    project: &NormalizedProject,
    analyzer: &dyn AutoCutAnalyzer,
) -> Result<AutoCutPlan> {
    let auto_cuts = project
        .auto_cuts
        .iter()
        .map(|auto_cut| plan_autocut(project, auto_cut, analyzer))
        .collect::<Result<Vec<_>>>()?;

    Ok(AutoCutPlan { auto_cuts })
}

fn plan_autocut(
    project: &NormalizedProject,
    auto_cut: &AutoCut,
    analyzer: &dyn AutoCutAnalyzer,
) -> Result<PlannedAutoCut> {
    let input = project.input(&auto_cut.input).ok_or_else(|| {
        CutlineError::InvalidProject(format!(
            "unknown input {:?} at auto_cut {:?}",
            auto_cut.input, auto_cut.name
        ))
    })?;
    let input_duration = input
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.duration)
        .ok_or_else(|| {
            CutlineError::InvalidProject(format!(
                "auto_cut {:?} requires probed duration for input {:?}",
                auto_cut.name, auto_cut.input
            ))
        })?;
    let selection_duration = input_duration
        .millis()
        .min(auto_cut.target_duration.millis()) as i64;
    let collected_analysis =
        collect_analysis(input.path.as_path(), input_duration, auto_cut, analyzer);
    let generated = generate_clips_from_analysis_with_status(
        selection_duration,
        auto_cut.clip_duration.millis() as i64,
        auto_cut.min_clip_duration.millis() as i64,
        &collected_analysis.scene_changes,
        &collected_analysis.audio_regions,
    );
    let clips = generated
        .segments
        .into_iter()
        .enumerate()
        .scan(0_u64, |output_cursor, (index, (start, end))| {
            let start = start.max(0) as u64;
            let end = end.max(0) as u64;
            let duration = end.saturating_sub(start);
            let output_start = *output_cursor;
            let output_end = output_start + duration;
            *output_cursor = output_end;

            Some(AutoCutClip {
                index,
                input: auto_cut.input.clone(),
                start: TimeValue::from_millis(start),
                end: TimeValue::from_millis(end),
                duration: TimeValue::from_millis(duration),
                output_start: TimeValue::from_millis(output_start),
                output_end: TimeValue::from_millis(output_end),
            })
        })
        .collect();

    Ok(PlannedAutoCut {
        name: auto_cut.name.clone(),
        input: auto_cut.input.clone(),
        input_path: input.path.clone(),
        input_duration,
        target_duration: auto_cut.target_duration,
        clip_duration: auto_cut.clip_duration,
        min_clip_duration: auto_cut.min_clip_duration,
        output_mode: auto_cut.output_mode.clone(),
        rules: auto_cut.rules.clone(),
        analysis: AutoCutAnalysis {
            scene_changes: collected_analysis
                .scene_changes
                .iter()
                .map(|timestamp| TimeValue::from_millis((*timestamp).max(0) as u64))
                .collect(),
            audio_regions: collected_analysis
                .audio_regions
                .iter()
                .map(|(start, end)| AutoCutAudioRegion {
                    start: TimeValue::from_millis((*start).max(0) as u64),
                    end: TimeValue::from_millis((*end).max(0) as u64),
                })
                .collect(),
            fallback_used: generated.fallback_used,
        },
        clips,
    })
}

#[derive(Debug, Clone)]
struct CollectedAnalysis {
    scene_changes: Vec<i64>,
    audio_regions: Vec<(i64, i64)>,
}

fn collect_analysis(
    video_path: &Utf8Path,
    input_duration: TimeValue,
    auto_cut: &AutoCut,
    analyzer: &dyn AutoCutAnalyzer,
) -> CollectedAnalysis {
    let mut scene_changes = Vec::new();
    let mut audio_regions = Vec::new();

    for rule in &auto_cut.rules {
        match rule {
            AutoCutRule::SceneChange { threshold } => {
                scene_changes.extend(analyzer.scene_changes(video_path, *threshold));
            }
            AutoCutRule::AudioActivity { threshold } => {
                audio_regions.extend(analyzer.audio_activity(
                    video_path,
                    *threshold,
                    input_duration,
                ));
            }
            AutoCutRule::Motion { threshold } => {
                scene_changes.extend(analyzer.motion_anchors(video_path, *threshold));
            }
        }
    }

    scene_changes.sort_unstable();
    scene_changes.dedup();
    audio_regions.sort_unstable();
    audio_regions.dedup();

    CollectedAnalysis {
        scene_changes,
        audio_regions,
    }
}

fn auto_cut_rule(rule: &AutoCutRuleConfig) -> AutoCutRule {
    match rule {
        AutoCutRuleConfig::SceneChange { threshold } => AutoCutRule::SceneChange {
            threshold: *threshold,
        },
        AutoCutRuleConfig::AudioActivity { threshold } => AutoCutRule::AudioActivity {
            threshold: *threshold,
        },
        AutoCutRuleConfig::Motion { threshold } => AutoCutRule::Motion {
            threshold: *threshold,
        },
    }
}

/// Analyze video using ffmpeg to detect scene changes.
/// Returns a list of timestamps (in milliseconds) where scene changes occur.
pub async fn analyze_scene_changes(video_path: &str) -> Vec<i64> {
    analyze_scene_changes_with_threshold(Utf8Path::new(video_path), 0.3)
}

fn analyze_scene_changes_with_threshold(video_path: &Utf8Path, threshold: f64) -> Vec<i64> {
    let filter = format!("select=gt(scene\\,{threshold}),showinfo");
    let output = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-i",
            video_path.as_str(),
            "-filter:v",
            &filter,
            "-f",
            "null",
            "-",
        ])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            parse_scene_showinfo(&stderr)
        }
        _ => Vec::new(),
    }
}

/// Analyze video audio to detect activity regions.
/// Returns a list of (start, end) timestamps where audio activity is detected.
pub async fn analyze_audio_activity(video_path: &str) -> Vec<(i64, i64)> {
    let duration = probe_duration(Utf8Path::new(video_path)).ok();
    analyze_audio_activity_with_threshold(Utf8Path::new(video_path), 0.01, duration)
}

fn analyze_audio_activity_with_threshold(
    video_path: &Utf8Path,
    threshold: f64,
    duration: Option<TimeValue>,
) -> Vec<(i64, i64)> {
    let noise_db = audio_threshold_to_noise_db(threshold);
    let filter = format!("silencedetect=noise={noise_db:.1}dB:d=0.2");
    let output = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-i",
            video_path.as_str(),
            "-af",
            &filter,
            "-f",
            "null",
            "-",
        ])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            parse_audio_activity_from_silencedetect(
                &stderr,
                duration.map(|duration| duration.millis() as i64),
            )
        }
        _ => Vec::new(),
    }
}

fn audio_threshold_to_noise_db(threshold: f64) -> f64 {
    if !threshold.is_finite() || threshold <= 0.0 {
        return -30.0;
    }

    let db = 20.0 * threshold.log10();
    db.clamp(-80.0, 0.0)
}

/// Generate clip segments based on analysis results and target duration.
pub fn generate_clips_from_analysis(
    total_duration: i64,
    clip_duration: i64,
    min_clip_duration: i64,
    scene_changes: &[i64],
    audio_regions: &[(i64, i64)],
) -> Vec<(i64, i64)> {
    generate_clips_from_analysis_with_status(
        total_duration,
        clip_duration,
        min_clip_duration,
        scene_changes,
        audio_regions,
    )
    .segments
}

#[derive(Debug, Clone)]
struct GeneratedClips {
    segments: Vec<(i64, i64)>,
    fallback_used: bool,
}

fn generate_clips_from_analysis_with_status(
    total_duration: i64,
    clip_duration: i64,
    min_clip_duration: i64,
    scene_changes: &[i64],
    audio_regions: &[(i64, i64)],
) -> GeneratedClips {
    if total_duration <= 0 || clip_duration <= 0 || min_clip_duration <= 0 {
        return GeneratedClips {
            segments: Vec::new(),
            fallback_used: true,
        };
    }

    let clip_duration = clip_duration.min(total_duration);
    let min_clip_duration = min_clip_duration.min(clip_duration);
    if scene_changes.is_empty() && audio_regions.is_empty() {
        return GeneratedClips {
            segments: fallback_clips(total_duration, clip_duration, min_clip_duration),
            fallback_used: true,
        };
    }

    let mut anchors = analysis_anchors(total_duration, scene_changes, audio_regions);
    anchors.sort_unstable();
    anchors.dedup();

    let mut clips = Vec::new();
    let mut covered_until = 0;
    for anchor in anchors {
        let start = align_clip_start(anchor, clip_duration, total_duration);
        let end = (start + clip_duration).min(total_duration);
        if end - start < min_clip_duration {
            continue;
        }
        if start < covered_until {
            continue;
        }
        clips.push((start, end));
        covered_until = end;
    }

    if clips.is_empty() {
        GeneratedClips {
            segments: fallback_clips(total_duration, clip_duration, min_clip_duration),
            fallback_used: true,
        }
    } else {
        GeneratedClips {
            segments: clips,
            fallback_used: false,
        }
    }
}

fn fallback_clips(
    total_duration: i64,
    clip_duration: i64,
    min_clip_duration: i64,
) -> Vec<(i64, i64)> {
    let mut clips = Vec::new();
    let mut cursor = 0;
    while cursor < total_duration {
        let end = (cursor + clip_duration).min(total_duration);
        if end - cursor >= min_clip_duration {
            clips.push((cursor, end));
        }
        cursor += clip_duration;
    }
    clips
}

fn analysis_anchors(
    total_duration: i64,
    scene_changes: &[i64],
    audio_regions: &[(i64, i64)],
) -> Vec<i64> {
    let mut anchors = Vec::new();
    anchors.extend(
        scene_changes
            .iter()
            .copied()
            .filter(|timestamp| (0..total_duration).contains(timestamp)),
    );
    anchors.extend(audio_regions.iter().filter_map(|(start, end)| {
        let start = (*start).clamp(0, total_duration);
        let end = (*end).clamp(0, total_duration);
        (end > start).then_some(start)
    }));
    anchors
}

fn align_clip_start(anchor: i64, clip_duration: i64, total_duration: i64) -> i64 {
    let latest_start = total_duration.saturating_sub(clip_duration);
    anchor.clamp(0, latest_start)
}

fn parse_scene_showinfo(stderr: &str) -> Vec<i64> {
    let mut timestamps = stderr
        .lines()
        .filter_map(|line| parse_key_seconds_millis(line, "pts_time:"))
        .collect::<Vec<_>>();
    timestamps.sort_unstable();
    timestamps.dedup();
    timestamps
}

fn parse_audio_activity_from_silencedetect(
    stderr: &str,
    total_duration: Option<i64>,
) -> Vec<(i64, i64)> {
    let mut regions = Vec::new();
    let mut active_start = Some(0_i64);

    for line in stderr.lines() {
        if let Some(silence_start) = parse_key_seconds_millis(line, "silence_start:") {
            if let Some(start) = active_start.take() {
                if silence_start > start {
                    regions.push((start, silence_start));
                }
            }
        }

        if let Some(silence_end) = parse_key_seconds_millis(line, "silence_end:") {
            active_start = Some(silence_end);
        }
    }

    if let (Some(start), Some(total_duration)) = (active_start, total_duration) {
        if total_duration > start {
            regions.push((start, total_duration));
        }
    }

    regions
}

fn parse_key_seconds_millis(line: &str, key: &str) -> Option<i64> {
    let rest = line.get(line.find(key)? + key.len()..)?.trim_start();
    let token = rest.split_whitespace().next()?.trim_end_matches('|');
    let seconds = token.parse::<f64>().ok()?;
    if seconds.is_finite() && seconds >= 0.0 {
        Some((seconds * 1000.0).round() as i64)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use camino::Utf8PathBuf;

    use crate::config::{InputConfig, OutputConfig, ProjectConfig, RenderConfig};
    use crate::model::{Input, InputMetadata};
    use crate::time::TimeValue;

    #[derive(Debug, Default)]
    struct FakeAnalyzer {
        scene_changes: Vec<i64>,
        audio_regions: Vec<(i64, i64)>,
        motion_anchors: Vec<i64>,
    }

    impl AutoCutAnalyzer for FakeAnalyzer {
        fn scene_changes(&self, _video_path: &Utf8Path, _threshold: f64) -> Vec<i64> {
            self.scene_changes.clone()
        }

        fn audio_activity(
            &self,
            _video_path: &Utf8Path,
            _threshold: f64,
            _duration: TimeValue,
        ) -> Vec<(i64, i64)> {
            self.audio_regions.clone()
        }

        fn motion_anchors(&self, _video_path: &Utf8Path, _threshold: f64) -> Vec<i64> {
            self.motion_anchors.clone()
        }
    }

    fn project_with_autocut(auto_cut: AutoCut, input_duration: u64) -> NormalizedProject {
        NormalizedProject {
            project_path: Utf8PathBuf::from("project.toml"),
            project_dir: Utf8PathBuf::from("."),
            output_path: Utf8PathBuf::from("dist/out.mp4"),
            render: RenderConfig::default(),
            inputs: vec![Input {
                name: "main".to_owned(),
                path: Utf8PathBuf::from("raw/vod.mp4"),
                chat: None,
                metadata: Some(InputMetadata {
                    size_bytes: 100,
                    modified_unix_millis: None,
                    duration: Some(TimeValue::from_millis(input_duration)),
                }),
            }],
            clips: vec![],
            auto_cuts: vec![auto_cut],
            story_videos: vec![],
        }
    }

    fn basic_autocut(rules: Vec<AutoCutRule>) -> AutoCut {
        AutoCut {
            name: "main_autocut".to_owned(),
            input: "main".to_owned(),
            target_duration: TimeValue::from_millis(60_000),
            clip_duration: TimeValue::from_millis(20_000),
            min_clip_duration: TimeValue::from_millis(10_000),
            rules,
            output_mode: AutoCutOutputMode::Single,
        }
    }

    #[test]
    fn generates_autocut_from_config() {
        let config = AutoCutConfig {
            name: "test".to_string(),
            input: "main".to_string(),
            target_duration: TimeValue::from_millis(300000), // 5 min
            clip_duration: TimeValue::from_millis(30000),    // 30 sec
            min_clip_duration: TimeValue::from_millis(15000), // 15 sec
            rules: vec![],
            output_mode: "single".to_string(),
        };

        let autocut = generate_autocut(&config);

        assert_eq!(autocut.name, "test");
        assert_eq!(autocut.input, "main");
        assert_eq!(autocut.target_duration.millis(), 300000);
        assert_eq!(autocut.clip_duration.millis(), 30000);
    }

    #[test]
    fn generates_fallback_clips_when_analysis_is_empty() {
        let clips = generate_clips_from_analysis(95_000, 30_000, 15_000, &[], &[]);

        assert_eq!(clips, vec![(0, 30_000), (30_000, 60_000), (60_000, 90_000)]);
    }

    #[test]
    fn keeps_short_final_fallback_clip_when_it_meets_minimum_duration() {
        let clips = generate_clips_from_analysis(75_000, 30_000, 15_000, &[], &[]);

        assert_eq!(clips, vec![(0, 30_000), (30_000, 60_000), (60_000, 75_000)]);
    }

    #[test]
    fn uses_scene_and_audio_anchors_without_overlapping_clips() {
        let clips = generate_clips_from_analysis(
            120_000,
            20_000,
            10_000,
            &[5_000, 12_000, 50_000],
            &[(80_000, 95_000)],
        );

        assert_eq!(
            clips,
            vec![(5_000, 25_000), (50_000, 70_000), (80_000, 100_000)]
        );
    }

    #[test]
    fn parses_scene_showinfo_timestamps() {
        let timestamps = parse_scene_showinfo(
            "[Parsed_showinfo_1 @ 0x] n: 0 pts: 12000 pts_time:12 pos:0\n\
             [Parsed_showinfo_1 @ 0x] n: 1 pts: 34567 pts_time:34.567 pos:0\n",
        );

        assert_eq!(timestamps, vec![12_000, 34_567]);
    }

    #[test]
    fn parses_audio_activity_from_silencedetect_output() {
        let regions = parse_audio_activity_from_silencedetect(
            "[silencedetect @ 0x] silence_start: 3\n\
             [silencedetect @ 0x] silence_end: 5.5 | silence_duration: 2.5\n\
             [silencedetect @ 0x] silence_start: 12.25\n",
            Some(20_000),
        );

        assert_eq!(regions, vec![(0, 3_000), (5_500, 12_250)]);
    }

    #[test]
    fn builds_autocut_plan_from_project_duration() {
        let mut auto_cut = basic_autocut(vec![]);
        auto_cut.clip_duration = TimeValue::from_millis(30_000);
        let project = project_with_autocut(auto_cut, 65_000);

        let plan = build_autocut_plan_with_analyzer(&project, &FakeAnalyzer::default()).unwrap();

        assert_eq!(plan.auto_cuts.len(), 1);
        assert_eq!(plan.auto_cuts[0].clips.len(), 2);
        assert_eq!(plan.auto_cuts[0].clips[1].start.millis(), 30_000);
        assert!(plan.auto_cuts[0].analysis.fallback_used);
    }

    #[test]
    fn autocut_plan_respects_target_duration() {
        let project = project_with_autocut(basic_autocut(vec![]), 180_000);

        let plan = build_autocut_plan_with_analyzer(&project, &FakeAnalyzer::default()).unwrap();

        assert_eq!(plan.auto_cuts[0].clips.len(), 3);
        assert_eq!(plan.auto_cuts[0].clips[2].end.millis(), 60_000);
    }

    #[test]
    fn autocut_plan_uses_scene_and_audio_rule_analysis() {
        let project = project_with_autocut(
            basic_autocut(vec![
                AutoCutRule::SceneChange { threshold: 0.3 },
                AutoCutRule::AudioActivity { threshold: 0.01 },
            ]),
            120_000,
        );
        let analyzer = FakeAnalyzer {
            scene_changes: vec![8_000, 50_000],
            audio_regions: vec![(82_000, 96_000)],
            motion_anchors: vec![],
        };

        let plan = build_autocut_plan_with_analyzer(&project, &analyzer).unwrap();

        assert!(!plan.auto_cuts[0].analysis.fallback_used);
        assert_eq!(
            plan.auto_cuts[0]
                .analysis
                .scene_changes
                .iter()
                .map(|timestamp| timestamp.millis())
                .collect::<Vec<_>>(),
            vec![8_000, 50_000]
        );
        assert_eq!(
            plan.auto_cuts[0].analysis.audio_regions[0].start.millis(),
            82_000
        );
        assert_eq!(plan.auto_cuts[0].clips[0].start.millis(), 8_000);
        assert_eq!(plan.auto_cuts[0].clips[1].start.millis(), 40_000);
    }

    #[test]
    fn autocut_plan_uses_motion_rule_as_visual_anchor() {
        let project = project_with_autocut(
            basic_autocut(vec![AutoCutRule::Motion { threshold: 0.1 }]),
            120_000,
        );
        let analyzer = FakeAnalyzer {
            motion_anchors: vec![35_000],
            ..Default::default()
        };

        let plan = build_autocut_plan_with_analyzer(&project, &analyzer).unwrap();

        assert!(!plan.auto_cuts[0].analysis.fallback_used);
        assert_eq!(plan.auto_cuts[0].analysis.scene_changes[0].millis(), 35_000);
        assert_eq!(plan.auto_cuts[0].clips[0].start.millis(), 35_000);
    }

    #[test]
    fn converts_audio_threshold_to_silencedetect_noise_floor() {
        assert_eq!(audio_threshold_to_noise_db(0.01), -40.0);
        assert_eq!(audio_threshold_to_noise_db(0.0), -30.0);
        assert_eq!(audio_threshold_to_noise_db(10.0), 0.0);
    }

    #[test]
    fn normalizes_autocut_config_from_project_config() {
        let mut input = BTreeMap::new();
        input.insert(
            "main".to_owned(),
            InputConfig {
                path: Utf8PathBuf::from("raw/vod.mp4"),
                chat: None,
            },
        );

        let config = ProjectConfig {
            output: OutputConfig {
                path: Utf8PathBuf::from("dist/out.mp4"),
            },
            input,
            render: RenderConfig::default(),
            clips: vec![],
            auto_cuts: vec![AutoCutConfig {
                name: "main_autocut".to_owned(),
                input: "main".to_owned(),
                target_duration: TimeValue::from_millis(60_000),
                clip_duration: TimeValue::from_millis(30_000),
                min_clip_duration: TimeValue::from_millis(10_000),
                rules: vec![AutoCutRuleConfig::SceneChange { threshold: 0.3 }],
                output_mode: "multiple".to_owned(),
            }],
            story_videos: vec![],
        };

        let auto_cut = generate_autocut(&config.auto_cuts[0]);

        assert_eq!(auto_cut.output_mode, AutoCutOutputMode::Multiple);
        assert!(matches!(
            auto_cut.rules[0],
            AutoCutRule::SceneChange { threshold } if threshold == 0.3
        ));
    }
}
