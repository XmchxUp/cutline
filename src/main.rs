use std::fs;

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use cutline::cli::{Cli, Command};
use cutline::config::ProjectConfig;
use cutline::ffmpeg::{clip_render_command, temp_clip_path};
use cutline::model::{AutoCutOutputMode, Clip, NormalizedProject};
use cutline::planner::{Plan, build_plan, final_concat_command};
use cutline::validate::{ValidationOptions, normalize_project_with_options};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Check { project, no_probe } => {
            let _project = load_project(
                &project,
                ValidationOptions {
                    require_inputs: true,
                    probe_media: !no_probe,
                },
            )?;
            if no_probe {
                println!("ok: project is valid (media probing skipped)");
            } else {
                println!("ok: project is valid");
            }
        }
        Command::Plan {
            project,
            json,
            no_probe,
        } => {
            let project = load_project(
                &project,
                ValidationOptions {
                    require_inputs: true,
                    probe_media: !no_probe,
                },
            )?;
            let plan = build_plan(&project)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&plan)?);
            } else {
                print_plan(&plan);
            }
        }
        Command::Render {
            project,
            force,
            autocut,
            story,
        } => {
            if autocut {
                let project = load_project(
                    &project,
                    ValidationOptions {
                        require_inputs: true,
                        probe_media: true,
                    },
                )?;
                render_autocut_project(&project, force)?;
            } else if story {
                let project = load_project(
                    &project,
                    ValidationOptions {
                        require_inputs: true,
                        probe_media: false,
                    },
                )?;
                render_story_project(&project, force)?;
            } else {
                let project = load_project(
                    &project,
                    ValidationOptions {
                        require_inputs: true,
                        probe_media: true,
                    },
                )?;
                if project.output_path.exists() && !force {
                    anyhow::bail!(
                        "output file already exists: {}\nhint: pass --force to overwrite",
                        project.output_path
                    );
                }
                let plan = build_plan(&project)?;
                render_project(&project, &plan)?;
            }
        }
        Command::Clean { project } => {
            let project = load_project(
                &project,
                ValidationOptions {
                    require_inputs: false,
                    probe_media: false,
                },
            )?;
            let cache_dir = cutline::cache::cache_dir(&project);
            if cache_dir.exists() {
                fs::remove_dir_all(&cache_dir)
                    .with_context(|| format!("failed to remove cache directory {cache_dir}"))?;
                println!("removed {cache_dir}");
            } else {
                println!("cache directory does not exist: {cache_dir}");
            }
        }
        Command::AutoCut { project, json } => {
            let output = run_autocut_command(&project, json)?;
            print!("{output}");
        }
        Command::Story {
            project,
            json,
            voice_list,
            render_preview,
        } => {
            let output = run_story_command(&project, json, voice_list, render_preview)?;
            print!("{output}");
        }
    }

    Ok(())
}

fn run_autocut_command(project: &Utf8Path, json: bool) -> anyhow::Result<String> {
    let project = load_project(
        project,
        ValidationOptions {
            require_inputs: true,
            probe_media: true,
        },
    )?;
    let plan = cutline::autocut::build_autocut_plan(&project)?;

    if json {
        Ok(format!("{}\n", serde_json::to_string_pretty(&plan)?))
    } else {
        Ok(format_autocut_plan(&plan))
    }
}

