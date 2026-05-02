use std::process::Command;

use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserialize;

use crate::config::RenderConfig;
use crate::error::{CutlineError, Result};
use crate::model::{Clip, Input, NormalizedProject};
use crate::planner::{Plan, PlannedClip};
use crate::time::TimeValue;

#[derive(Debug, Clone)]
pub struct FfmpegCommand {
    pub program: String,
    pub args: Vec<String>,
}

impl FfmpegCommand {
    pub fn run(&self) -> Result<()> {
        let status = Command::new(&self.program).args(&self.args).status()?;
        if status.success() {
            Ok(())
        } else {
            Err(CutlineError::CommandFailed {
                program: self.program.clone(),
                args: self.args.join(" "),
            })
        }
    }

    pub fn display(&self) -> String {
        let mut parts = vec![self.program.clone()];
        parts.extend(self.args.iter().map(|arg| shell_quote(arg)));
        parts.join(" ")
    }
}

pub fn probe_duration(path: &Utf8Path) -> Result<TimeValue> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "json",
            path.as_str(),
        ])
        .output()
        .map_err(|err| CutlineError::MediaProbe {
            path: path.to_path_buf(),
            message: format!("failed to run ffprobe: {err}"),
        })?;

    if !output.status.success() {
        return Err(CutlineError::MediaProbe {
            path: path.to_path_buf(),
            message: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }

    #[derive(Debug, Deserialize)]
    struct ProbeFormat {
        duration: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    struct ProbeOutput {
        format: Option<ProbeFormat>,
    }

    let probe: ProbeOutput =
        serde_json::from_slice(&output.stdout).map_err(|err| CutlineError::MediaProbe {
            path: path.to_path_buf(),
            message: err.to_string(),
        })?;

    let duration = probe
        .format
        .and_then(|format| format.duration)
        .ok_or_else(|| CutlineError::MediaProbe {
            path: path.to_path_buf(),
            message: "ffprobe did not report format.duration".to_owned(),
        })?;

    let seconds = duration
        .parse::<f64>()
        .map_err(|err| CutlineError::MediaProbe {
            path: path.to_path_buf(),
            message: format!("invalid ffprobe duration {duration:?}: {err}"),
        })?;

    if !seconds.is_finite() || seconds < 0.0 {
        return Err(CutlineError::MediaProbe {
            path: path.to_path_buf(),
            message: format!("invalid ffprobe duration {duration:?}"),
        });
    }

    Ok(TimeValue::from_millis((seconds * 1000.0).round() as u64))
}

pub fn clip_render_command(
    project: &NormalizedProject,
    planned: &PlannedClip,
    temp_path: &Utf8Path,
) -> Result<FfmpegCommand> {
    let input = project.input(&planned.clip.input).ok_or_else(|| {
        CutlineError::InvalidProject(format!("unknown input {:?}", planned.clip.input))
    })?;

    Ok(FfmpegCommand {
        program: "ffmpeg".to_owned(),
        args: clip_render_args(input, &planned.clip, &project.render, temp_path),
    })
}

pub fn concat_command(plan: &Plan, concat_list_path: &Utf8Path) -> FfmpegCommand {
    FfmpegCommand {
        program: "ffmpeg".to_owned(),
        args: vec![
            "-y".to_owned(),
            "-f".to_owned(),
            "concat".to_owned(),
            "-safe".to_owned(),
            "0".to_owned(),
            "-i".to_owned(),
            concat_list_path.to_string(),
            "-c".to_owned(),
            "copy".to_owned(),
            plan.output_path.to_string(),
        ],
    }
}

pub fn clip_render_args(
    input: &Input,
    clip: &Clip,
    render: &RenderConfig,
    output_path: &Utf8Path,
) -> Vec<String> {
    let mut args = vec![
        "-y".to_owned(),
        "-ss".to_owned(),
        clip.start.as_ffmpeg_seconds(),
    ];

    args.extend(render.extra.input_args.iter().cloned());
    args.extend([
        "-i".to_owned(),
        input.path.to_string(),
        "-t".to_owned(),
        clip.duration().as_ffmpeg_seconds(),
        "-c:v".to_owned(),
        render.video_codec.clone(),
    ]);

    if let Some(preset) = &render.preset {
        args.extend(["-preset".to_owned(), preset.clone()]);
    }

    if let Some(crf) = render.crf {
        args.extend(["-crf".to_owned(), crf.to_string()]);
    } else {
        args.extend(["-b:v".to_owned(), render.video_bitrate.clone()]);
    }

    args.extend([
        "-pix_fmt".to_owned(),
        render.pixel_format.clone(),
        "-c:a".to_owned(),
        render.audio_codec.clone(),
        "-b:a".to_owned(),
        render.audio_bitrate.clone(),
    ]);

    if clip.blur {
        args.extend(["-vf".to_owned(), "boxblur=50:5".to_owned()]);
    }

    if clip.mute {
        args.extend(["-af".to_owned(), "volume=0".to_owned()]);
    }

    args.extend(render.extra.output_args.iter().cloned());
    args.push(output_path.to_string());
    args
}

pub fn shell_quote(arg: &str) -> String {
    if arg.is_empty() {
        "''".to_owned()
    } else if arg
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "-_./:=+,@%".contains(ch))
    {
        arg.to_owned()
    } else {
        format!("'{}'", arg.replace('\'', "'\\''"))
    }
}

pub fn temp_clip_path(cache_path: &Utf8Path) -> Utf8PathBuf {
    let file_name = cache_path.file_name().unwrap_or("clip.mp4");
    cache_path.with_file_name(format!("{file_name}.tmp"))
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;

    use crate::config::RenderConfig;
    use crate::model::{Clip, Input};
    use crate::time::TimeValue;

    use super::{clip_render_args, shell_quote};

    #[test]
    fn builds_clip_render_args_with_effects() {
        let input = Input {
            name: "main".to_owned(),
            path: Utf8PathBuf::from("raw/vod.mp4"),
            chat: None,
            metadata: None,
        };
        let clip = Clip {
            index: 0,
            input: "main".to_owned(),
            start: TimeValue::from_millis(10_000),
            end: TimeValue::from_millis(20_500),
            output_start: TimeValue::from_millis(0),
            output_end: TimeValue::from_millis(10_500),
            chapter: None,
            blur: true,
            mute: true,
        };
        let mut render = RenderConfig::default();
        render.preset = Some("veryfast".to_owned());
        render.crf = Some(23);

        let args = clip_render_args(&input, &clip, &render, "cache/out.mp4".into());

        assert!(args.windows(2).any(|pair| pair == ["-ss", "10.000"]));
        assert!(args.windows(2).any(|pair| pair == ["-t", "10.500"]));
        assert!(args.windows(2).any(|pair| pair == ["-vf", "boxblur=50:5"]));
        assert!(args.windows(2).any(|pair| pair == ["-af", "volume=0"]));
    }

    #[test]
    fn quotes_shell_args() {
        assert_eq!(shell_quote("simple"), "simple");
        assert_eq!(shell_quote("has space"), "'has space'");
        assert_eq!(shell_quote("has'quote"), "'has'\\''quote'");
    }
}
