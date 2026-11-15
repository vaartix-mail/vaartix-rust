//! The official Rust client for [Vaartix](https://vaartix.com), the transactional email API built in India.
//!
//! ```no_run
//! # async fn run() -> Result<(), vaartix::Error> {
//! let client = vaartix::Client::from_env()?; // reads VAARTIX_API_KEY
//! let sent = client
//!     .emails()
//!     .send(&vaartix::SendEmail::new("Acme <orders@acme.in>", "priya@example.com", "Your order is confirmed")
//!         .html("<p>Thanks for your order!</p>"))
//!     .await?;
//! println!("{}", sent.id);
//! # Ok(()) }
//! ```
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod types;
mod webhook;

pub use types::*;
pub use webhook::{verify_webhook, verify_webhook_at, WebhookError, DEFAULT_TOLERANCE_SECS};

use serde::de::DeserializeOwned;
use serde::Serialize;
use std::time::Duration;

/// This SDK's version, sent in the User-Agent header.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// The Vaartix API.
pub const DEFAULT_BASE_URL: &str = "https://api.vaartix.com";

/// The API answered with an error, or the request could not be made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    /// The API's error type, for example `validation_error`, or `network_error`.
    pub kind: String,
    /// What went wrong.
    pub message: String,
    /// The HTTP status, or `None` when no response was received.
    pub status: Option<u16>,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.status {
            Some(s) => write!(f, "vaartix: {} ({}): {}", self.kind, s, self.message),
            None => write!(f, "vaartix: {}: {}", self.kind, self.message),
        }
    }
}

impl std::error::Error for Error {}

fn err(kind: &str, message: impl Into<String>, status: Option<u16>) -> Error {
    Error {
        kind: kind.into(),
        message: message.into(),
        status,
    }
}

/// The Vaartix client. Cheap to clone; share one across your app.
#[derive(Clone, Debug)]
pub struct Client {
    api_key: String,
    base_url: String,
    http: reqwest::Client,
}

impl Client {
    /// Create a client with an API key.
    pub fn new(api_key: impl Into<String>) -> Result<Self, Error> {
        Self::builder(api_key).build()
    }

    /// Create a client with the key from the `VAARTIX_API_KEY` environment variable.
    pub fn from_env() -> Result<Self, Error> {
        Self::new(std::env::var("VAARTIX_API_KEY").unwrap_or_default())
    }

    /// Configure the base URL or timeout before building.
    pub fn builder(api_key: impl Into<String>) -> ClientBuilder {
        ClientBuilder {
            api_key: api_key.into(),
            base_url: DEFAULT_BASE_URL.into(),
            timeout: Duration::from_secs(30),
        }
    }

    /// Send and read emails.
    pub fn emails(&self) -> Emails<'_> {
        Emails(self)
    }
    /// Send up to 100 emails in one request.
    pub fn batch(&self) -> Batch<'_> {
        Batch(self)
    }
    /// Sending domains.
    pub fn domains(&self) -> Domains<'_> {
        Domains(self)
    }
    /// Webhook endpoints.
    pub fn webhooks(&self) -> Webhooks<'_> {
        Webhooks(self)
    }

    async fn request<T: DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&B>,
        idempotency_key: Option<&str>,
    ) -> Result<T, Error> {
        let mut req = self
            .http
            .request(method, format!("{}{}", self.base_url, path))
            .bearer_auth(&self.api_key)
            .header("User-Agent", format!("vaartix-rust/{VERSION}"))
            .header("Accept", "application/json");
        if let Some(k) = idempotency_key {
            req = req.header("Idempotency-Key", k);
        }
        if let Some(b) = body {
            req = req.json(b);
        }
        let res = req
            .send()
            .await
            .map_err(|e| err("network_error", e.to_string(), None))?;
        let status = res.status().as_u16();
        let bytes = res
            .bytes()
            .await
            .map_err(|e| err("network_error", e.to_string(), Some(status)))?;
        if !(200..300).contains(&status) {
            let parsed: Option<serde_json::Value> = serde_json::from_slice(&bytes).ok();
            let e = parsed.as_ref().and_then(|v| v.get("error"));
            let kind = e
                .and_then(|e| e.get("type"))
                .and_then(|v| v.as_str())
                .unwrap_or("api_error");
            let message = e
                .and_then(|e| e.get("message"))
                .and_then(|v| v.as_str())
                .map(str::to_owned)
                .unwrap_or_else(|| format!("Request failed with status {status}"));
            return Err(err(kind, message, Some(status)));
        }
        let bytes = if bytes.is_empty() {
            &b"null"[..]
        } else {
            &bytes[..]
        };
        serde_json::from_slice(bytes).map_err(|e| {
            err(
                "api_error",
                format!("Could not read the response: {e}"),
                Some(status),
            )
        })
    }
}

/// Builder for [`Client`].
#[derive(Debug)]
pub struct ClientBuilder {
    api_key: String,
    base_url: String,
    timeout: Duration,
}

impl ClientBuilder {
    /// Point the client at another API URL, for example a local mock.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into().trim_end_matches('/').to_owned();
        self
    }
    /// Request timeout (default 30 seconds).
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
    /// Build the client.
    pub fn build(self) -> Result<Client, Error> {
        if self.api_key.is_empty() {
            return Err(err(
                "invalid_request",
                "Missing API key. Pass it to Client::new or set VAARTIX_API_KEY.",
                None,
            ));
        }
        let http = reqwest::Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|e| err("invalid_request", e.to_string(), None))?;
        Ok(Client {
            api_key: self.api_key,
            base_url: self.base_url,
            http,
        })
    }
}

