use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use reqwest::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use url::{form_urlencoded, Url};

use crate::config::GoogleAuth;
use crate::error::VigenError;

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GEMINI_SCOPE: &str = "https://www.googleapis.com/auth/cloud-platform";

const GOOGLE_CLIENT_ID: &str =
    "681255809395-oo8ft2oprdrnp9e3aqf6av3hmdib135j.apps.googleusercontent.com";
const GOOGLE_CLIENT_SECRET: &str = "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";

const SUCCESS_PAGE: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
<!DOCTYPE html><html><head><meta charset=utf-8></head>\
<body style='font-family:sans-serif;text-align:center;padding-top:40px'>\
<h1>vigen: login successful</h1><p>You can close this tab.</p></body></html>";

#[derive(Debug, Deserialize)]
struct TokenResponse {
    #[allow(dead_code)]
    access_token: String,
    refresh_token: Option<String>,
}

pub async fn google_login(proxy_url: Option<&str>) -> Result<GoogleAuth, VigenError> {
    let (port, listener) = pick_port().await?;
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
    if let Some(ref url) = proxy_url {
        client_builder = client_builder.proxy(
            reqwest::Proxy::all(*url)
                .map_err(|e| VigenError::OAuth(format!("proxy: {e}")))?,
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

    Ok(GoogleAuth {
        client_id: GOOGLE_CLIENT_ID.to_string(),
        client_secret: GOOGLE_CLIENT_SECRET.to_string(),
        refresh_token,
    })
}

fn generate_code_verifier() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn compute_code_challenge(verifier: &str) -> String {
    let hash = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(hash)
}

fn generate_state() -> String {
    let mut bytes = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn url_encode(s: &str) -> String {
    form_urlencoded::byte_serialize(s.as_bytes()).collect::<String>()
}

async fn pick_port() -> Result<(u16, TcpListener), VigenError> {
    for port in [18080u16, 18081, 18082, 18083, 52841] {
        if let Ok(listener) = TcpListener::bind(format!("127.0.0.1:{port}")).await {
            return Ok((port, listener));
        }
    }
    Err(VigenError::OAuth("no available port for callback".into()))
}
