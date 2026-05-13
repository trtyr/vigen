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

impl VigenError {
    pub fn is_fatal(&self) -> bool {
        match self {
            VigenError::Config(_) => true,
            VigenError::ProviderNotConfigured(_) => true,
            VigenError::OAuth(_) => true,
            VigenError::ApiError { status, .. } => {
                matches!(status, 401 | 403)
            }
            VigenError::Io(_) => false,
            VigenError::Http(_) => false,
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_io_error_display() {
        let e = VigenError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "not found"));
        assert!(e.to_string().contains("not found"));
    }

    #[test]
    fn test_config_error_display() {
        let e = VigenError::Config("bad format".into());
        assert!(e.to_string().contains("bad format"));
    }

    #[test]
    fn test_api_error_display() {
        let e = VigenError::ApiError {
            status: 429,
            message: "rate limited".into(),
        };
        let s = e.to_string();
        assert!(s.contains("429"));
        assert!(s.contains("rate limited"));
    }

    #[test]
    fn test_provider_not_configured_display() {
        let e = VigenError::ProviderNotConfigured("openai".into());
        assert!(e.to_string().contains("openai"));
    }

    #[test]
    fn test_oauth_error_display() {
        let e = VigenError::OAuth("token expired".into());
        assert!(e.to_string().contains("token expired"));
    }

    #[test]
    fn test_from_io_error() {
        let io = std::io::Error::new(std::io::ErrorKind::Other, "boom");
        let ve: VigenError = io.into();
        matches!(ve, VigenError::Io(_));
    }

    #[tokio::test]
    async fn test_from_reqwest_error() {
        let client = reqwest::Client::new();
        let err = client.get("not-a-url://").send().await.unwrap_err();
        let ve: VigenError = err.into();
        matches!(ve, VigenError::Http(_));
    }

    #[test]
    fn test_is_fatal_config() {
        assert!(VigenError::Config("bad".into()).is_fatal());
    }

    #[test]
    fn test_is_fatal_provider_not_configured() {
        assert!(VigenError::ProviderNotConfigured("x".into()).is_fatal());
    }

    #[test]
    fn test_is_fatal_oauth() {
        assert!(VigenError::OAuth("bad".into()).is_fatal());
    }

    #[test]
    fn test_is_fatal_auth_401() {
        assert!(VigenError::ApiError {
            status: 401,
            message: "unauthorized".into()
        }
        .is_fatal());
    }

    #[test]
    fn test_is_fatal_auth_403() {
        assert!(VigenError::ApiError {
            status: 403,
            message: "forbidden".into()
        }
        .is_fatal());
    }

    #[test]
    fn test_is_not_fatal_429() {
        assert!(!VigenError::ApiError {
            status: 429,
            message: "rate limited".into()
        }
        .is_fatal());
    }

    #[test]
    fn test_is_not_fatal_500() {
        assert!(!VigenError::ApiError {
            status: 500,
            message: "server error".into()
        }
        .is_fatal());
    }

    #[tokio::test]
    async fn test_is_not_fatal_http() {
        let e = VigenError::Http(
            reqwest::Client::new()
                .get("not-a-url://")
                .send()
                .await
                .unwrap_err(),
        );
        assert!(!e.is_fatal());
    }
}
