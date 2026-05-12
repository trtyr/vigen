use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::VigenError;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VigenConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy: Option<ProxyConfig>,

    #[serde(default)]
    pub providers: ProviderConfigs,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProxyConfig {
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ProviderConfigs {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub google: Option<GoogleConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AuthConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub google: Option<GoogleAuth>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GoogleAuth {
    pub client_id: String,
    pub client_secret: String,
    pub refresh_token: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GoogleConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    #[serde(default = "default_google_model")]
    pub model: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

fn default_google_model() -> String {
    "gemini-2.0-flash".to_string()
}

impl VigenConfig {
    pub fn config_path() -> PathBuf {
        let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        base.join("vigen").join("config.toml")
    }

    pub fn load() -> Result<Self, VigenError> {
        let path = Self::config_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)
            .map_err(|e| VigenError::Config(format!("cannot read config: {e}")))?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self) -> Result<(), VigenError> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                VigenError::Config(format!("cannot create config dir: {e}"))
            })?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)
            .map_err(|e| VigenError::Config(format!("cannot write config: {e}")))?;
        Ok(())
    }
}

impl Default for VigenConfig {
    fn default() -> Self {
        Self {
            proxy: None,
            providers: ProviderConfigs::default(),
            auth: None,
        }
    }
}

pub fn resolve_proxy(
    provider_proxy: Option<&str>,
    global_proxy: Option<&ProxyConfig>,
) -> Option<String> {
    provider_proxy
        .map(String::from)
        .or_else(|| global_proxy.map(|p| p.url.clone()))
}
