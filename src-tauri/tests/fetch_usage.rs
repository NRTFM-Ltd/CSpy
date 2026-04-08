use cspy_lib::usage::{build_client, fetch_usage_from};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn success_returns_normalised_data() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/oauth/usage"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "five_hour": { "utilization": 42.0, "resets_at": "2026-04-08T15:00:00Z" },
            "seven_day": { "utilization": 8.5, "resets_at": "2026-04-12T00:00:00Z" }
        })))
        .mount(&mock_server)
        .await;

    let client = build_client().unwrap();
    let url = format!("{}/api/oauth/usage", mock_server.uri());
    let data = fetch_usage_from(&client, "test-token", &url).await.unwrap();

    let five = data.five_hour.unwrap();
    assert!((five.utilisation - 0.42).abs() < f64::EPSILON);
    assert_eq!(five.resets_at, Some("2026-04-08T15:00:00Z".to_string()));

    let seven = data.seven_day.unwrap();
    assert!((seven.utilisation - 0.085).abs() < f64::EPSILON);
}

#[tokio::test]
async fn unauthorized_returns_token_expired() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/oauth/usage"))
        .respond_with(ResponseTemplate::new(401).set_body_string("invalid token"))
        .mount(&mock_server)
        .await;

    let client = build_client().unwrap();
    let url = format!("{}/api/oauth/usage", mock_server.uri());
    let err = fetch_usage_from(&client, "bad-token", &url).await.unwrap_err();

    assert_eq!(err, "token_expired");
}

#[tokio::test]
async fn rate_limited_with_retry_after() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/oauth/usage"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("Retry-After", "60")
        )
        .mount(&mock_server)
        .await;

    let client = build_client().unwrap();
    let url = format!("{}/api/oauth/usage", mock_server.uri());
    let err = fetch_usage_from(&client, "test-token", &url).await.unwrap_err();

    assert_eq!(err, "rate_limited:60");
}

#[tokio::test]
async fn rate_limited_without_retry_after() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/oauth/usage"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&mock_server)
        .await;

    let client = build_client().unwrap();
    let url = format!("{}/api/oauth/usage", mock_server.uri());
    let err = fetch_usage_from(&client, "test-token", &url).await.unwrap_err();

    assert_eq!(err, "rate_limited:0");
}

#[tokio::test]
async fn server_error_returns_status_in_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/oauth/usage"))
        .respond_with(ResponseTemplate::new(500).set_body_string("internal error"))
        .mount(&mock_server)
        .await;

    let client = build_client().unwrap();
    let url = format!("{}/api/oauth/usage", mock_server.uri());
    let err = fetch_usage_from(&client, "test-token", &url).await.unwrap_err();

    assert!(err.contains("500"), "error should contain status code: {err}");
    assert!(err.contains("internal error"), "error should contain body: {err}");
}

#[tokio::test]
async fn malformed_json_returns_parse_error() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/oauth/usage"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("not json at all")
                .insert_header("Content-Type", "application/json")
        )
        .mount(&mock_server)
        .await;

    let client = build_client().unwrap();
    let url = format!("{}/api/oauth/usage", mock_server.uri());
    let err = fetch_usage_from(&client, "test-token", &url).await.unwrap_err();

    assert!(err.contains("parse"), "error should mention parsing: {err}");
}
