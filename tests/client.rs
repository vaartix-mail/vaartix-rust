use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use vaartix::{Client, SendEmail, UpdateDomain, UpdateWebhook};

#[derive(Default, Debug, Clone)]
struct Seen {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: String,
}

impl Seen {
    fn header(&self, name: &str) -> Option<String> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.clone())
    }
    fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body).unwrap_or(serde_json::Value::Null)
    }
}

/// A one-connection-at-a-time HTTP mock that answers every request with `status` and `body`.
async fn mock(status: u16, body: &'static str) -> (Client, Arc<Mutex<Seen>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let seen = Arc::new(Mutex::new(Seen::default()));
    let s = seen.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            let mut buf = Vec::new();
            let mut tmp = [0u8; 4096];
            let (head_end, content_len) = loop {
                let n = sock.read(&mut tmp).await.unwrap_or(0);
                if n == 0 {
                    return;
                }
                buf.extend_from_slice(&tmp[..n]);
                if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&buf[..pos]).to_string();
                    let len = head
                        .lines()
                        .find_map(|l| {
                            let (k, v) = l.split_once(':')?;
                            k.eq_ignore_ascii_case("content-length")
                                .then(|| v.trim().parse::<usize>().ok())
                                .flatten()
                        })
                        .unwrap_or(0);
                    break (pos + 4, len);
                }
            };
            while buf.len() < head_end + content_len {
                let n = sock.read(&mut tmp).await.unwrap_or(0);
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&tmp[..n]);
            }
            let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
            let mut lines = head.lines();
            let first: Vec<&str> = lines.next().unwrap_or("").split(' ').collect();
            let headers = lines
                .filter_map(|l| {
                    l.split_once(':')
                        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
                })
                .collect();
            *s.lock().unwrap() = Seen {
                method: first[0].to_owned(),
                path: first.get(1).copied().unwrap_or("").to_owned(),
                headers,
                body: String::from_utf8_lossy(&buf[head_end..]).to_string(),
            };
            let resp = format!("HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            let _ = sock.write_all(resp.as_bytes()).await;
        }
    });
    let client = Client::builder("vx_test")
        .base_url(format!("http://{addr}/"))
        .build()
        .unwrap();
    (client, seen)
}

#[tokio::test]
async fn sends_an_email() {
    let (c, seen) = mock(200, r#"{"id":"em_123"}"#).await;
    let email = SendEmail::new("Acme <orders@acme.in>", "priya@example.com", "Hi")
        .html("<p>Hi</p>")
        .reply_to("help@acme.in")
        .tracking(Some(true), None);
    let res = c.emails().send_idempotent(&email, "order-1").await.unwrap();
    assert_eq!(res.id, "em_123");
    let s = seen.lock().unwrap().clone();
    assert_eq!((s.method.as_str(), s.path.as_str()), ("POST", "/emails"));
    assert_eq!(s.header("authorization").unwrap(), "Bearer vx_test");
    assert_eq!(s.header("idempotency-key").unwrap(), "order-1");
    assert_eq!(
        s.header("user-agent").unwrap(),
        format!("vaartix-rust/{}", vaartix::VERSION)
    );
    let j = s.json();
    assert_eq!(j["replyTo"][0], "help@acme.in");
    assert!(j.get("text").is_none() && j.get("cc").is_none());
    assert_eq!(j["tracking"], serde_json::json!({"opens": true}));
}

#[tokio::test]
async fn errors() {
    let (c, _) = mock(
        422,
        r#"{"error":{"type":"validation_error","message":"to: not a valid email address."}}"#,
    )
    .await;
    let e = c
        .emails()
        .send(&SendEmail::new("a@acme.in", "x", "s").text("t"))
        .await
        .unwrap_err();
    assert_eq!((e.kind.as_str(), e.status), ("validation_error", Some(422)));
    let (c2, _) = mock(500, "oops").await;
    let e2 = c2.emails().get("em_1").await.unwrap_err();
    assert_eq!(e2.kind, "api_error");
    let dead = Client::builder("vx_test")
        .base_url("http://127.0.0.1:1")
        .build()
        .unwrap();
    let e3 = dead.emails().get("em_1").await.unwrap_err();
    assert_eq!((e3.kind.as_str(), e3.status), ("network_error", None));
    assert!(Client::new("").is_err());
}

#[tokio::test]
async fn reads_lists_and_cancels() {
    let (c, seen) = mock(200, r#"{"id":"em_1","status":"delivered","tracking":{"opens":true},"events":[{"type":"delivered","recipient":"a@b.in","data":{},"occurredAt":"x"}],"hasMore":true,"data":[]}"#).await;
    let e = c.emails().get("em_1/x").await.unwrap();
    assert_eq!(e.summary.status, "delivered");
    assert_eq!(e.tracking.unwrap().opens, Some(true));
    assert_eq!(e.events.len(), 1);
    assert_eq!(seen.lock().unwrap().path, "/emails/em_1%2Fx");
    let page = c.emails().list(Some(5), Some("em_9")).await.unwrap();
    assert!(page.has_more);
    assert_eq!(seen.lock().unwrap().path, "/emails?limit=5&before=em_9");
    c.emails().cancel("em_3").await.unwrap();
    assert_eq!(seen.lock().unwrap().path, "/emails/em_3/cancel");
}

#[tokio::test]
async fn batch_and_domains() {
    let (c, seen) = mock(200, r#"{"data":[{"id":"em_a"},{"id":"em_b"}]}"#).await;
    let out = c
        .batch()
        .send(&[SendEmail::new("a@acme.in", "x@y.in", "1").text("1")])
        .await
        .unwrap();
    assert_eq!(out.len(), 2);
    assert!(seen.lock().unwrap().json().is_array());
    let (c2, seen2) = mock(
        200,
        r#"{"id":"dom_1","name":"acme.in","openTracking":true,"clickTracking":false}"#,
    )
    .await;
    let d = c2
        .domains()
        .update(
            "dom_1",
            &UpdateDomain {
                open_tracking: Some(true),
                click_tracking: None,
            },
        )
        .await
        .unwrap();
    assert!(d.open_tracking);
    assert_eq!(d.raw["name"], "acme.in");
    let s = seen2.lock().unwrap().clone();
    assert_eq!(s.method, "PATCH");
    assert_eq!(s.json(), serde_json::json!({"openTracking": true}));
}

#[tokio::test]
async fn webhooks() {
    let (c, seen) = mock(200, r#"{"id":"wh_1","url":"https://acme.in/h","events":["email.delivered"],"status":"enabled","secret":"whsec_x","data":[]}"#).await;
    let w = c
        .webhooks()
        .create("https://acme.in/h", &[vaartix::events::DELIVERED])
        .await
        .unwrap();
    assert_eq!(w.secret.as_deref(), Some("whsec_x"));
    c.webhooks()
        .update(
            "wh_1",
            &UpdateWebhook {
                status: Some("disabled".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        seen.lock().unwrap().json(),
        serde_json::json!({"status": "disabled"})
    );
    c.webhooks().rotate_secret("wh_1").await.unwrap();
    assert_eq!(seen.lock().unwrap().path, "/webhooks/wh_1/rotate-secret");
    c.webhooks().deliveries("wh_1", Some(10)).await.unwrap();
    assert_eq!(
        seen.lock().unwrap().path,
        "/webhooks/wh_1/deliveries?limit=10"
    );
    let (c2, seen2) = mock(204, "").await;
    c2.webhooks().remove("wh_1").await.unwrap();
    assert_eq!(seen2.lock().unwrap().method, "DELETE");
}
