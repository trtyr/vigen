use reqwest::{RequestBuilder, Response, StatusCode};
use tokio::time::{sleep, Duration};

use crate::error::VigenError;

const MAX_ATTEMPTS: usize = 3;

pub async fn send_with_retry(
    request: RequestBuilder,
    context: &str,
) -> Result<Response, VigenError> {
    let mut last_error = None;

    for attempt in 1..=MAX_ATTEMPTS {
        let Some(next_request) = request.try_clone() else {
            return request
                .send()
                .await
                .map_err(|source| VigenError::Http {
                    context: context.to_string(),
                    source,
                });
        };

        match next_request.send().await {
            Ok(response) => {
                if should_retry_status(response.status()) && attempt < MAX_ATTEMPTS {
                    wait_before_retry(context, attempt, format!("status {}", response.status()))
                        .await;
                    continue;
                }
                return Ok(response);
            }
            Err(source) => {
                if should_retry_error(&source) && attempt < MAX_ATTEMPTS {
                    let reason = source.to_string();
                    last_error = Some(source);
                    wait_before_retry(context, attempt, reason).await;
                    continue;
                }
                return Err(VigenError::Http {
                    context: context.to_string(),
                    source,
                });
            }
        }
    }

    Err(VigenError::Http {
        context: context.to_string(),
        source: last_error.expect("retry loop always returns on success or final error"),
    })
}

async fn wait_before_retry(context: &str, attempt: usize, reason: String) {
    let delay = Duration::from_millis(400 * attempt as u64);
    eprintln!(
        "{context} failed transiently ({reason}); retrying attempt {}/{} in {}ms",
        attempt + 1,
        MAX_ATTEMPTS,
        delay.as_millis()
    );
    sleep(delay).await;
}

fn should_retry_status(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

fn should_retry_error(error: &reqwest::Error) -> bool {
    error.is_connect() || error.is_timeout() || error.is_request()
}