fn format_autocut_plan(plan: &cutline::autocut::AutoCutPlan) -> String {
    let mut output = String::new();
    for auto_cut in &plan.auto_cuts {
        output.push_str(&format!("AutoCut: {}\n", auto_cut.name));
        output.push_str(&format!(
            "  input: {} ({})\n",
            auto_cut.input, auto_cut.input_path
        ));
        output.push_str(&format!(
            "  target: {}  clip: {}  min: {}\n",
            auto_cut.target_duration.display(),
            auto_cut.clip_duration.display(),
            auto_cut.min_clip_duration.display()
        ));
        output.push_str(&format!("  output_mode: {:?}\n", auto_cut.output_mode));
        output.push_str(&format!(
            "  analysis: {} scene changes, {} audio regions, fallback {}\n",
            auto_cut.analysis.scene_changes.len(),
            auto_cut.analysis.audio_regions.len(),
            auto_cut.analysis.fallback_used
        ));
        output.push_str("  clips:\n");
        for clip in &auto_cut.clips {
            output.push_str(&format!(
                "    {}  {} -> {}  duration {}  out {} -> {}\n",
                clip.index + 1,
                clip.start.display(),
                clip.end.display(),
                clip.duration.display(),
                clip.output_start.display(),
                clip.output_end.display()
            ));
        }
    }
    output
}

fn render_autocut_project(project: &NormalizedProject, force: bool) -> anyhow::Result<()> {
    let plan = cutline::autocut::build_autocut_plan(project)?;
    for auto_cut in &plan.auto_cuts {
        match auto_cut.output_mode {
            AutoCutOutputMode::Single => {
                let render_project_model =
                    project_for_autocut_clips(project, auto_cut.name.as_str(), &auto_cut.clips);
                if render_project_model.output_path.exists() && !force {
                    anyhow::bail!(
                        "output file already exists: {}\nhint: pass --force to overwrite",
                        render_project_model.output_path
                    );
                }
                let render_plan = build_plan(&render_project_model)?;
                render_project(&render_project_model, &render_plan)?;
            }
            AutoCutOutputMode::Multiple => {
                for clip in &auto_cut.clips {
                    let output_path = multiple_autocut_output_path(
                        &project.output_path,
                        auto_cut.name.as_str(),
                        clip.index,
                    );
                    if output_path.exists() && !force {
                        anyhow::bail!(
                            "output file already exists: {}\nhint: pass --force to overwrite",
                            output_path
                        );
                    }
                    let render_project_model = project_for_autocut_output(
                        project,
                        auto_cut.name.as_str(),
                        output_path,
                        &[clip.clone()],
                    );
                    let render_plan = build_plan(&render_project_model)?;
                    render_project(&render_project_model, &render_plan)?;
                }
            }
        }
    }

    Ok(())
}

fn render_story_project(project: &NormalizedProject, force: bool) -> anyhow::Result<()> {
    if project.output_path.exists() && !force {
        anyhow::bail!(
            "output file already exists: {}\nhint: pass --force to overwrite",
            project.output_path
        );
    }

    let summary = cutline::story::generate_reviewable_draft_package_with_options(
        project,
        cutline::story::DraftPackageOptions {
            render_preview: true,
            ffmpeg_program: "ffmpeg".to_owned(),
            voice_provider: cutline::story::VoiceProviderConfig::None,
        },
    )?;
    let preview_path = summary.package_path.join("preview.mp4");
    if !preview_path.is_file() {
        anyhow::bail!("story render did not create expected preview: {preview_path}");
    }
    if let Some(parent) = project.output_path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("failed to create {parent}"))?;
    }
    fs::copy(&preview_path, &project.output_path).with_context(|| {
        format!(
            "failed to copy story preview {} to {}",
            preview_path, project.output_path
        )
    })?;
    println!("rendered story output: {}", project.output_path);
    println!("created draft package: {}", summary.package_path);
    Ok(())
}

fn project_for_autocut_clips(
    project: &NormalizedProject,
    auto_cut_name: &str,
    clips: &[cutline::autocut::AutoCutClip],
) -> NormalizedProject {
    project_for_autocut_output(project, auto_cut_name, project.output_path.clone(), clips)
}

