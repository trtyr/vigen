pub mod gpt;
pub mod google;

use std::fmt::Write;
use std::time::Duration;

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

struct FallbackStep {
    provider: ProviderType,
    model: String,
    error: VigenError,
}

fn format_fallback_error(steps: &[FallbackStep]) -> VigenError {
    let mut msg = format!(
        "all fallback steps exhausted ({} attempt{}):\n",
        steps.len(),
        if steps.len() == 1 { "" } else { "s" }
    );
    for (i, step) in steps.iter().enumerate() {
        let _ = writeln!(
            &mut msg,
            "  {}. {:?}/{}: {}",
            i + 1,
            step.provider,
            step.model,
            step.error
        );
    }
    VigenError::ApiError {
        status: 0,
        message: msg.trim_end().to_string(),
    }
}

fn is_rate_limit(e: &VigenError) -> bool {
    matches!(e, VigenError::ApiError { status: 429, .. })
}

fn get_model_info(config: &VigenConfig, pt: ProviderType) -> (String, Option<String>) {
    match pt {
        ProviderType::Google => {
            let c = &config.providers.google;
            (
                c.as_ref()
                    .map(|c| c.model.clone())
                    .unwrap_or_else(|| "gemini-2.0-flash".into()),
                c.as_ref().and_then(|c| c.fallback_model.clone()),
            )
        }
        ProviderType::Gpt => {
            let c = &config.providers.gpt;
            (
                c.as_ref()
                    .map(|c| c.model.clone())
                    .unwrap_or_else(|| "gpt-4o".into()),
                c.as_ref().and_then(|c| c.fallback_model.clone()),
            )
        }
    }
}

fn get_gen_model_info(config: &VigenConfig, pt: ProviderType) -> (String, Option<String>) {
    match pt {
        ProviderType::Gpt => {
            let c = &config.providers.gpt;
            (
                c.as_ref()
                    .map(|c| c.gen_model.clone())
                    .unwrap_or_else(|| "gpt-image-2".into()),
                c.as_ref().and_then(|c| c.gen_fallback_model.clone()),
            )
        }
        ProviderType::Google => ("gpt-image-2".into(), None),
    }
}

async fn try_with_model(
    config: &VigenConfig,
    pt: ProviderType,
    model: &str,
    image_data: &[u8],
    mime: &str,
    prompt: &str,
) -> Result<String, VigenError> {
    match pt {
        ProviderType::Google => {
            let p = google::GoogleProvider::from_config_with_model(config, model)?;
            p.analyze_image(image_data, mime, prompt).await
        }
        ProviderType::Gpt => {
            let p = gpt::GptProvider::from_config_with_model(config, model)?;
            p.analyze_image(image_data, mime, prompt).await
        }
    }
}

async fn try_gen_with_model(
    config: &VigenConfig,
    pt: ProviderType,
    model: &str,
    prompt: &str,
    size: &str,
    n: u8,
) -> Result<Vec<String>, VigenError> {
    match pt {
        ProviderType::Gpt => {
            let p = gpt::GptProvider::from_config_with_gen_model(config, model)?;
            p.generate_image(prompt, size, n).await
        }
        ProviderType::Google => Err(VigenError::ProviderNotConfigured(
            "google does not support image generation".into(),
        )),
    }
}

