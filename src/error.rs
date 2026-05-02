pub type Result<T> = std::result::Result<T, CutlineError>;

#[derive(Debug, thiserror::Error)]
pub enum CutlineError {
    #[error("invalid time value: {0}")]
    InvalidTime(String),

    #[error("invalid project: {0}")]
    InvalidProject(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Toml(#[from] toml::de::Error),
}
