use std::fs;

use anyhow::Context;
use clap::Parser;
use cutline::cli::{Cli, Command};
use cutline::config::ProjectConfig;
use cutline::planner::build_plan;
use cutline::validate::normalize_project;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Check { project, no_probe } => {
            let _project = load_project(&project)?;
            if no_probe {
                println!("ok: project is valid (media probing skipped)");
            } else {
                println!("ok: project is valid (media probing not implemented yet)");
            }
        }
        Command::Plan { project, json } => {
            let project = load_project(&project)?;
            let plan = build_plan(&project);
            if json {
                println!("{}", serde_json::to_string_pretty(&plan)?);
            } else {
                print_plan(&plan);
            }
        }
        Command::Render { project, force } => {
            let project = load_project(&project)?;
            if project.output_path.exists() && !force {
                anyhow::bail!(
                    "output file already exists: {}\nhint: pass --force to overwrite",
                    project.output_path
                );
            }
            println!("render is not implemented yet");
        }
        Command::Clean { project } => {
            let project = load_project(&project)?;
            let cache_dir = cutline::cache::cache_dir(&project);
            if cache_dir.exists() {
                fs::remove_dir_all(&cache_dir)
                    .with_context(|| format!("failed to remove cache directory {cache_dir}"))?;
                println!("removed {cache_dir}");
            } else {
                println!("cache directory does not exist: {cache_dir}");
            }
        }
    }

    Ok(())
}

fn load_project(path: &camino::Utf8Path) -> anyhow::Result<cutline::model::NormalizedProject> {
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read project file {path}"))?;
    let config: ProjectConfig =
        toml::from_str(&content).with_context(|| format!("failed to parse project file {path}"))?;
    normalize_project(path, config).map_err(anyhow::Error::from)
}

fn print_plan(plan: &cutline::planner::Plan) {
    println!("Output: {}", plan.output_path);
    println!();
    println!("Clips:");
    for planned in &plan.clips {
        let clip = &planned.clip;
        println!(
            "  {}  {}  {} -> {}  out {} -> {}  cache {}",
            clip.index + 1,
            clip.input,
            clip.start.display(),
            clip.end.display(),
            clip.output_start.display(),
            clip.output_end.display(),
            planned.cache_key,
        );
    }
}