pub async fn analyze_image(
    provider: Option<ProviderType>,
    config: &VigenConfig,
    image_data: &[u8],
    mime: &str,
    prompt: &str,
) -> Result<String, VigenError> {
    let primary = provider.unwrap_or_else(|| {
        config.defaults.vision.unwrap_or(ProviderType::Google)
    });
    let allow_cross = provider.is_none();

    let mut chain: Vec<(ProviderType, String)> = Vec::new();
    let (p_model, p_fallback) = get_model_info(config, primary);
    chain.push((primary, p_model));
    if let Some(fb) = p_fallback {
        chain.push((primary, fb));
    }

    if allow_cross {
        if let Some(fb_pt) = config.defaults.vision_fallback {
            let (fb_model, fb_fallback_model) = get_model_info(config, fb_pt);
            chain.push((fb_pt, fb_model));
            if let Some(fb_fb) = fb_fallback_model {
                chain.push((fb_pt, fb_fb));
            }
        }
    }

    let mut steps: Vec<FallbackStep> = Vec::new();
    for (pt, model) in &chain {
        match try_with_model(config, *pt, model, image_data, mime, prompt).await {
            Ok(r) => return Ok(r),
            Err(e) => {
                let fatal = e.is_fatal();
                steps.push(FallbackStep {
                    provider: *pt,
                    model: model.clone(),
                    error: e,
                });
                if fatal {
                    break;
                }
                if is_rate_limit(&steps.last().unwrap().error) {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }

    Err(format_fallback_error(&steps))
}

pub async fn generate_image(
    provider: Option<ProviderType>,
    config: &VigenConfig,
    prompt: &str,
    size: &str,
    n: u8,
) -> Result<Vec<String>, VigenError> {
    let primary = provider.unwrap_or_else(|| {
        config.defaults.image_gen.unwrap_or(ProviderType::Gpt)
    });
    let allow_cross = provider.is_none();

    let mut chain: Vec<(ProviderType, String)> = Vec::new();
    let (p_model, p_fallback) = get_gen_model_info(config, primary);
    chain.push((primary, p_model));
    if let Some(fb) = p_fallback {
        chain.push((primary, fb));
    }

    if allow_cross {
        if let Some(fb_pt) = config.defaults.image_gen_fallback {
            let (fb_model, fb_fallback_model) = get_gen_model_info(config, fb_pt);
            chain.push((fb_pt, fb_model));
            if let Some(fb_fb) = fb_fallback_model {
                chain.push((fb_pt, fb_fb));
            }
        }
    }

    let mut steps: Vec<FallbackStep> = Vec::new();
    for (pt, model) in &chain {
        match try_gen_with_model(config, *pt, model, prompt, size, n).await {
            Ok(r) => return Ok(r),
            Err(e) => {
                let fatal = e.is_fatal();
                steps.push(FallbackStep {
                    provider: *pt,
                    model: model.clone(),
                    error: e,
                });
                if fatal {
                    break;
                }
                if is_rate_limit(&steps.last().unwrap().error) {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }

    Err(format_fallback_error(&steps))
}

pub async fn login(
    provider: ProviderType,
    config: &mut VigenConfig,
    proxy: Option<String>,
) -> Result<(), VigenError> {
    match provider {
        ProviderType::Google => google::login(config, proxy.as_deref()).await,
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
                ("gpt-4o".into(), Some("GPT-4o".into())),
                ("gpt-4o-mini".into(), Some("GPT-4o mini".into())),
                ("gpt-4-turbo".into(), Some("GPT-4 Turbo".into())),
            ])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_google_lowercase() {
        assert_eq!(ProviderType::parse("google").unwrap(), ProviderType::Google);
    }

    #[test]
    fn test_parse_google_mixed_case() {
        assert_eq!(ProviderType::parse("GoOgLe").unwrap(), ProviderType::Google);
    }

    #[test]
    fn test_parse_gemini_alias() {
        assert_eq!(ProviderType::parse("gemini").unwrap(), ProviderType::Google);
    }

    #[test]
    fn test_parse_gpt_lowercase() {
        assert_eq!(ProviderType::parse("gpt").unwrap(), ProviderType::Gpt);
    }

    #[test]
    fn test_parse_gpt_uppercase() {
        assert_eq!(ProviderType::parse("GPT").unwrap(), ProviderType::Gpt);
    }

    #[test]
    fn test_parse_openai_alias() {
        assert_eq!(ProviderType::parse("openai").unwrap(), ProviderType::Gpt);
    }

    #[test]
    fn test_parse_unknown_provider() {
        assert!(ProviderType::parse("anthropic").is_err());
        assert!(ProviderType::parse("").is_err());
    }

    #[test]
    fn test_provider_type_eq() {
        assert_eq!(ProviderType::Google, ProviderType::Google);
        assert_ne!(ProviderType::Google, ProviderType::Gpt);
    }
}
