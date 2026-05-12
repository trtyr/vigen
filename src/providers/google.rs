use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::config::{resolve_proxy, VigenConfig};
use crate::error::VigenError;

use super::VisionProvider;

const BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

#[derive(Debug, Serialize)]
struct GeminiRequest {
    contents: Vec<Content>,
}

#[derive(Debug, Serialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Debug, Serialize)]
struct Part {
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    inline_data: Option<InlineData>,
}

#[derive(Debug, Serialize)]
struct InlineData {
    mime_type: String,
    data: String,
}

#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Vec<Candidate>,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    content: ContentResp,
}

#[derive(Debug, Deserialize)]
struct ContentResp {
    parts: Vec<PartResp>,
}

#[derive(Debug, Deserialize)]
struct PartResp {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ModelsListResponse {
    pub models: Vec<ModelInfo>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub name: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub supported_generation_methods: Vec<String>,
    #[serde(default)]
    pub input_token_limit: Option<u64>,
    #[serde(default)]
    pub output_token_limit: Option<u64>,
}

pub struct GoogleProvider {
    api_key: String,
    model: String,
    client: Client,
}

impl GoogleProvider {
    pub fn from_config(full: &VigenConfig) -> Result<Self, VigenError> {
        let config = full
            .providers
            .google
            .clone()
            .ok_or_else(|| VigenError::ProviderNotConfigured("google".into()))?;

        let api_key = config
            .api_key
            .filter(|k| !k.is_empty())
            .ok_or_else(|| VigenError::ProviderNotConfigured("google api_key".into()))?;

        let proxy_url = resolve_proxy(config.proxy.as_deref(), full.proxy.as_ref());
        let mut builder = Client::builder();
        if let Some(ref url) = proxy_url {
            builder = builder.proxy(reqwest::Proxy::all(url)?);
        }
        let client = builder.build()?;

        Ok(Self {
            api_key,
            model: config.model,
            client,
        })
    }

    fn make_url(&self, path: &str) -> String {
        format!("{BASE_URL}{path}?key={}", self.api_key)
    }
}

#[async_trait]
impl VisionProvider for GoogleProvider {
    async fn analyze_image(
        &self,
        image_data: &[u8],
        mime_type: &str,
        prompt: &str,
    ) -> Result<String, VigenError> {
        let request = GeminiRequest {
            contents: vec![Content {
                parts: vec![
                    Part {
                        text: Some(prompt.to_string()),
                        inline_data: None,
                    },
                    Part {
                        text: None,
                        inline_data: Some(InlineData {
                            mime_type: mime_type.to_string(),
                            data: BASE64.encode(image_data),
                        }),
                    },
                ],
            }],
        };

        let url = self.make_url(&format!("/models/{}:generateContent", self.model));
        let response = self.client.post(&url).json(&request).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await?;
            return Err(VigenError::ApiError {
                status,
                message: body,
            });
        }

        let gemini_response: GeminiResponse = response.json().await?;
        let text = gemini_response
            .candidates
            .first()
            .and_then(|c| c.content.parts.first())
            .and_then(|p| p.text.as_deref())
            .ok_or_else(|| VigenError::ApiError {
                status: 200,
                message: "empty response from model".into(),
            })?;

        Ok(text.to_string())
    }
}

impl GoogleProvider {
    pub async fn list_models(&self) -> Result<Vec<ModelInfo>, VigenError> {
        let response = self
            .client
            .get(&self.make_url("/models"))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await?;
            return Err(VigenError::ApiError {
                status,
                message: body,
            });
        }

        let list: ModelsListResponse = response.json().await?;
        Ok(list.models)
    }
}
