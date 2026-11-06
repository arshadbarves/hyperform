use super::*;

const FAST: RetryPolicy = RetryPolicy {
    max_attempts: 3,
    base_backoff: Duration::from_millis(1),
    max_backoff: Duration::from_millis(4),
    max_server_delay: Duration::from_secs(60),
};

fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in pairs {
        map.insert(*name, value.parse().unwrap());
    }
    map
}

fn refused_url() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    format!("http://{address}/chat/completions")
}

#[test]
fn backoff_doubles_from_half_a_second_and_caps_at_sixteen() {
    let millis: Vec<u128> = (1..=7)
        .map(|retry| RetryPolicy::DEFAULT.backoff(retry).as_millis())
        .collect();
    assert_eq!(millis, [500, 1_000, 2_000, 4_000, 8_000, 16_000, 16_000]);
}

#[test]
fn server_delay_prefers_millis_and_caps_at_sixty_seconds() {
    let policy = RetryPolicy::DEFAULT;
    let both = headers(&[("retry-after-ms", "250"), ("retry-after", "9")]);
    assert_eq!(policy.server_delay(&both), Some(Duration::from_millis(250)));
    let seconds = headers(&[("retry-after", "1.5")]);
    assert_eq!(
        policy.server_delay(&seconds),
        Some(Duration::from_millis(1_500))
    );
    let huge = headers(&[("retry-after", "1e30")]);
    assert_eq!(policy.server_delay(&huge), Some(Duration::from_secs(60)));
    for bad in ["-1", "soon", "Wed, 21 Oct 2015 07:28:00 GMT", "NaN"] {
        assert_eq!(policy.server_delay(&headers(&[("retry-after", bad)])), None);
    }
}

#[test]
fn final_statuses_map_to_typed_errors() {
    let err = |code: u16| status_error(StatusCode::from_u16(code).unwrap(), "b".into(), 5);
    assert!(matches!(
        err(429),
        LlmError::RateLimited { attempts: 5, .. }
    ));
    assert!(matches!(err(529), LlmError::Overloaded { .. }));
    assert!(matches!(err(503), LlmError::Overloaded { .. }));
    assert_eq!(
        err(401),
        LlmError::Http {
            status: 401,
            body: "b".into()
        }
    );
    let empty = status_error(StatusCode::BAD_GATEWAY, " ".into(), 1);
    assert_eq!(
        empty,
        LlmError::Http {
            status: 502,
            body: "502 Bad Gateway".into()
        }
    );
    for code in [408, 429, 500, 502, 503, 504, 529] {
        assert!(is_retryable_status(StatusCode::from_u16(code).unwrap()));
    }
    for code in [400, 401, 403, 404, 413, 422, 501] {
        assert!(!is_retryable_status(StatusCode::from_u16(code).unwrap()));
    }
}

#[tokio::test]
async fn connect_refused_is_retried_then_reported() {
    let http = reqwest::Client::new();
    let url = refused_url();
    let build = || http.post(&url);
    let mut notices = Vec::new();
    let result = send_with_retry(&build, &FAST, &CancellationToken::new(), |notice| {
        notices.push(notice);
        async {}
    })
    .await;
    assert!(
        matches!(result, Err(LlmError::Connect { attempts: 3, .. })),
        "{result:?}"
    );
    let attempts: Vec<u32> = notices.iter().map(|notice| notice.attempt).collect();
    assert_eq!(attempts, [1, 2]);
    assert!(notices[0].reason.starts_with("connection failed"));
    assert_eq!(notices[1].delay, Duration::from_millis(2));
}

#[tokio::test]
async fn cancellation_interrupts_backoff() {
    let slow = RetryPolicy {
        base_backoff: Duration::from_secs(30),
        ..FAST
    };
    let http = reqwest::Client::new();
    let url = refused_url();
    let build = || http.post(&url);
    let cancel = CancellationToken::new();
    let started = std::time::Instant::now();
    let result = send_with_retry(&build, &slow, &cancel, |_| {
        cancel.cancel();
        async {}
    })
    .await;
    assert_eq!(result.unwrap_err(), LlmError::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(5));
}
