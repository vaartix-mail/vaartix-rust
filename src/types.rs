use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A name/value label on an email.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tag {
    /// Tag name.
    pub name: String,
    /// Tag value.
    pub value: String,
}

/// A file sent with an email.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    /// File name.
    pub filename: String,
    /// Base64-encoded content.
    pub content: String,
    /// Detected from the filename if `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// For inline images: reference it in your HTML as `cid:<content_id>`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_id: Option<String>,
}

/// Open and click tracking for one email. `None` uses the domain's default.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tracking {
    /// Add an open-tracking pixel.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opens: Option<bool>,
    /// Rewrite links to count clicks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clicks: Option<bool>,
}

/// One email to send. Set `html` or `text` (or both).
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendEmail {
    /// Sender, on a domain verified in Vaartix.
    pub from: String,
    /// Recipients.
    pub to: Vec<String>,
    /// Subject line.
    pub subject: String,
    /// HTML body.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,
    /// Plain-text body.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Cc recipients.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cc: Vec<String>,
    /// Bcc recipients.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub bcc: Vec<String>,
    /// Reply-To addresses.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub reply_to: Vec<String>,
    /// Extra headers.
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,
    /// Tags, returned in events and webhooks.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<Tag>,
    /// Attachments.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<Attachment>,
    /// ISO 8601 date-time to send later, e.g. `2026-11-01T09:00:00+05:30`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduled_at: Option<String>,
    /// Open and click tracking for this email, overriding the domain's defaults.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracking: Option<Tracking>,
}

impl SendEmail {
    /// Start an email with sender, one recipient and subject.
    pub fn new(from: impl Into<String>, to: impl Into<String>, subject: impl Into<String>) -> Self {
        Self {
            from: from.into(),
            to: vec![to.into()],
            subject: subject.into(),
            ..Default::default()
        }
    }
    /// Set the HTML body.
    pub fn html(mut self, html: impl Into<String>) -> Self {
        self.html = Some(html.into());
        self
    }
    /// Set the plain-text body.
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }
    /// Add a recipient.
    pub fn add_to(mut self, to: impl Into<String>) -> Self {
        self.to.push(to.into());
        self
    }
    /// Add a Reply-To address.
    pub fn reply_to(mut self, addr: impl Into<String>) -> Self {
        self.reply_to.push(addr.into());
        self
    }
    /// Add a tag.
    pub fn tag(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.tags.push(Tag {
            name: name.into(),
            value: value.into(),
        });
        self
    }
    /// Send later (ISO 8601).
    pub fn scheduled_at(mut self, at: impl Into<String>) -> Self {
        self.scheduled_at = Some(at.into());
        self
    }
    /// Turn tracking on or off for this email (`None` keeps the domain's default).
    pub fn tracking(mut self, opens: Option<bool>, clicks: Option<bool>) -> Self {
        self.tracking = Some(Tracking { opens, clicks });
        self
    }
}

/// The new email's ID.
#[derive(Debug, Clone, Deserialize)]
pub struct SendEmailResponse {
    /// `em_...`
    pub id: String,
}

/// One step in an email's timeline.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailEvent {
    /// Event type.
    #[serde(rename = "type")]
    pub kind: String,
    /// Recipient this event is about, if any.
    pub recipient: Option<String>,
    /// Event data.
    #[serde(default)]
    pub data: serde_json::Value,
    /// When it happened.
    pub occurred_at: String,
}

/// An email as listed.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailSummary {
    /// `em_...`
    pub id: String,
    /// Sender.
    #[serde(default)]
    pub from: String,
    /// Recipients.
    #[serde(default)]
    pub to: Vec<String>,
    /// Subject.
    #[serde(default)]
    pub subject: String,
    /// Status, e.g. `delivered`.
    #[serde(default)]
    pub status: String,
    /// Tags.
    #[serde(default)]
    pub tags: Vec<Tag>,
    /// When scheduled to send.
    pub scheduled_at: Option<String>,
    /// When sent.
    pub sent_at: Option<String>,
    /// Error, if it failed.
    pub error: Option<String>,
    /// When created.
    #[serde(default)]
    pub created_at: String,
}

/// One email with its content and events.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Email {
    /// Fields shared with the list view.
    #[serde(flatten)]
    pub summary: EmailSummary,
    /// HTML body.
    pub html: Option<String>,
    /// Plain-text body.
    pub text: Option<String>,
    /// The email's own tracking setting, or `None` when it uses the domain's defaults.
    pub tracking: Option<Tracking>,
    /// Timeline.
    #[serde(default)]
    pub events: Vec<EmailEvent>,
}

