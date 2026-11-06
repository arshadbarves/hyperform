//! GitHub's device sign-in for Copilot over HTTP, one request per call.

mod support;

use serde_json::{Value, json};
use z_engine_llm::{DeviceCode, LlmError, LoginPoll, poll_login, start_login};

use support::server::{MockServer, Reply};

const CLIENT_ID: &str = "Ov23li8tweQw6odWQebz";

fn reply(body: Value) -> Reply {
    Reply::status(
        200,
        &[("content-type", "application/json")],
        &body.to_string(),
    )
}

#[tokio::test]
async fn sign_in_succeeds_after_pending_and_slow_down() {
    let server = MockServer::start(vec![
        reply(json!({
            "device_code": "dc_1", "user_code": "ABCD-1234",
            "verification_uri": "https://github.com/login/device",
            "expires_in": 900, "interval": 5,
        })),
        reply(json!({"error": "authorization_pending", "error_description": "pending"})),
        reply(json!({"error": "slow_down", "error_description": "too fast", "interval": 10})),
        reply(json!({"access_token": "gho_token", "token_type": "bearer", "scope": "read:user"})),
    ])
    .await;
    let http = reqwest::Client::new();
    let code = start_login(&http, &format!("{}/", server.url))
        .await
        .unwrap();
    assert_eq!(
        code,
        DeviceCode {
            user_code: "ABCD-1234".into(),
            verification_uri: "https://github.com/login/device".into(),
            device_code: "dc_1".into(),
            interval: 5,
            expires_in: 900,
        }
    );
    let mut polls = Vec::new();
    for _ in 0..3 {
        polls.push(
            poll_login(&http, &server.url, &code.device_code)
                .await
                .unwrap(),
        );
    }
    assert_eq!(
        polls,
        [
            LoginPoll::Pending,
            LoginPoll::SlowDown { interval: 10 },
            LoginPoll::Done("gho_token".into()),
        ]
    );

    let sent = server.requests();
    let start = &sent[0];
    assert_eq!(
        (start.method.as_str(), start.path.as_str()),
        ("POST", "/login/device/code")
    );
    assert_eq!(
        start.body,
        json!({"client_id": CLIENT_ID, "scope": "read:user"})
    );
    assert_eq!(start.headers["accept"], "application/json");
    assert_eq!(start.headers["content-type"], "application/json");
    assert!(
        start.headers["user-agent"]
            .to_str()
            .unwrap()
            .starts_with("opencode/")
    );
    for poll in &sent[1..] {
        assert_eq!(
            (poll.method.as_str(), poll.path.as_str()),
            ("POST", "/login/oauth/access_token")
        );
        assert_eq!(
            poll.body,
            json!({
                "client_id": CLIENT_ID,
                "device_code": "dc_1",
                "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
            })
        );
        assert_eq!(poll.headers["accept"], "application/json");
    }
}

#[tokio::test]
async fn expired_denied_and_refused_polls_fail() {
    let server = MockServer::start(vec![
        reply(json!({
            "error": "expired_token",
            "error_description": "The device_code has expired.",
        })),
        reply(json!({"error": "access_denied"})),
        Reply::status(503, &[], "unavailable"),
    ])
    .await;
    let http = reqwest::Client::new();
    let mut polls = Vec::new();
    for _ in 0..3 {
        polls.push(poll_login(&http, &server.url, "dc_1").await.unwrap());
    }
    assert_eq!(
        polls,
        [
            LoginPoll::Failed("expired_token: The device_code has expired.".into()),
            LoginPoll::Failed("access_denied".into()),
            LoginPoll::Failed("GitHub answered HTTP 503 Service Unavailable".into()),
        ]
    );
}

#[tokio::test]
async fn a_refused_or_unreachable_start_is_an_error() {
    let refused = r#"{"error":"Not Found"}"#;
    let server = MockServer::start(vec![Reply::status(404, &[], refused)]).await;
    let http = reqwest::Client::new();
    assert_eq!(
        start_login(&http, &server.url).await,
        Err(LlmError::Http {
            status: 404,
            body: "Not Found".into()
        })
    );
    assert!(matches!(
        start_login(&http, "http://127.0.0.1:9").await,
        Err(LlmError::Connect { attempts: 1, .. })
    ));
}
