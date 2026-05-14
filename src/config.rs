use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::VigenError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderType {
    Google,
    Gpt,
}

impl ProviderType {
    pub fn parse(s: &str) -> Result<Self, VigenError> {
        match s.to_lowercase().as_str() {
            "google" | "gemini" => Ok(Self::Google),
            "gpt" | "openai" => Ok(Self::Gpt),
            other => Err(VigenError::Config(format!("unknown provider: {other}"))),
        }
    }
}

impl Serialize for ProviderType {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(match self {
            ProviderType::Google => "google",
            ProviderType::Gpt => "gpt",
        })
    }
}

impl<'de> Deserialize<'de> for ProviderType {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        ProviderType::parse(&s).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
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

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpt: Option<GptConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AuthConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub google: Option<GoogleAuth>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpt: Option<GptAuth>,
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
    pub fallback_model: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

fn default_google_model() -> String {
    "gemini-2.0-flash".to_string()
}

fn default_gpt_model() -> String {
    "gpt-image-2".to_string()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GptAuth {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GptConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    #[serde(default = "default_gpt_model")]
    pub model: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_endpoint: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_model: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy: Option<String>,
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

pub fn resolve_proxy(
    provider_proxy: Option<&str>,
    global_proxy: Option<&ProxyConfig>,
) -> Option<String> {
    provider_proxy
        .map(String::from)
        .or_else(|| global_proxy.map(|p| p.url.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_google_config() -> GoogleConfig {
        GoogleConfig {
            api_key: Some("g-key-123".into()),
            model: "gemini-2.0-flash".into(),
            fallback_model: Some("gemini-1.5-flash".into()),
            proxy: None,
            project: None,
        }
    }

    fn sample_gpt_config() -> GptConfig {
        GptConfig {
            api_key: Some("c-key-456".into()),
            model: "gpt-image-2".into(),
            base_url: Some("https://api.openai.com".into()),
            image_endpoint: Some("/v1/images/generations".into()),
            fallback_model: None,
            proxy: None,
        }
    }

    #[test]
    fn test_default_config_empty_providers() {
        let cfg = VigenConfig::default();
        assert!(cfg.providers.google.is_none());
        assert!(cfg.providers.gpt.is_none());
        assert!(cfg.auth.is_none());
    }

    #[test]
    fn test_roundtrip_google_only() {
        let mut cfg = VigenConfig::default();
        cfg.providers.google = Some(sample_google_config());
        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        let round: VigenConfig = toml::from_str(&toml_str).unwrap();
        let g = round.providers.google.unwrap();
        assert_eq!(g.api_key.unwrap(), "g-key-123");
        assert_eq!(g.model, "gemini-2.0-flash");
        assert_eq!(g.fallback_model.unwrap(), "gemini-1.5-flash");
    }

    #[test]
    fn test_roundtrip_gpt_only() {
        let mut cfg = VigenConfig::default();
        cfg.providers.gpt = Some(sample_gpt_config());
        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        let round: VigenConfig = toml::from_str(&toml_str).unwrap();
        let c = round.providers.gpt.unwrap();
        assert_eq!(c.api_key.unwrap(), "c-key-456");
        assert_eq!(c.model, "gpt-image-2");
        assert_eq!(c.base_url.unwrap(), "https://api.openai.com");
        assert_eq!(c.image_endpoint.unwrap(), "/v1/images/generations");
        assert!(c.fallback_model.is_none());
    }

    #[test]
    fn test_roundtrip_both_providers() {
        let mut cfg = VigenConfig::default();
        cfg.providers.google = Some(sample_google_config());
        cfg.providers.gpt = Some(sample_gpt_config());
        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        let round: VigenConfig = toml::from_str(&toml_str).unwrap();
        assert!(round.providers.google.is_some());
        assert!(round.providers.gpt.is_some());
    }

    #[test]
    fn test_roundtrip_with_proxy() {
        let mut cfg = VigenConfig::default();
        cfg.proxy = Some(ProxyConfig {
            url: "http://127.0.0.1:7890".into(),
        });
        cfg.providers.google = Some(sample_google_config());
        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        let round: VigenConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(round.proxy.unwrap().url, "http://127.0.0.1:7890");
    }

    #[test]
    fn test_roundtrip_with_auth() {
        let mut cfg = VigenConfig::default();
        cfg.auth = Some(AuthConfig {
            google: Some(GoogleAuth {
                client_id: "cid".into(),
                client_secret: "csec".into(),
                refresh_token: "rtok".into(),
            }),
            gpt: Some(GptAuth {
                access_token: "atok".into(),
                refresh_token: "ctok".into(),
                expires_at: 9999999999,
            }),
        });
        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        let round: VigenConfig = toml::from_str(&toml_str).unwrap();
        let auth = round.auth.unwrap();
        assert_eq!(auth.google.unwrap().refresh_token, "rtok");
        assert_eq!(auth.gpt.unwrap().refresh_token, "ctok");
    }

    #[test]
    fn test_provider_type_deserialize_unknown() {
        let result: Result<ProviderType, _> = toml::from_str("\"anthropic\"");
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_proxy_provider_wins() {
        let result = resolve_proxy(
            Some("http://p.proxy:8080"),
            Some(&ProxyConfig {
                url: "http://global.proxy:8080".into(),
            }),
        );
        assert_eq!(result.unwrap(), "http://p.proxy:8080");
    }

    #[test]
    fn test_resolve_proxy_falls_back_to_global() {
        let result = resolve_proxy(
            None,
            Some(&ProxyConfig {
                url: "http://global.proxy:8080".into(),
            }),
        );
        assert_eq!(result.unwrap(), "http://global.proxy:8080");
    }

    #[test]
    fn test_resolve_proxy_none() {
        let result = resolve_proxy(None, None);
        assert!(result.is_none());
    }

    #[test]
    fn test_config_path_ends_with_vigen_config_toml() {
        let path = VigenConfig::config_path();
        let s = path.to_string_lossy();
        assert!(s.contains("vigen"), "path must contain vigen: {s}");
        assert!(s.ends_with("config.toml"), "path must end with config.toml: {s}");
    }
}
