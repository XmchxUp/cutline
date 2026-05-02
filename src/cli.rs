use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "cutline")]
#[command(about = "Declarative video cutting with cached ffmpeg clips")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Parse and validate a project file.
    Check {
        project: Utf8PathBuf,

        /// Skip ffprobe media duration checks.
        #[arg(long)]
        no_probe: bool,
    },

    /// Print the derived timeline and render plan.
    Plan {
        project: Utf8PathBuf,

        /// Emit machine-readable JSON.
        #[arg(long)]
        json: bool,

        /// Skip ffprobe media duration checks.
        #[arg(long)]
        no_probe: bool,
    },

    /// Render clip cache files and concatenate the final output.
    Render {
        project: Utf8PathBuf,

        /// Overwrite an existing final output file.
        #[arg(long)]
        force: bool,
    },

    /// Remove this project's cache directory.
    Clean { project: Utf8PathBuf },
}
