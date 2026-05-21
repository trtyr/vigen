pub mod gpt;
pub mod google;
mod http;

use async_trait::async_trait;

use crate::config::{resolve_proxy, ProviderType, VigenConfig};
use crate::error::VigenError;

macro_rules! try_endpoint {
    ($last_err:expr, $label:expr, $result:expr) => {
        match $result {
            Ok(r) => return Ok(r),
            Err(e) => {
                eprintln!("[vigen] {} failed: {}", $label, e);
                if e.is_fatal() {
                    return Err(e);
                }
                $last_err = Some(e);
            }
        }
    };
}

#[async_trait]
pub trait VisionProvider: Send + Sync {
    async fn analyze_image(
        &self,
        image_data: &[u8],
        mime_type: &str,
        prompt: &str,
    ) -> Result<String, VigenError>;
}

#[async_trait]
pub trait ImageGenProvider: Send + Sync {
    async fn generate_image(
        &mut self,
        prompt: &str,
        size: &str,
        n: u8,
    ) -> Result<Vec<String>, VigenError>;
}

pub async fn analyze_image(
    config: &VigenConfig,
    image_data: &[u8],
    mime: &str,
    prompt: &str,
) -> Result<String, VigenError> {
    let c = config
        .providers
        .google
        .as_ref()
        .ok_or_else(|| VigenError::ProviderNotConfigured("google".into()))?;

    let mut models = vec![c.model.clone()];
    if let Some(ref fb) = c.fallback_model {
        models.push(fb.clone());
    }

    let mut last_err = None;
    for model in &models {
        let result = {
            let p = google::GoogleProvider::from_config_with_model(config, model)?;
            p.analyze_image(image_data, mime, prompt).await
        };
        match result {
            Ok(r) => return Ok(r),
            Err(e) => {
                if e.is_fatal() {
                    return Err(e);
                }
                last_err = Some(e);
            }
        }
    }

    Err(last_err.unwrap_or_else(|| VigenError::ApiError {
        status: 0,
        message: "no models configured".into(),
    }))
}

pub async fn generate_image(
    config: &mut VigenConfig,
    prompt: &str,
    size: &str,
    n: u8,
) -> Result<Vec<String>, VigenError> {
    let c = config
        .providers
        .gpt
        .as_ref()
        .ok_or_else(|| VigenError::ProviderNotConfigured("gpt".into()))?;
    let model = c.model.clone();
    let fallback_model = c.fallback_model.clone();
    let base_url = c.base_url.clone();
    let image_endpoint = c.image_endpoint.clone();
    let proxy = c.proxy.clone();
    let fallbacks = c.fallbacks.clone();

    let mut models = vec![model.clone()];
    if let Some(fb) = fallback_model {
        models.push(fb);
    }

    let mut last_err = None;
    for model in &models {
        let result = {
            let mut p = gpt::GptProvider::from_config_with_model(config, model)?;
            let result = p.generate_image(prompt, size, n).await;
            p.write_auth_if_dirty(config)?;
            result
        };
        try_endpoint!(last_err, format!("primary ({model})"), result);
    }

    for (i, fallback) in fallbacks.iter().enumerate() {
        let fallback_model = fallback.model.as_deref().unwrap_or(&model).to_string();
        let fallback_base_url = fallback.base_url.as_deref().or(base_url.as_deref());
        let fallback_image_endpoint = fallback
            .image_endpoint
            .as_deref()
            .or(image_endpoint.as_deref());
        let proxy_url = resolve_proxy(proxy.as_deref(), config.proxy.as_ref());
        let result = {
            let mut p = gpt::GptProvider::from_parts(
                fallback.api_key.clone(),
                fallback_model,
                fallback_base_url,
                fallback_image_endpoint,
                proxy_url.as_deref(),
            )?;
            p.generate_image(prompt, size, n).await
        };
        try_endpoint!(last_err, format!("fallback #{}", i + 1), result);
    }

    Err(last_err.unwrap_or_else(|| VigenError::ApiError {
        status: 0,
        message: "no models configured".into(),
    }))
}

pub async fn login(
    provider: ProviderType,
    config: &mut VigenConfig,
    proxy: Option<&str>,
) -> Result<(), VigenError> {
    match provider {
        ProviderType::Google => google::login(config, proxy).await,
        ProviderType::Gpt => gpt::login_oauth(config, proxy).await,
    }
}

pub async fn list_models(
    provider: ProviderType,
    config: &VigenConfig,
) -> Result<Vec<(String, Option<String>)>, VigenError> {
    match provider {
        ProviderType::Google => {
            let p = google::GoogleProvider::from_config(config)?;
            let models = p.list_models().await?;
            Ok(models
                .into_iter()
                .map(|m| (m.name, m.display_name))
                .collect())
        }
        ProviderType::Gpt => {
            Ok(vec![
                ("gpt-image-2".into(), Some("DALL·E 3".into())),
                ("dall-e-2".into(), Some("DALL·E 2".into())),
            ])
        }
    }
}
