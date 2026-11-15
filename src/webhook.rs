use crate::WebhookPayload;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};
use subtle::ConstantTimeEq;

/// How old a webhook's timestamp may be, in seconds.
pub const DEFAULT_TOLERANCE_SECS: u64 = 300;

/// A webhook request's signature, timestamp or body is not valid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookError(pub String);

impl std::fmt::Display for WebhookError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "vaartix webhook verification failed: {}", self.0)
    }
}

impl std::error::Error for WebhookError {}

/// Check that a request came from Vaartix (Standard Webhooks) and parse its body.
///
/// `payload` is the raw body. `header` looks a header up by its lowercase name, for example
/// `|name| req.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_owned)`.
pub fn verify_webhook<F>(
    payload: &[u8],
    header: F,
    secret: &str,
) -> Result<WebhookPayload, WebhookError>
where
    F: Fn(&str) -> Option<String>,
{
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    verify_webhook_at(payload, header, secret, DEFAULT_TOLERANCE_SECS, now)
}

/// [`verify_webhook`] with an explicit tolerance and current time (Unix seconds).
pub fn verify_webhook_at<F>(
    payload: &[u8],
    header: F,
    secret: &str,
    tolerance_secs: u64,
    now: u64,
) -> Result<WebhookPayload, WebhookError>
where
    F: Fn(&str) -> Option<String>,
{
    let fail = |m: &str| WebhookError(m.to_owned());
    let (Some(id), Some(ts), Some(sigs)) = (
        header("webhook-id"),
        header("webhook-timestamp"),
        header("webhook-signature"),
    ) else {
        return Err(fail(
            "missing webhook-id, webhook-timestamp or webhook-signature header",
        ));
    };
    let sent: u64 = ts
        .parse()
        .map_err(|_| fail("the webhook timestamp is too old or in the future"))?;
    if now.abs_diff(sent) > tolerance_secs {
        return Err(fail("the webhook timestamp is too old or in the future"));
    }
    let key = secret
        .strip_prefix("whsec_")
        .ok_or_else(|| fail("the secret should start with whsec_"))?;
    let key = B64
        .decode(key)
        .map_err(|_| fail("the secret is not valid base64"))?;
    let mut mac =
        Hmac::<Sha256>::new_from_slice(&key).map_err(|_| fail("the secret is not valid"))?;
    mac.update(format!("{id}.{ts}.").as_bytes());
    mac.update(payload);
    let expected = mac.finalize().into_bytes();
    // The header can hold several space-separated signatures, e.g. while a secret is rotated.
    let valid = sigs.split(' ').any(|entry| match entry.split_once(',') {
        Some(("v1", sig)) => B64
            .decode(sig)
            .map(|given| given.ct_eq(&expected).into())
            .unwrap_or(false),
        _ => false,
    });
    if !valid {
        return Err(fail("the webhook signature does not match"));
    }
    serde_json::from_slice(payload).map_err(|_| fail("the body is not valid JSON"))
}
