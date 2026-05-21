use async_trait::async_trait;
use base64::Engine;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use url::Url;

use crate::config::{resolve_proxy, GptAuth, GptConfig, VigenConfig};
use crate::error::VigenError;
use crate::pkce::{
    compute_code_challenge, generate_code_verifier, generate_state, url_encode, SUCCESS_PAGE,
};

use super::http::send_with_retry;

const OAUTH_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const OAUTH_AUTH_URL: &str = "https://auth.openai.com/oauth/authorize";
const OAUTH_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const OAUTH_REDIRECT_URI: &str = "http://localhost:1455/auth/callback";
const OAUTH_SCOPE: &str = "openid profile email offline_access";

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
}

pub struct GptProvider {
    api_key: String,
    model: String,
    base_url: String,
    image_endpoint: String,
    client: Client,
    refresh_token: Option<String>,
    expires_at: i64,
    auth_dirty: bool,
}

impl GptProvider {
    pub fn from_config(full: &VigenConfig) -> Result<Self, VigenError> {
        let config = full
            .providers
            .gpt
            .clone()
            .ok_or_else(|| VigenError::ProviderNotConfigured("gpt".into()))?;
        let proxy_url = resolve_proxy(config.proxy.as_deref(), full.proxy.as_ref());
        let mut builder = Client::builder();
        if let Some(ref url) = proxy_url {
            builder = builder
                .proxy(reqwest::Proxy::all(url).map_err(|e| VigenError::http("GPT proxy", e))?);
        }
        let client = builder
            .build()
            .map_err(|e| VigenError::http("GPT HTTP client", e))?;

        if let Some(ref api_key) = config.api_key {
            if !api_key.is_empty() {
                return Ok(Self {
                    api_key: api_key.clone(),
                    model: config.model,
                    base_url: normalized_base_url(config.base_url.as_deref()),
                    image_endpoint: normalized_endpoint(config.image_endpoint.as_deref()),
                    client,
                    refresh_token: None,
                    expires_at: i64::MAX,
                    auth_dirty: false,
                });
            }
        }

        let auth = full
            .auth
            .as_ref()
            .and_then(|a| a.gpt.as_ref())
            .ok_or_else(|| VigenError::ProviderNotConfigured("gpt api_key".into()))?;
        Ok(Self {
            api_key: auth.access_token.clone(),
            model: config.model,
            base_url: normalized_base_url(config.base_url.as_deref()),
            image_endpoint: normalized_endpoint(config.image_endpoint.as_deref()),
            client,
            refresh_token: Some(auth.refresh_token.clone()),
            expires_at: auth.expires_at,
            auth_dirty: false,
        })
    }

    pub fn from_config_with_model(full: &VigenConfig, model: &str) -> Result<Self, VigenError> {
        let mut p = Self::from_config(full)?;
        p.model = model.to_string();
        Ok(p)
    }

    pub fn from_parts(
        api_key: String,
        model: String,
        base_url: Option<&str>,
        image_endpoint: Option<&str>,
        proxy_url: Option<&str>,
    ) -> Result<Self, VigenError> {
        let mut builder = Client::builder();
        if let Some(url) = proxy_url {
            builder = builder
                .proxy(reqwest::Proxy::all(url).map_err(|e| VigenError::http("GPT proxy", e))?);
        }
        let client = builder
            .build()
            .map_err(|e| VigenError::http("GPT HTTP client", e))?;
        Ok(Self {
            api_key,
            model,
            base_url: normalized_base_url(base_url),
            image_endpoint: normalized_endpoint(image_endpoint),
            client,
            refresh_token: None,
            expires_at: i64::MAX,
            auth_dirty: false,
        })
    }

    async fn refresh_access_token(&mut self) -> Result<(), VigenError> {
        let rt = self
            .refresh_token
            .as_ref()
            .ok_or_else(|| VigenError::OAuth("no refresh token".into()))?;
        let body = format!(
            "grant_type=refresh_token&client_id={}&refresh_token={}",
            url_encode(OAUTH_CLIENT_ID),
            url_encode(rt),
        );
        let resp = self
            .client
            .post(OAUTH_TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body);
        let resp = send_with_retry(resp, "OpenAI token refresh").await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp
                .text()
                .await
                .map_err(|e| VigenError::http("OpenAI token refresh response body", e))?;
            return Err(VigenError::OAuth(format!(
                "token refresh failed (status {status}): {body}"
            )));
        }
        let tr: TokenResponse = resp
            .json()
            .await
            .map_err(|e| VigenError::http("OpenAI token refresh response JSON", e))?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        self.api_key = tr.access_token;
        self.refresh_token = Some(tr.refresh_token);
        self.expires_at = now + tr.expires_in;
        self.auth_dirty = true;
        Ok(())
    }

    pub fn write_auth_if_dirty(&self, config: &mut VigenConfig) -> Result<(), VigenError> {
        if !self.auth_dirty {
            return Ok(());
        }
        let Some(refresh_token) = self.refresh_token.clone() else {
            return Ok(());
        };
        config.auth.get_or_insert_with(Default::default).gpt = Some(GptAuth {
            access_token: self.api_key.clone(),
            refresh_token,
            expires_at: self.expires_at,
        });
        config.save()
    }
}

