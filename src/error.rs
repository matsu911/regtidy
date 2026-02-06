use thiserror::Error;

#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum AppError {
    #[error("Registry API error: {0}")]
    RegistryApi(String),

    #[error("No cleanup strategy specified. Use --keep, --older-than, or --pattern")]
    NoStrategy,

    #[error("Invalid regex pattern: {0}")]
    InvalidPattern(#[from] regex::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Date parse error: {0}")]
    DateParse(#[from] chrono::ParseError),
}
