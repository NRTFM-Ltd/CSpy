use serde::{Deserialize, Serialize};

pub const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const USER_AGENT: &str = "cspy/0.1.0";
const BETA_HEADER: &str = "oauth-2025-04-20";

/// Raw API response from the oauth/usage endpoint.
#[derive(Deserialize)]
struct ApiResponse {
    five_hour: Option<ApiBucket>,
    seven_day: Option<ApiBucket>,
}

#[derive(Deserialize)]
struct ApiBucket {
    utilization: f64,
    resets_at: Option<String>,
}

/// Normalised usage data sent to the Svelte frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageData {
    pub five_hour: Option<UsageBucket>,
    pub seven_day: Option<UsageBucket>,
    pub fetched_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageBucket {
    pub utilisation: f64,
    pub resets_at: Option<String>,
}

/// Build a shared HTTP client with sensible defaults.
pub fn build_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))
}

/// Fetch current usage from the Anthropic OAuth endpoint.
pub async fn fetch_usage(client: &reqwest::Client, token: &str) -> Result<UsageData, String> {
    fetch_usage_from(client, token, USAGE_URL).await
}

/// Fetch usage from a given URL. Used by integration tests with a mock server.
pub async fn fetch_usage_from(client: &reqwest::Client, token: &str, url: &str) -> Result<UsageData, String> {
    // Small random jitter (50–250ms) to avoid clustering with other callers sharing this token
    let jitter = std::time::Duration::from_millis(50 + (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_millis() as u64 % 200));
    tokio::time::sleep(jitter).await;

    log::info!("API request → {} (after {}ms jitter)", url, jitter.as_millis());

    let resp = client
        .get(url)
        .header("Authorization", format!("Bearer {token}"))
        .header("anthropic-beta", BETA_HEADER)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    let status = resp.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        let body = resp.text().await.unwrap_or_default();
        log::warn!("API returned 401 — token expired or invalid: {body}");
        return Err("token_expired".into());
    }
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        let retry_after = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        log::warn!("API rate limited (429) — Retry-After: {retry_after}s");
        return Err(format!("rate_limited:{retry_after}"));
    }
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("API returned {status}: {body}"));
    }

    let api: ApiResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {e}"))?;

    let now = chrono::Utc::now().to_rfc3339();

    Ok(UsageData {
        five_hour: api.five_hour.map(|b| UsageBucket {
            utilisation: b.utilization / 100.0, // API returns 0-100, normalise to 0.0-1.0
            resets_at: b.resets_at,
        }),
        seven_day: api.seven_day.map(|b| UsageBucket {
            utilisation: b.utilization / 100.0,
            resets_at: b.resets_at,
        }),
        fetched_at: now,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_api_response(json: &str) -> UsageData {
        let api: ApiResponse = serde_json::from_str(json).unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        UsageData {
            five_hour: api.five_hour.map(|b| UsageBucket {
                utilisation: b.utilization / 100.0,
                resets_at: b.resets_at,
            }),
            seven_day: api.seven_day.map(|b| UsageBucket {
                utilisation: b.utilization / 100.0,
                resets_at: b.resets_at,
            }),
            fetched_at: now,
        }
    }

    #[test]
    fn normalises_utilization_50_to_0_5() {
        let data = parse_api_response(r#"{
            "five_hour": { "utilization": 50.0, "resets_at": "2026-04-08T12:00:00Z" },
            "seven_day": null
        }"#);
        let bucket = data.five_hour.unwrap();
        assert!((bucket.utilisation - 0.5).abs() < f64::EPSILON);
        assert_eq!(bucket.resets_at, Some("2026-04-08T12:00:00Z".to_string()));
    }

    #[test]
    fn normalises_utilization_0_to_0() {
        let data = parse_api_response(r#"{
            "five_hour": { "utilization": 0.0, "resets_at": null },
            "seven_day": null
        }"#);
        let bucket = data.five_hour.unwrap();
        assert!((bucket.utilisation - 0.0).abs() < f64::EPSILON);
        assert!(bucket.resets_at.is_none());
    }

    #[test]
    fn normalises_utilization_100_to_1() {
        let data = parse_api_response(r#"{
            "five_hour": { "utilization": 100.0, "resets_at": null },
            "seven_day": null
        }"#);
        let bucket = data.five_hour.unwrap();
        assert!((bucket.utilisation - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn missing_five_hour_is_none() {
        let data = parse_api_response(r#"{
            "five_hour": null,
            "seven_day": { "utilization": 10.0, "resets_at": null }
        }"#);
        assert!(data.five_hour.is_none());
        assert!(data.seven_day.is_some());
    }

    #[test]
    fn both_buckets_present() {
        let data = parse_api_response(r#"{
            "five_hour": { "utilization": 25.0, "resets_at": "2026-04-08T15:00:00Z" },
            "seven_day": { "utilization": 10.0, "resets_at": "2026-04-12T00:00:00Z" }
        }"#);
        assert!((data.five_hour.unwrap().utilisation - 0.25).abs() < f64::EPSILON);
        assert!((data.seven_day.unwrap().utilisation - 0.10).abs() < f64::EPSILON);
    }
}