fn normalized_base_url(base_url: Option<&str>) -> String {
    base_url
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("https://api.openai.com")
        .trim_end_matches('/')
        .to_string()
}

fn normalized_endpoint(endpoint: Option<&str>) -> String {
    let endpoint = endpoint
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("/v1/images/generations")
        .trim();
    if endpoint.starts_with('/') {
        endpoint.to_string()
    } else {
        format!("/{endpoint}")
    }
}

fn extract_base64_images(resp: &serde_json::Value) -> Vec<String> {
    let mut images = Vec::new();
    collect_base64_images(resp, &mut images);
    images
}

fn extract_image_urls(resp: &serde_json::Value) -> Vec<String> {
    let mut urls = Vec::new();
    collect_strings(resp, &mut urls);
    urls
}

fn collect_strings(value: &serde_json::Value, urls: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) => {
            for url in extract_urls_from_text(s) {
                urls.push(url);
            }
        }
        serde_json::Value::Object(map) => {
            for v in map.values() {
                collect_strings(v, urls);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                collect_strings(v, urls);
            }
        }
        _ => {}
    }
}

fn extract_urls_from_text(text: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("![") {
        let after = &rest[start..];
        if let Some(paren_start) = after.find("](") {
            let url_start = start + paren_start + 2;
            let remaining = &text[url_start..];
            if let Some(end) = remaining.find(')') {
                let url = remaining[..end].to_string();
                if url.starts_with("http") {
                    urls.push(url);
                }
                rest = &remaining[end..];
                continue;
            }
        }
        rest = &after[2..];
    }
    urls
}

fn collect_base64_images(value: &serde_json::Value, images: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for key in ["b64_json", "b64", "base64", "image_base64"] {
                if let Some(image) = map.get(key).and_then(|v| v.as_str()) {
                    images.push(strip_data_url(image).to_string());
                }
            }
            for value in map.values() {
                collect_base64_images(value, images);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_base64_images(value, images);
            }
        }
        serde_json::Value::String(s) => {
            for image in extract_data_urls(s) {
                images.push(image.to_string());
            }
        }
        _ => {}
    }
}

fn extract_data_urls(text: &str) -> Vec<&str> {
    let mut images = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("data:image/") {
        let candidate = &rest[start..];
        let end = candidate
            .find(|c: char| c == ')' || c == ']' || c == '}' || c == '"' || c.is_whitespace())
            .unwrap_or(candidate.len());
        images.push(strip_data_url(&candidate[..end]));
        rest = &candidate[end..];
    }
    images
}

fn strip_data_url(image: &str) -> &str {
    image
        .split_once(",")
        .filter(|(prefix, _)| prefix.starts_with("data:image/"))
        .map(|(_, data)| data)
        .unwrap_or(image)
}

#[async_trait]
impl super::ImageGenProvider for GptProvider {
    async fn generate_image(
        &mut self,
        prompt: &str,
        size: &str,
        n: u8,
    ) -> Result<Vec<String>, VigenError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        if self.refresh_token.is_some() && self.expires_at <= now + 60 {
            self.refresh_access_token().await?;
        }
        let is_chat_completions = self.image_endpoint == "/v1/chat/completions";
        let body = if is_chat_completions {
            json!({
                "model": self.model,
                "messages": [
                    {
                        "role": "user",
                        "content": prompt
                    }
                ],
                "n": n,
                "size": size,
                "modalities": ["text", "image"]
            })
        } else {
            json!({
                "model": self.model,
                "prompt": prompt,
                "n": n,
                "size": size,
                "response_format": "b64_json"
            })
        };
        let url = format!("{}{}", self.base_url, self.image_endpoint);
        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body);
        let response = send_with_retry(response, "GPT image generation").await?;
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response
                .text()
                .await
                .map_err(|e| VigenError::http("GPT image generation error body", e))?;
            return Err(VigenError::ApiError {
                status,
                message: body,
            });
        }
        let resp: serde_json::Value = response
            .json()
            .await
            .map_err(|e| VigenError::http("GPT image generation response JSON", e))?;
        let images = extract_base64_images(&resp);
        if !images.is_empty() {
            return Ok(images);
        }
        let urls = extract_image_urls(&resp);
        if urls.is_empty() {
            return Err(VigenError::ApiError {
                status: 200,
                message: "no images in response".into(),
            });
        }
        let mut downloaded = Vec::with_capacity(urls.len());
        for url in &urls {
            let resp = send_with_retry(
                self.client.get(url),
                "GPT image download",
            )
            .await?;
            if !resp.status().is_success() {
                return Err(VigenError::ApiError {
                    status: resp.status().as_u16(),
                    message: format!("failed to download image from {url}"),
                });
            }
            let bytes = resp
                .bytes()
                .await
                .map_err(|e| VigenError::http("GPT image download body", e))?;
            downloaded.push(base64::engine::general_purpose::STANDARD.encode(&bytes));
        }
        Ok(downloaded)
    }
}

