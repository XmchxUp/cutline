pub type Result<T> = std::result::Result<T, CutlineError>;

#[derive(Debug, thiserror::Error)]
pub enum CutlineError {
    #[error("invalid time value: {0}")]
    InvalidTime(String),

    #[error("invalid project: {0}")]
    InvalidProject(String),

    #[error("media probe failed for {path}: {message}")]
    MediaProbe {
        path: camino::Utf8PathBuf,
        message: String,
    },

    #[error("command failed: {program} {args}")]
    CommandFailed { program: String, args: String },

    #[error("path is not valid UTF-8: {0}")]
    NonUtf8Path(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Toml(#[from] toml::de::Error),
}
