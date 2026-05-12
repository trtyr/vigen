use thiserror::Error;

#[derive(Error, Debug)]
pub enum VigenError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("config error: {0}")]
    Config(String),

    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("api error (status {status}): {message}")]
    ApiError { status: u16, message: String },

    #[error("provider '{0}' not configured")]
    ProviderNotConfigured(String),

    #[error("oauth error: {0}")]
    OAuth(String),
}

impl From<serde_json::Error> for VigenError {
    fn from(e: serde_json::Error) -> Self {
        VigenError::Config(e.to_string())
    }
}

impl From<toml::de::Error> for VigenError {
    fn from(e: toml::de::Error) -> Self {
        VigenError::Config(e.to_string())
    }
}

impl From<toml::ser::Error> for VigenError {
    fn from(e: toml::ser::Error) -> Self {
        VigenError::Config(e.to_string())
    }
}
