use serde::{Deserialize, Serialize};

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
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
    // Small random jitter (50–250ms) to avoid clustering with other callers sharing this token
    let jitter = std::time::Duration::from_millis(50 + (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_millis() as u64 % 200));
    tokio::time::sleep(jitter).await;

    log::info!("API request → {} (after {}ms jitter)", USAGE_URL, jitter.as_millis());

    let resp = client
        .get(USAGE_URL)
        .header("Authorization", format!("Bearer {token}"))
        .header("anthropic-beta", BETA_HEADER)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    let status = resp.status();
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        log::warn!("API rate limited (429) — will use cached data");
        return Err("rate_limited".into());
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