fn project_for_autocut_output(
    project: &NormalizedProject,
    auto_cut_name: &str,
    output_path: Utf8PathBuf,
    clips: &[cutline::autocut::AutoCutClip],
) -> NormalizedProject {
    let clips = clips
        .iter()
        .enumerate()
        .scan(0_u64, |output_cursor, (index, clip)| {
            let output_start = *output_cursor;
            let output_end = output_start + clip.duration.millis();
            *output_cursor = output_end;

            Some((index, clip, output_start, output_end))
        })
        .map(|(index, clip, output_start, output_end)| Clip {
            index,
            input: clip.input.clone(),
            start: clip.start,
            end: clip.end,
            output_start: cutline::time::TimeValue::from_millis(output_start),
            output_end: cutline::time::TimeValue::from_millis(output_end),
            chapter: Some(format!("{} {}", auto_cut_name, clip.index + 1)),
            blur: false,
            mute: false,
        })
        .collect();

    NormalizedProject {
        project_path: project.project_path.clone(),
        project_dir: project.project_dir.clone(),
        output_path,
        render: project.render.clone(),
        inputs: project.inputs.clone(),
        clips,
        auto_cuts: Vec::new(),
        story_videos: Vec::new(),
    }
}

fn multiple_autocut_output_path(
    base_output_path: &Utf8Path,
    auto_cut_name: &str,
    index: usize,
) -> Utf8PathBuf {
    let parent = base_output_path
        .parent()
        .unwrap_or_else(|| Utf8Path::new(""));
    let stem = base_output_path.file_stem().unwrap_or("autocut");
    let extension = base_output_path.extension().unwrap_or("mp4");
    parent.join(format!(
        "{stem}-{auto_cut_name}-{index:03}.{extension}",
        index = index + 1
    ))
}

fn run_story_command(
    project: &Utf8Path,
    json: bool,
    voice_list: bool,
    render_preview: bool,
) -> anyhow::Result<String> {
    if voice_list {
        return Ok("Voice list not yet implemented\n".to_owned());
    }

    let project = load_project(
        project,
        ValidationOptions {
            require_inputs: true,
            probe_media: false,
        },
    )?;
    let summary = cutline::story::generate_reviewable_draft_package_with_options(
        &project,
        cutline::story::DraftPackageOptions {
            render_preview,
            ffmpeg_program: "ffmpeg".to_owned(),
            voice_provider: cutline::story::VoiceProviderConfig::None,
        },
    )?;

    if json {
        Ok(format!(
            "{}\n",
            serde_json::json!({
                "draft_id": summary.draft_id,
                "package_path": summary.package_path,
            })
        ))
    } else {
        Ok(format!("created draft package: {}\n", summary.package_path))
    }
}

fn load_project(
    path: &Utf8Path,
    options: ValidationOptions,
) -> anyhow::Result<cutline::model::NormalizedProject> {
    let canonical_path = canonicalize_utf8(path)?;
    let content = fs::read_to_string(&canonical_path)
        .with_context(|| format!("failed to read project file {canonical_path}"))?;
    let config: ProjectConfig = toml::from_str(&content)
        .with_context(|| format!("failed to parse project file {canonical_path}"))?;
    normalize_project_with_options(&canonical_path, config, options).map_err(anyhow::Error::from)
}

fn print_plan(plan: &cutline::planner::Plan) {
    println!("Output: {}", plan.output_path);
    println!();
    println!("Inputs:");
    for input in &plan.inputs {
        let duration = input.duration.as_deref().unwrap_or("unknown");
        println!("  {}  {}  duration {}", input.name, input.path, duration);
    }
    println!();
    println!("Clips:");
    for planned in &plan.clips {
        let clip = &planned.clip;
        let cache_status = if planned.cache_exists { "hit" } else { "miss" };
        println!(
            "  {}  {}  {} -> {}  out {} -> {}  cache {}  {}",
            clip.index + 1,
            clip.input,
            clip.start.display(),
            clip.end.display(),
            clip.output_start.display(),
            clip.output_end.display(),
            cache_status,
            planned.cache_key,
        );
        println!("      {}", planned.ffmpeg);
    }

    if !plan.chapters.is_empty() {
        println!();
        println!("Chapters:");
        for chapter in &plan.chapters {
            println!("  {}  {}", chapter.time, chapter.title);
        }
    }
}