#[allow(dead_code)]
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
        model: "gpt-image-2".into(),
        base_url: None,
        image_endpoint: None,
        fallback_model: None,
        proxy: None,
        fallbacks: vec![],
    });
    gpt_cfg.api_key = Some(api_key);
    config.save()?;
    eprintln!("API key saved.");
    Ok(())
}

pub async fn login_oauth(
    config: &mut VigenConfig,
    proxy: Option<&str>,
) -> Result<(), VigenError> {
    let verifier = generate_code_verifier();
    let challenge = compute_code_challenge(&verifier);
    let state = generate_state();

    let auth_url = format!(
        "{OAUTH_AUTH_URL}?response_type=code&\
         client_id={}&redirect_uri={}&scope={}&\
         code_challenge={}&code_challenge_method=S256&state={}&\
         id_token_add_organizations=true&codex_cli_simplified_flow=true&originator=vigen",
        url_encode(OAUTH_CLIENT_ID),
        url_encode(OAUTH_REDIRECT_URI),
        url_encode(OAUTH_SCOPE),
        url_encode(&challenge),
        url_encode(&state),
    );

    let (port, listener) = crate::pkce::pick_port(&[1455]).await?;

    let url = auth_url.clone();
    eprintln!("OpenAI OAuth login URL:\n{url}\n\nWaiting for browser callback on {OAUTH_REDIRECT_URI} ...");
    let _ = webbrowser::open(&url);

    let (mut stream, _) = listener
        .accept()
        .await
        .map_err(|e| VigenError::OAuth(format!("accept: {e}")))?;
    let mut buf = vec![0u8; 4096];
    let n = stream
        .read(&mut buf)
        .await
        .map_err(|e| VigenError::OAuth(format!("read request: {e}")))?;
    let request = String::from_utf8_lossy(&buf[..n]);

    let first_line = request.lines().next().unwrap_or("");
    if !first_line.starts_with("GET ") {
        return Err(VigenError::OAuth("invalid callback request".into()));
    }
    let path = first_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/");
    let callback_url = format!("http://127.0.0.1:{port}{path}");
    let parsed = Url::parse(&callback_url)
        .map_err(|e| VigenError::OAuth(format!("invalid callback URL: {e}")))?;

    let pairs: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();

    let returned_state = pairs
        .get("state")
        .ok_or_else(|| VigenError::OAuth("no state in callback".into()))?;
    if returned_state != &state {
        return Err(VigenError::OAuth("state mismatch".into()));
    }
    let error = pairs.get("error").map(|s| s.to_string());
    if let Some(error) = error {
        return Err(VigenError::OAuth(error));
    }
    let code = pairs
        .get("code")
        .ok_or_else(|| VigenError::OAuth("no code in callback".into()))?;

    if let Err(e) = stream.write_all(SUCCESS_PAGE).await {
        return Err(VigenError::OAuth(format!("write response: {e}")));
    }

    let token_client = {
        let mut builder = Client::builder();
        if let Some(url) = proxy {
            builder = builder.proxy(
                reqwest::Proxy::all(url)
                    .map_err(|e| VigenError::OAuth(format!("proxy: {e}")))?,
            );
        }
        builder
            .build()
            .map_err(|e| VigenError::OAuth(format!("build client: {e}")))?
    };

    let body = format!(
        "grant_type=authorization_code&code={}&redirect_uri={}&client_id={}&code_verifier={}",
        url_encode(code),
        url_encode(OAUTH_REDIRECT_URI),
        url_encode(OAUTH_CLIENT_ID),
        url_encode(&verifier),
    );
    let resp = token_client
        .post(OAUTH_TOKEN_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body);
    let resp = send_with_retry(resp, "OpenAI OAuth token exchange")
        .await
        .map_err(|e| VigenError::OAuth(e.to_string()))?;
    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let body = resp
            .text()
            .await
            .map_err(|e| VigenError::http("OpenAI OAuth token exchange error body", e))?;
        return Err(VigenError::OAuth(format!(
            "token exchange failed (status {status}): {body}"
        )));
    }
    let tr: TokenResponse = resp
        .json()
        .await
        .map_err(|e| VigenError::OAuth(format!("parse token: {e}")))?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    config.auth.get_or_insert_with(Default::default).gpt = Some(GptAuth {
        access_token: tr.access_token,
        refresh_token: tr.refresh_token,
        expires_at: now + tr.expires_in,
    });
    config.providers.gpt.get_or_insert_with(|| GptConfig {
        api_key: None,
        model: "gpt-image-2".into(),
        base_url: None,
        image_endpoint: None,
        fallback_model: None,
        proxy: None,
        fallbacks: vec![],
    });
    config.save()
}
