# Vaartix Rust SDK

The official Rust client for [Vaartix](https://vaartix.com), the transactional email API built in India.
Data stored in India (AWS Mumbai), pricing in ₹.

```sh
cargo add vaartix
```

Async (Tokio), reqwest with rustls (no OpenSSL needed). Rust 1.88 or newer.

## Send an email

```rust
use vaartix::{Client, SendEmail};

#[tokio::main]
async fn main() -> Result<(), vaartix::Error> {
    let client = Client::from_env()?; // reads VAARTIX_API_KEY

    let email = SendEmail::new("Acme <orders@acme.in>", "priya@example.com", "Your order is confirmed")
        .html("<p>Thanks for your order!</p>");
    let sent = client.emails().send(&email).await?;
    println!("{}", sent.id); // em_...
    Ok(())
}
```

`from` must be an address on a domain verified in Vaartix. Testing? Send to `delivered@test.vaartix.com`
(or `bounced@` / `complained@`). Nothing real is sent.

## Errors

Every call returns `Result<_, vaartix::Error>` with the API's `kind` (e.g. `validation_error`), `message` and
`status` (`None` with kind `network_error` when no response was received).

## More sending options

```rust
client.emails().send_idempotent(&email, "receipt-10482").await?;     // safe retries for 24 hours
let later = email.clone().scheduled_at("2026-11-15T10:00:00+05:30"); // send later
client.batch().send(&[email_a, email_b]).await?;                     // up to 100 in one request
let tracked = email.tracking(Some(true), Some(false));               // opens on, clicks off
```

Keep click tracking off for login codes, password resets and magic links.

## Read emails

Needs a full-access key.

```rust
let email = client.emails().get("em_...").await?;            // content, status and events
let page = client.emails().list(Some(50), None).await?;
client.emails().cancel("em_...").await?;                     // a scheduled email
```

## Webhooks

Check that a webhook request really came from Vaartix before you trust it. Use the **raw** body.

```rust
// axum
async fn webhook(headers: axum::http::HeaderMap, body: axum::body::Bytes) -> axum::http::StatusCode {
    let secret = std::env::var("VAARTIX_WEBHOOK_SECRET").unwrap();
    let get = |name: &str| headers.get(name).and_then(|v| v.to_str().ok()).map(str::to_owned);
    match vaartix::verify_webhook(&body, get, &secret) {
        Ok(event) if event.kind == vaartix::events::BOUNCED => {
            let reply = &event.data.details["diagnosticCode"]; // the receiving server's reply
            axum::http::StatusCode::OK
        }
        Ok(_) => axum::http::StatusCode::OK,
        Err(_) => axum::http::StatusCode::BAD_REQUEST,
    }
}
```

Signatures follow the [Standard Webhooks](https://www.standardwebhooks.com) spec. Manage endpoints with
`client.webhooks()`: `create`, `list`, `get`, `update`, `remove`, `rotate_secret`, `deliveries`, `retry_delivery`.

## Domains

```rust
client.domains().update(domain_id, &vaartix::UpdateDomain { open_tracking: Some(true), ..Default::default() }).await?;
```

## Docs

- Docs: https://vaartix.com/docs
- API reference: https://vaartix.com/docs/api
- Crate docs: https://docs.rs/vaartix

## Contributing and security

Issues and pull requests are welcome. Report security problems privately, see [SECURITY.md](SECURITY.md).

## License

MIT