fn render_project(project: &cutline::model::NormalizedProject, plan: &Plan) -> anyhow::Result<()> {
    let work_dir = cutline::cache::work_dir(project);
    let cache_dir = cutline::cache::cache_dir(project);
    fs::create_dir_all(&cache_dir).with_context(|| format!("failed to create {cache_dir}"))?;

    if let Some(parent) = project.output_path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("failed to create {parent}"))?;
    }

    for planned in &plan.clips {
        if planned.cache_path.exists() {
            println!("cache hit: {}", planned.cache_path);
            continue;
        }

        let temp_path = temp_clip_path(&planned.cache_path);
        if temp_path.exists() {
            fs::remove_file(&temp_path)
                .with_context(|| format!("failed to remove stale temp file {temp_path}"))?;
        }

        let command = clip_render_command(project, planned, &temp_path)?;
        println!("{}", command.display());
        command.run()?;
        fs::rename(&temp_path, &planned.cache_path).with_context(|| {
            format!(
                "failed to move rendered clip {} to {}",
                temp_path, planned.cache_path
            )
        })?;
    }

    fs::create_dir_all(&work_dir).with_context(|| format!("failed to create {work_dir}"))?;
    write_concat_list(&plan.concat_list_path, plan)?;
    let command = final_concat_command(plan);
    println!("{}", command.display());
    command.run()?;
    write_plan_json(project)?;
    Ok(())
}

fn write_concat_list(path: &Utf8Path, plan: &Plan) -> anyhow::Result<()> {
    let mut content = String::new();
    for planned in &plan.clips {
        content.push_str("file '");
        content.push_str(&planned.cache_path.as_str().replace('\'', "'\\''"));
        content.push_str("'\n");
    }

    fs::write(path, content).with_context(|| format!("failed to write concat list {path}"))
}

fn write_plan_json(project: &cutline::model::NormalizedProject) -> anyhow::Result<()> {
    let plan = build_plan(project)?;
    let path = cutline::cache::plan_json_path(project);
    let content = serde_json::to_string_pretty(&plan)?;
    fs::write(&path, content).with_context(|| format!("failed to write plan json {path}"))
}

fn canonicalize_utf8(path: &Utf8Path) -> anyhow::Result<Utf8PathBuf> {
    let canonical = fs::canonicalize(path).with_context(|| format!("failed to access {path}"))?;
    Utf8PathBuf::from_path_buf(canonical)
        .map_err(|path| anyhow::anyhow!("path is not valid UTF-8: {}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use camino::Utf8PathBuf;

    use super::run_story_command;

    #[test]
    fn story_command_json_outputs_draft_summary() {
        let root =
            std::env::temp_dir().join(format!("cutline-story-command-json-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("stories")).unwrap();
        fs::create_dir_all(root.join("assets")).unwrap();
        fs::write(root.join("stories/demo.txt"), "第一行\n第二行\n").unwrap();
        fs::write(root.join("assets/bg.mp4"), "not real media").unwrap();
        fs::write(
            root.join("project.toml"),
            r#"
            [output]
            path = "dist/story.mp4"

            [[story]]
            name = "demo"
            source = "stories/demo.txt"
            start_line = 1
            end_line = 2
            engagement_angle = "reversal"
            background = "assets/bg.mp4"
            platform = "douyin"
            "#,
        )
        .unwrap();

        let project_path = Utf8PathBuf::from_path_buf(root.join("project.toml")).unwrap();
        let output = run_story_command(&project_path, true, false, false).unwrap();
        let json: serde_json::Value = serde_json::from_str(&output).unwrap();

        assert_eq!(json["draft_id"], "demo");
        assert!(
            json["package_path"]
                .as_str()
                .unwrap()
                .ends_with(".cutline/drafts/demo")
        );
        assert!(root.join(".cutline/drafts/demo/draft.json").is_file());

        let _ = fs::remove_dir_all(&root);
    }
}
