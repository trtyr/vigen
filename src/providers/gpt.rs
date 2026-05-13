use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use reqwest::Client;

use crate::config::{resolve_proxy, GptConfig, VigenConfig};
use crate::error::VigenError;

use super::VisionProvider;

pub struct GptProvider {
    api_key: String,
    model: String,
    client: Client,
}

impl GptProvider {
    pub fn from_config(full: &VigenConfig) -> Result<Self, VigenError> {
        let config = full
            .providers
            .gpt
            .clone()
            .ok_or_else(|| VigenError::ProviderNotConfigured("gpt".into()))?;
        let api_key = config
            .api_key
            .filter(|k| !k.is_empty())
            .ok_or_else(|| VigenError::ProviderNotConfigured("gpt api_key".into()))?;
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

    pub fn from_config_with_model(full: &VigenConfig, model: &str) -> Result<Self, VigenError> {
        let mut p = Self::from_config(full)?;
        p.model = model.to_string();
        Ok(p)
    }

    pub fn from_config_with_gen_model(full: &VigenConfig, model: &str) -> Result<Self, VigenError> {
        let mut p = Self::from_config(full)?;
        p.model = model.to_string();
        Ok(p)
    }
}

#[async_trait]
impl VisionProvider for GptProvider {
    async fn analyze_image(
        &self,
        image_data: &[u8],
        mime_type: &str,
        prompt: &str,
    ) -> Result<String, VigenError> {
        let b64 = BASE64.encode(image_data);
        let data_url = format!("data:{mime_type};base64,{b64}");
        let body = serde_json::json!({
            "model": self.model,
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "text", "text": prompt},
                    {"type": "image_url", "image_url": {"url": data_url, "detail": "auto"}}
                ]
            }]
        });
        let response = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
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
        let resp: serde_json::Value = response.json().await?;
        let text = resp["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| VigenError::ApiError {
                status: 200,
                message: "empty response".into(),
            })?;
        Ok(text.to_string())
    }
}

#[async_trait]
impl super::ImageGenProvider for GptProvider {
    async fn generate_image(
        &self,
        prompt: &str,
        size: &str,
        n: u8,
    ) -> Result<Vec<String>, VigenError> {
        let body = serde_json::json!({
            "model": self.model,
            "prompt": prompt,
            "n": n,
            "size": size,
            "response_format": "b64_json"
        });
        let response = self
            .client
            .post("https://api.openai.com/v1/images/generations")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
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
        let resp: serde_json::Value = response.json().await?;
        let images: Vec<String> = resp["data"]
            .as_array()
            .ok_or_else(|| VigenError::ApiError {
                status: 200,
                message: "empty response".into(),
            })?
            .iter()
            .filter_map(|v| v["b64_json"].as_str().map(String::from))
            .collect();
        if images.is_empty() {
            return Err(VigenError::ApiError {
                status: 200,
                message: "no images in response".into(),
            });
        }
        Ok(images)
    }
}

pub fn login_with_api_key(config: &mut VigenConfig) -> Result<(), VigenError> {
    let mut input = String::new();
    eprint!("Enter OpenAI API key: ");
    std::io::stdin()
        .read_line(&mut input)
        .map_err(VigenError::Io)?;
    let api_key = input.trim().to_string();
    if api_key.is_empty() {
        return Err(VigenError::OAuth("no api key provided".into()));
    }
    let gpt_cfg = config.providers.gpt.get_or_insert_with(|| GptConfig {
        api_key: None,
        model: "gpt-4o".into(),
        fallback_model: None,
        gen_model: "gpt-image-2".into(),
        gen_fallback_model: None,
        proxy: None,
    });
    gpt_cfg.api_key = Some(api_key);
    config.save()?;
    eprintln!("API key saved.");
    Ok(())
}
