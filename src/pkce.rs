use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use url::form_urlencoded;

use crate::error::VigenError;

pub(crate) const SUCCESS_PAGE: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
<!DOCTYPE html><html><head><meta charset=utf-8></head>\
<body style='font-family:sans-serif;text-align:center;padding-top:40px'>\
<h1>vigen: login successful</h1><p>You can close this tab.</p></body></html>";

pub(crate) fn generate_code_verifier() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub(crate) fn compute_code_challenge(verifier: &str) -> String {
    let hash = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(hash)
}

pub(crate) fn generate_state() -> String {
    let mut bytes = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub(crate) fn url_encode(s: &str) -> String {
    form_urlencoded::byte_serialize(s.as_bytes()).collect::<String>()
}

pub(crate) async fn pick_port(preferred: &[u16]) -> Result<(u16, TcpListener), VigenError> {
    for &port in preferred {
        if let Ok(listener) = TcpListener::bind(format!("127.0.0.1:{port}")).await {
            return Ok((port, listener));
        }
    }
    Err(VigenError::OAuth("no available port for callback".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_verifier_length() {
        let v = generate_code_verifier();
        assert_eq!(v.len(), 43, "32 bytes → 43 base64url chars");
        assert!(v.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn test_code_verifier_randomized() {
        let a = generate_code_verifier();
        let b = generate_code_verifier();
        assert_ne!(a, b, "successive calls must produce different values");
    }

    #[test]
    fn test_code_challenge_deterministic() {
        let v = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let c = compute_code_challenge(v);
        assert_eq!(
            c, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
            "known verifier → known challenge"
        );
    }

    #[test]
    fn test_code_challenge_output_format() {
        let v = generate_code_verifier();
        let c = compute_code_challenge(&v);
        assert_eq!(c.len(), 43, "SHA-256 hash → 32 bytes → 43 base64url chars");
        assert!(c.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn test_generate_state_length() {
        let s = generate_state();
        assert_eq!(s.len(), 22, "16 bytes → 22 base64url chars");
    }

    #[test]
    fn test_generate_state_randomized() {
        let a = generate_state();
        let b = generate_state();
        assert_ne!(a, b);
    }

    #[test]
    fn test_url_encode_simple() {
        assert_eq!(url_encode("hello"), "hello");
    }

    #[test]
    fn test_url_encode_spaces_as_plus() {
        let encoded = url_encode("hello world");
        assert_eq!(encoded, "hello+world", "form-urlencoded uses + for space");
    }

    #[test]
    fn test_url_encode_special_chars() {
        let encoded = url_encode("user@example.com");
        assert!(encoded.contains("%40"));
    }

    #[tokio::test]
    async fn test_pick_port_returns_requested_port() {
        let (port, _listener) = pick_port(&[19876]).await.unwrap();
        assert_eq!(port, 19876, "returns the requested port verbatim");
    }

    #[tokio::test]
    async fn test_pick_port_skips_taken_and_finds_free() {
        let first = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let taken = first.local_addr().unwrap().port();
        let (port, _l) = pick_port(&[taken, 19877]).await.unwrap();
        assert_eq!(port, 19877, "should skip taken port and return the free one");
    }
}
