use super::*;

const FIXTURE: &str = include_str!("../../../tests/fixtures/models_dev.json");

fn catalog() -> ModelCatalog {
    ModelCatalog::from_models_dev_json(FIXTURE).unwrap()
}

async fn serve(status: u16, body: &'static str) -> String {
    let status = axum::http::StatusCode::from_u16(status).unwrap();
    let app = axum::Router::new().fallback(move || async move { (status, body) });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}/api.json")
}

#[test]
fn keeps_every_provider_in_preference_order() {
    let catalog = catalog();
    let providers: Vec<&str> = catalog
        .models
        .iter()
        .map(|model| model.provider.as_str())
        .collect();
    assert_eq!(
        providers,
        [
            "openrouter",
            "openrouter",
            "openrouter",
            "anthropic",
            "anthropic",
            "openai",
            "openai",
            "opencode",
            "opencode",
            "opencode",
            "github-copilot",
        ]
    );
    let ids: Vec<&str> = catalog.models[..3]
        .iter()
        .map(|model| model.id.as_str())
        .collect();
    assert_eq!(
        ids,
        [
            "anthropic/claude-sonnet-4.5",
            "moonshotai/kimi-k2",
            "openai/gpt-4o-mini"
        ]
    );
}

#[test]
fn maps_limits_prices_and_capabilities() {
    let catalog = catalog();
    let sonnet = catalog.lookup("claude-sonnet-4-5-20250929").unwrap();
    assert_eq!(sonnet.name, "Claude Sonnet 4.5");
    assert_eq!(sonnet.provider, "anthropic");
    assert_eq!(
        (sonnet.context_window, sonnet.max_output),
        (200_000, 64_000)
    );
    assert_eq!(
        sonnet.pricing,
        Some(Pricing {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75
        })
    );
    assert!(sonnet.tools && sonnet.vision && sonnet.reasoning);
    let gpt = catalog.lookup("gpt-4o").unwrap();
    assert_eq!(gpt.provider, "openai");
    let pricing = gpt.pricing.unwrap();
    assert_eq!((pricing.cache_read, pricing.cache_write), (1.25, 2.5));
    let o3 = catalog.lookup("o3-mini").unwrap();
    assert!(!o3.vision, "modalities win over the attachment flag");
    let kimi = catalog.lookup("kimi-k2").unwrap();
    assert_eq!((kimi.context_window, kimi.pricing), (131_072, None));
}

#[test]
fn marks_models_listed_as_deprecated() {
    let catalog = catalog();
    let deprecated = |id: &str| catalog.lookup_in("opencode", id).unwrap().deprecated;
    assert!(deprecated("glm-4.7-free"));
    assert!(!deprecated("big-pickle"));
}

#[test]
fn resolves_openrouter_and_native_ids() {
    let catalog = catalog();
    let provider = |id: &str| catalog.lookup(id).map(|model| model.provider.as_str());
    assert_eq!(provider("anthropic/claude-sonnet-4.5"), Some("openrouter"));
    assert_eq!(provider("openai/gpt-4o"), Some("openai"));
    assert_eq!(provider("claude-sonnet-4-5"), Some("opencode"));
    assert_eq!(provider("big-pickle"), Some("opencode"));
    assert_eq!(provider("OPENAI/GPT-4O-MINI"), Some("openrouter"));
    assert_eq!(provider("no-such-model"), None);
}

#[test]
fn rejects_payloads_that_are_not_catalogs() {
    for bad in ["", "not json", "[1, 2]"] {
        assert!(matches!(
            ModelCatalog::from_models_dev_json(bad),
            Err(LlmError::Decode(_))
        ));
    }
    let empty = ModelCatalog::from_models_dev_json(r#"{"x": {"name": "no models"}}"#).unwrap();
    assert!(empty.models.is_empty());
}

#[tokio::test]
async fn fetch_returns_the_body_or_a_typed_error() {
    let http = reqwest::Client::new();
    let ok = serve(200, r#"{"openai": {}}"#).await;
    assert_eq!(
        fetch_catalog(&http, &ok).await.unwrap(),
        r#"{"openai": {}}"#
    );
    let down = serve(503, "maintenance").await;
    assert_eq!(
        fetch_catalog(&http, &down).await,
        Err(LlmError::Http {
            status: 503,
            body: "maintenance".into()
        })
    );
}