/// One page of emails.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListEmails {
    /// Emails, newest first.
    pub data: Vec<EmailSummary>,
    /// More pages exist.
    pub has_more: bool,
}

/// A sending domain. Only the tracking fields are typed; `raw` has the full JSON.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Domain {
    /// Domain ID.
    pub id: String,
    /// Default open tracking.
    #[serde(default)]
    pub open_tracking: bool,
    /// Default click tracking.
    #[serde(default)]
    pub click_tracking: bool,
    /// Every other field.
    #[serde(flatten)]
    pub raw: serde_json::Map<String, serde_json::Value>,
}

/// Change a domain's tracking defaults. `None` keeps the current value.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDomain {
    /// Default open tracking.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_tracking: Option<bool>,
    /// Default click tracking.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub click_tracking: Option<bool>,
}

/// A webhook endpoint.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Webhook {
    /// `wh_...`
    pub id: String,
    /// Endpoint URL.
    #[serde(default)]
    pub url: String,
    /// Subscribed events.
    #[serde(default)]
    pub events: Vec<String>,
    /// `enabled` or `disabled`.
    #[serde(default)]
    pub status: String,
    /// The `whsec_` signing secret: only returned when created or rotated.
    pub secret: Option<String>,
}

/// Change an endpoint. `None` keeps the current value.
#[derive(Debug, Clone, Default, Serialize)]
pub struct UpdateWebhook {
    /// New URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// New events.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<String>>,
    /// `enabled` or `disabled`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

/// One request Vaartix made to your endpoint.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookDelivery {
    /// Also sent as the `webhook-id` header.
    pub id: String,
    /// Event type.
    #[serde(rename = "type", default)]
    pub kind: String,
    /// `pending`, `succeeded` or `failed`.
    pub status: String,
    /// Attempts so far.
    #[serde(default)]
    pub attempts: u32,
    /// Your server's HTTP status.
    pub response_status: Option<u16>,
    /// The body that was sent.
    #[serde(default)]
    pub payload: serde_json::Value,
}

#[derive(Deserialize)]
pub(crate) struct DataList<T> {
    pub(crate) data: Vec<T>,
}

/// Webhook event types.
pub mod events {
    /// Accepted by the mail servers.
    pub const SENT: &str = "email.sent";
    /// Accepted by the recipient's server.
    pub const DELIVERED: &str = "email.delivered";
    /// The receiving server is deferring it; details carry its exact reply.
    pub const DELIVERY_DELAYED: &str = "email.delivery_delayed";
    /// Bounced; details carry the receiving server's reply.
    pub const BOUNCED: &str = "email.bounced";
    /// Marked as spam.
    pub const COMPLAINED: &str = "email.complained";
    /// Rejected before sending.
    pub const REJECTED: &str = "email.rejected";
    /// The template could not be rendered.
    pub const RENDERING_FAILURE: &str = "email.rendering_failure";
    /// Not sent because the address is suppressed.
    pub const SUPPRESSED: &str = "email.suppressed";
    /// Opened (machine opens flagged).
    pub const OPENED: &str = "email.opened";
    /// A link was clicked (scanner clicks flagged).
    pub const CLICKED: &str = "email.clicked";
}

/// The JSON body Vaartix posts to your endpoint.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookPayload {
    /// Event type, see [`events`].
    #[serde(rename = "type")]
    pub kind: String,
    /// When the event happened. Deliveries can arrive out of order; sort by this.
    #[serde(default)]
    pub created_at: String,
    /// The email and event details.
    pub data: WebhookData,
}

/// Email fields in a webhook. `details` depends on the event type.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookData {
    /// `em_...`
    #[serde(default)]
    pub email_id: String,
    /// `evt_...`
    #[serde(default)]
    pub event_id: String,
    /// `None` when the email had more than one recipient.
    pub recipient: Option<String>,
    /// Sender.
    #[serde(default)]
    pub from: String,
    /// Recipients.
    #[serde(default)]
    pub to: Vec<String>,
    /// Subject.
    #[serde(default)]
    pub subject: String,
    /// Tags.
    #[serde(default)]
    pub tags: Vec<Tag>,
    /// Event details: for opens/clicks `machine`, `link`…; for delays and bounces `diagnosticCode`
    /// (the receiving server's exact reply) and `statusCode`. Decode with serde as needed.
    #[serde(default)]
    pub details: serde_json::Value,
}
