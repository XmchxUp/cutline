use std::fs;

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use cutline::cli::{Cli, Command};
use cutline::config::ProjectConfig;
use cutline::ffmpeg::{clip_render_command, temp_clip_path};
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
                println!("AutoCut rendering not yet implemented");
            } else if story {
                println!("StoryToVideo rendering not yet implemented");
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
        Command::AutoCut {
            project: _,
            json: _,
        } => {
            println!("AutoCut command not yet implemented");
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
