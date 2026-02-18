use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum PageError {
    #[error("Config file not found: {path}")]
    ConfigNotFound { path: PathBuf },

    #[error("Invalid config: {message}")]
    ConfigInvalid { message: String },

    #[error("Content error in {path}: {message}")]
    Content { path: PathBuf, message: String },

    #[error("Frontmatter parse error in {path}: {source}")]
    Frontmatter {
        path: PathBuf,
        source: serde_yaml_ng::Error,
    },

    #[error("Template error: {0}")]
    Template(#[from] tera::Error),

    #[error("Shortcode error in {path} at line {line}: {message}")]
    Shortcode {
        path: PathBuf,
        line: usize,
        message: String,
    },

    #[error("Build error: {0}")]
    Build(String),

    #[error("Server error: {0}")]
    Server(String),

    #[error("Deploy error: {0}")]
    Deploy(String),

    #[error("Agent error: {0}")]
    Agent(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Data file error in {path}: {message}")]
    Data { path: PathBuf, message: String },

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, PageError>;