fn esc(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

use reqwest::Method;
const NO_BODY: Option<&()> = None;

/// Send and read emails.
pub struct Emails<'a>(&'a Client);

impl Emails<'_> {
    /// Send one email.
    pub async fn send(&self, email: &SendEmail) -> Result<SendEmailResponse, Error> {
        self.0
            .request(Method::POST, "/emails", Some(email), None)
            .await
    }
    /// Send one email; retrying with the same idempotency key within 24 hours returns the first result.
    pub async fn send_idempotent(
        &self,
        email: &SendEmail,
        idempotency_key: &str,
    ) -> Result<SendEmailResponse, Error> {
        self.0
            .request(Method::POST, "/emails", Some(email), Some(idempotency_key))
            .await
    }
    /// One email with its events. Needs a full-access key.
    pub async fn get(&self, id: &str) -> Result<Email, Error> {
        self.0
            .request(Method::GET, &format!("/emails/{}", esc(id)), NO_BODY, None)
            .await
    }
    /// Recent emails, newest first. `limit` 1–100 (None = 20); `before` pages back from an email ID.
    pub async fn list(
        &self,
        limit: Option<u32>,
        before: Option<&str>,
    ) -> Result<ListEmails, Error> {
        let mut q = Vec::new();
        if let Some(l) = limit {
            q.push(format!("limit={l}"));
        }
        if let Some(b) = before {
            q.push(format!("before={}", esc(b)));
        }
        let path = if q.is_empty() {
            "/emails".to_owned()
        } else {
            format!("/emails?{}", q.join("&"))
        };
        self.0.request(Method::GET, &path, NO_BODY, None).await
    }
    /// Cancel a scheduled email before it is sent. Needs a full-access key.
    pub async fn cancel(&self, id: &str) -> Result<EmailSummary, Error> {
        self.0
            .request(
                Method::POST,
                &format!("/emails/{}/cancel", esc(id)),
                NO_BODY,
                None,
            )
            .await
    }
}

/// Send many emails in one request.
pub struct Batch<'a>(&'a Client);

impl Batch<'_> {
    /// Up to 100 emails (no attachments or scheduling). All or nothing.
    pub async fn send(&self, emails: &[SendEmail]) -> Result<Vec<SendEmailResponse>, Error> {
        let out: DataList<SendEmailResponse> = self
            .0
            .request(Method::POST, "/emails/batch", Some(emails), None)
            .await?;
        Ok(out.data)
    }
}

/// Sending domains. Changes need a full-access key from an owner or admin.
pub struct Domains<'a>(&'a Client);

impl Domains<'_> {
    /// Change a domain's default open and click tracking. `None` fields keep their value.
    pub async fn update(&self, id: &str, update: &UpdateDomain) -> Result<Domain, Error> {
        self.0
            .request(
                Method::PATCH,
                &format!("/domains/{}", esc(id)),
                Some(update),
                None,
            )
            .await
    }
}

/// Webhook endpoints. Needs a full-access key.
pub struct Webhooks<'a>(&'a Client);

impl Webhooks<'_> {
    /// Create an endpoint. The response includes the signing secret, shown only this once.
    pub async fn create(&self, url: &str, events: &[&str]) -> Result<Webhook, Error> {
        let body = serde_json::json!({ "url": url, "events": events });
        self.0
            .request(Method::POST, "/webhooks", Some(&body), None)
            .await
    }
    /// All endpoints.
    pub async fn list(&self) -> Result<Vec<Webhook>, Error> {
        let out: DataList<Webhook> = self
            .0
            .request(Method::GET, "/webhooks", NO_BODY, None)
            .await?;
        Ok(out.data)
    }
    /// One endpoint.
    pub async fn get(&self, id: &str) -> Result<Webhook, Error> {
        self.0
            .request(
                Method::GET,
                &format!("/webhooks/{}", esc(id)),
                NO_BODY,
                None,
            )
            .await
    }
    /// Change the URL or events, or turn the endpoint off (`disabled`) and on (`enabled`).
    pub async fn update(&self, id: &str, update: &UpdateWebhook) -> Result<Webhook, Error> {
        self.0
            .request(
                Method::PATCH,
                &format!("/webhooks/{}", esc(id)),
                Some(update),
                None,
            )
            .await
    }
    /// Delete an endpoint.
    pub async fn remove(&self, id: &str) -> Result<(), Error> {
        let _: serde_json::Value = self
            .0
            .request(
                Method::DELETE,
                &format!("/webhooks/{}", esc(id)),
                NO_BODY,
                None,
            )
            .await?;
        Ok(())
    }
    /// Replace the signing secret. The old one stops working immediately.
    pub async fn rotate_secret(&self, id: &str) -> Result<Webhook, Error> {
        self.0
            .request(
                Method::POST,
                &format!("/webhooks/{}/rotate-secret", esc(id)),
                NO_BODY,
                None,
            )
            .await
    }
    /// Recent deliveries, newest first.
    pub async fn deliveries(
        &self,
        id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<WebhookDelivery>, Error> {
        let q = limit.map(|l| format!("?limit={l}")).unwrap_or_default();
        let out: DataList<WebhookDelivery> = self
            .0
            .request(
                Method::GET,
                &format!("/webhooks/{}/deliveries{q}", esc(id)),
                NO_BODY,
                None,
            )
            .await?;
        Ok(out.data)
    }
    /// Send a delivery again, with the same webhook-id.
    pub async fn retry_delivery(
        &self,
        id: &str,
        delivery_id: &str,
    ) -> Result<WebhookDelivery, Error> {
        let path = format!(
            "/webhooks/{}/deliveries/{}/retry",
            esc(id),
            esc(delivery_id)
        );
        self.0.request(Method::POST, &path, NO_BODY, None).await
    }
}
