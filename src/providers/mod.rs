pub mod gpt;
pub mod google;

use async_trait::async_trait;

use crate::config::{ProviderType, VigenConfig};
use crate::error::VigenError;

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
        &self,
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
    config: &VigenConfig,
    prompt: &str,
    size: &str,
    n: u8,
) -> Result<Vec<String>, VigenError> {
    let c = config
        .providers
        .gpt
        .as_ref()
        .ok_or_else(|| VigenError::ProviderNotConfigured("gpt".into()))?;

    let mut models = vec![c.model.clone()];
    if let Some(ref fb) = c.fallback_model {
        models.push(fb.clone());
    }

    let mut last_err = None;
    for model in &models {
        let result = {
            let p = gpt::GptProvider::from_config_with_model(config, model)?;
            p.generate_image(prompt, size, n).await
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

pub async fn login(
    provider: ProviderType,
    config: &mut VigenConfig,
    proxy: Option<&str>,
) -> Result<(), VigenError> {
    match provider {
        ProviderType::Google => google::login(config, proxy).await,
        ProviderType::Gpt => gpt::login_with_api_key(config),
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
