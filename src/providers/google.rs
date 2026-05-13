use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use url::Url;

use crate::config::{resolve_proxy, GoogleAuth, VigenConfig};
use crate::error::VigenError;
use crate::pkce::{
    compute_code_challenge, generate_code_verifier, generate_state, pick_port, url_encode,
    SUCCESS_PAGE,
};

use super::VisionProvider;

const BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";
const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GEMINI_SCOPE: &str = "https://www.googleapis.com/auth/cloud-platform";

const GOOGLE_CLIENT_ID: &str =
    "681255809395-oo8ft2oprdrnp9e3aqf6av3hmdib135j.apps.googleusercontent.com";
const GOOGLE_CLIENT_SECRET: &str = "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";

#[derive(Debug, Deserialize)]
struct TokenResponse {
    #[allow(dead_code)]
    access_token: String,
    refresh_token: Option<String>,
}

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

    pub fn from_config_with_model(full: &VigenConfig, model: &str) -> Result<Self, VigenError> {
        let mut p = Self::from_config(full)?;
        p.model = model.to_string();
        Ok(p)
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
            .get(self.make_url("/models"))
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

pub async fn login(config: &mut VigenConfig, proxy: Option<&str>) -> Result<(), VigenError> {
    let (port, listener) = pick_port(&[18080, 18081, 18082, 18083, 52841]).await?;
    let redirect_uri = format!("http://localhost:{port}/callback");

    let code_verifier = generate_code_verifier();
    let code_challenge = compute_code_challenge(&code_verifier);
    let state = generate_state();

    let auth_url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&\
         scope={}&code_challenge={}&code_challenge_method=S256&state={}&\
         access_type=offline&prompt=consent",
        AUTH_URL,
        url_encode(GOOGLE_CLIENT_ID),
        url_encode(&redirect_uri),
        url_encode(GEMINI_SCOPE),
        &code_challenge,
        &state,
    );

    println!("Opening browser for Google authentication...");
    if webbrowser::open(&auth_url).is_err() {
        println!("Could not open browser. Open this URL:\n{auth_url}");
    }

    let (mut stream, _) = listener
        .accept()
        .await
        .map_err(|e| VigenError::OAuth(format!("accept: {e}")))?;

    let mut buf = [0u8; 8192];
    let n = stream
        .read(&mut buf)
        .await
        .map_err(|e| VigenError::OAuth(format!("read request: {e}")))?;

    let request = String::from_utf8_lossy(&buf[..n]);
    let request_line = request.lines().next().unwrap_or("");
    let parts: Vec<&str> = request_line.split_whitespace().collect();

    if parts.len() < 2 {
        stream
            .write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n")
            .await
            .ok();
        return Err(VigenError::OAuth("invalid callback request".into()));
    }

    let callback_url = Url::parse(&format!("http://localhost{}", parts[1]))
        .map_err(|e| VigenError::OAuth(format!("invalid callback URL: {e}")))?;

    let received_state = callback_url
        .query_pairs()
        .find(|(k, _)| k == "state")
        .map(|(_, v)| v.to_string())
        .unwrap_or_default();

    if received_state != state {
        stream
            .write_all(b"HTTP/1.1 400 Bad Request\r\n\r\nstate mismatch")
            .await
            .ok();
        return Err(VigenError::OAuth("state mismatch".into()));
    }

    let code = callback_url
        .query_pairs()
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.to_string())
        .ok_or_else(|| {
            let error = callback_url
                .query_pairs()
                .find(|(k, _)| k == "error")
                .map(|(_, v)| v.to_string())
                .unwrap_or_else(|| "unknown error".into());
            VigenError::OAuth(error)
        })?;

    stream
        .write_all(SUCCESS_PAGE)
        .await
        .map_err(|e| VigenError::OAuth(format!("write response: {e}")))?;

    let mut client_builder = Client::builder();
    if let Some(url) = proxy {
        client_builder = client_builder.proxy(
            reqwest::Proxy::all(url).map_err(|e| VigenError::OAuth(format!("proxy: {e}")))?,
        );
    }
    let client = client_builder
        .build()
        .map_err(|e| VigenError::OAuth(format!("build client: {e}")))?;

    let params = [
        ("code", code.as_str()),
        ("client_id", GOOGLE_CLIENT_ID),
        ("client_secret", GOOGLE_CLIENT_SECRET),
        ("code_verifier", &code_verifier),
        ("redirect_uri", &redirect_uri),
        ("grant_type", "authorization_code"),
    ];

    let response = client
        .post(TOKEN_URL)
        .form(&params)
        .send()
        .await
        .map_err(|e| VigenError::OAuth(format!("token request: {e}")))?;

    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(VigenError::OAuth(format!("token exchange failed: {body}")));
    }

    let token: TokenResponse = response
        .json()
        .await
        .map_err(|e| VigenError::OAuth(format!("parse token: {e}")))?;

    let refresh_token = token
        .refresh_token
        .ok_or_else(|| VigenError::OAuth("no refresh token returned".into()))?;

    let google_auth = GoogleAuth {
        client_id: GOOGLE_CLIENT_ID.to_string(),
        client_secret: GOOGLE_CLIENT_SECRET.to_string(),
        refresh_token,
    };

    let auth_cfg = config.auth.get_or_insert_with(Default::default);
    auth_cfg.google = Some(google_auth);
    config.save()?;

    Ok(())
}
