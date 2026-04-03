use serde::Deserialize;
use std::process::Command;

/// Credential blob stored by Claude Code in macOS Keychain.
#[derive(Deserialize)]
pub struct ClaudeCredentials {
    #[serde(rename = "claudeAiOauth")]
    pub claude_ai_oauth: Option<OAuthCreds>,
}

#[derive(Deserialize)]
pub struct OAuthCreds {
    #[serde(rename = "accessToken")]
    pub access_token: String,
    #[serde(rename = "subscriptionType")]
    pub subscription_type: Option<String>,
}

// Redacted Debug — never print tokens to logs
impl std::fmt::Debug for OAuthCreds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuthCreds")
            .field("access_token", &"[REDACTED]")
            .field("subscription_type", &self.subscription_type)
            .finish()
    }
}

impl std::fmt::Debug for ClaudeCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClaudeCredentials")
            .field("claude_ai_oauth", &self.claude_ai_oauth)
            .finish()
    }
}

/// Read the Claude OAuth token.
///
/// Sources checked in order:
/// 1. Token file at `~/.config/cspy/token` (for users without Claude Code)
/// 2. macOS Keychain — "Claude Code-credentials" (automatic if Claude Code is installed)
pub fn get_oauth_token() -> Result<String, String> {
    // Source 1: token file
    if let Some(token) = read_token_file() {
        log::info!("Token loaded from ~/.config/cspy/token");
        return Ok(token);
    }

    // Source 2: macOS Keychain (Claude Code stores credentials here)
    read_keychain_token()
}

/// Read token from ~/.config/cspy/token if it exists.
fn read_token_file() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let path = std::path::Path::new(&home).join(".config/cspy/token");
    let contents = std::fs::read_to_string(&path).ok()?;
    let token = contents.trim().to_string();
    if token.is_empty() {
        return None;
    }
    Some(token)
}

/// Read token from macOS Keychain via the `security` CLI.
fn read_keychain_token() -> Result<String, String> {
    let output = Command::new("security")
        .args(["find-generic-password", "-s", "Claude Code-credentials", "-w"])
        .output()
        .map_err(|e| format!("Failed to run `security`: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "No token found. Either:\n  \
             • Install Claude Code and log in (automatic), or\n  \
             • Save your token to ~/.config/cspy/token\n\n\
             Keychain error: {stderr}"
        ));
    }

    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();

    let creds: ClaudeCredentials =
        serde_json::from_str(&raw).map_err(|e| format!("Failed to parse credentials: {e}"))?;

    let oauth = creds
        .claude_ai_oauth
        .ok_or("No claudeAiOauth field in credentials")?;

    if oauth.access_token.is_empty() {
        return Err("OAuth access token is empty".into());
    }

    log::info!(
        "Keychain: got token for subscription type {:?}",
        oauth.subscription_type.as_deref().unwrap_or("unknown")
    );

    Ok(oauth.access_token)
}
