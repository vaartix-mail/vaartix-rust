use base64::Engine;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use std::collections::HashMap;
use vaartix::{verify_webhook_at, DEFAULT_TOLERANCE_SECS};

// Test-only secret: the worked example from the Standard Webhooks specification.
const PREFIX: &str = "whsec_";
const NOW: u64 = 1_614_265_330;
const BODY: &str = r#"{"test": 2432232314}"#;

fn secret() -> String {
    format!("{PREFIX}MfKQ9r8GKYqrTwjUPD8ILPZIo2LaLaSw")
}

fn headers() -> HashMap<&'static str, String> {
    HashMap::from([
        ("webhook-id", "msg_p5jXN8AQM9LWM0D4loKWxJek".to_owned()),
        ("webhook-timestamp", "1614265330".to_owned()),
        (
            "webhook-signature",
            "v1,g0hM9SsE+OTPJTGt/tmIKtSyZlE3uFJELVlNIOLJ1OE=".to_owned(),
        ),
    ])
}

#[test]
fn verifies_the_reference_example() {
    // The reference body isn't a Vaartix payload, so parsing fails after the signature passes.
    let h = headers();
    let err = verify_webhook_at(
        BODY.as_bytes(),
        |n| h.get(n).cloned(),
        &secret(),
        DEFAULT_TOLERANCE_SECS,
        NOW,
    )
    .unwrap_err();
    assert_eq!(err.0, "the body is not valid JSON");
}

#[test]
fn rejects_bad_requests() {
    let h = headers();
    let get = |n: &str| h.get(n).cloned();
    let cases = [
        verify_webhook_at(br#"{"test": 1}"#, get, &secret(), 300, NOW)
            .unwrap_err()
            .0,
        verify_webhook_at(BODY.as_bytes(), get, &format!("{PREFIX}c2VjcmV0"), 300, NOW)
            .unwrap_err()
            .0,
        verify_webhook_at(BODY.as_bytes(), get, &secret(), 300, NOW + 600)
            .unwrap_err()
            .0,
        verify_webhook_at(BODY.as_bytes(), |_| None, &secret(), 300, NOW)
            .unwrap_err()
            .0,
        verify_webhook_at(BODY.as_bytes(), get, "nope", 300, NOW)
            .unwrap_err()
            .0,
    ];
    assert_eq!(cases[0], "the webhook signature does not match");
    assert_eq!(cases[1], "the webhook signature does not match");
    assert_eq!(
        cases[2],
        "the webhook timestamp is too old or in the future"
    );
    assert!(cases[3].starts_with("missing"));
    assert_eq!(cases[4], "the secret should start with whsec_");
}

#[test]
fn parses_a_signed_bounce() {
    let body = r#"{"type":"email.bounced","createdAt":"x","data":{"emailId":"em_1","eventId":"evt_1","recipient":"old@example.com","from":"a@acme.in","to":["old@example.com"],"subject":"s","tags":[],"details":{"bounceType":"Permanent","diagnostics":[],"diagnosticCode":"550 5.1.1 user unknown","statusCode":"5.1.1"}}}"#;
    let b64 = base64::engine::general_purpose::STANDARD;
    let secret = format!(
        "{PREFIX}{}",
        b64.encode(b"0123456789abcdef0123456789abcdef")
    );
    let mut mac =
        Hmac::<Sha256>::new_from_slice(&b64.decode(&secret[PREFIX.len()..]).unwrap()).unwrap();
    mac.update(format!("whd_1.{NOW}.{body}").as_bytes());
    let sig = format!("v1,{}", b64.encode(mac.finalize().into_bytes()));
    let h = HashMap::from([
        ("webhook-id", "whd_1".to_owned()),
        ("webhook-timestamp", NOW.to_string()),
        (
            "webhook-signature",
            format!("v1,bm90LXRoaXM= {}", &sig[3..]).replacen(
                "v1,bm90LXRoaXM= ",
                "v1,bm90LXRoaXM= v1,",
                1,
            ),
        ),
    ]);
    let p = verify_webhook_at(body.as_bytes(), |n| h.get(n).cloned(), &secret, 300, NOW).unwrap();
    assert_eq!(p.kind, vaartix::events::BOUNCED);
    assert_eq!(p.data.recipient.as_deref(), Some("old@example.com"));
    assert_eq!(p.data.details["diagnosticCode"], "550 5.1.1 user unknown");
}
