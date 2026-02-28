// GeminiHydra v15 — Google OAuth PKCE + API Key management
// Two auth methods: (1) Google API key stored encrypted in DB, (2) Google OAuth 2.0 PKCE
// Priority: DB OAuth token → DB API key → GOOGLE_API_KEY env var

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse};
use axum::Json;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::state::AppState;

// ── AES-256-GCM token encryption ────────────────────────────────────────
// Encrypts tokens/keys at rest in DB when OAUTH_ENCRYPTION_KEY or AUTH_SECRET
// is set. Graceful degradation: stores plaintext if neither key is available.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};

/// Derive a 32-byte AES-256 key from a secret string via SHA-256.
fn derive_encryption_key(secret: &str) -> [u8; 32] {
    let hash = Sha256::digest(secret.as_bytes());
    let mut key = [0u8; 32];
    key.copy_from_slice(&hash);
    key
}

/// Get the encryption key from env, if available.
/// Priority: OAUTH_ENCRYPTION_KEY > AUTH_SECRET > None (plaintext).
fn get_encryption_key() -> Option<[u8; 32]> {
    std::env::var("OAUTH_ENCRYPTION_KEY")
        .ok()
        .or_else(|| std::env::var("AUTH_SECRET").ok())
        .filter(|s| !s.is_empty())
        .map(|s| derive_encryption_key(&s))
}

/// Encrypt a plaintext string. Returns hex-encoded "enc:nonce:ciphertext".
/// Returns the original plaintext if no encryption key is available.
pub(crate) fn encrypt_token(plaintext: &str) -> String {
    let Some(key_bytes) = get_encryption_key() else {
        return plaintext.to_string();
    };
    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .expect("AES-256-GCM key is always 32 bytes");
    let nonce_bytes: [u8; 12] = rand::random();
    let nonce = Nonce::from_slice(&nonce_bytes);
    match cipher.encrypt(nonce, plaintext.as_bytes()) {
        Ok(ciphertext) => {
            format!("enc:{}:{}", hex::encode(nonce_bytes), hex::encode(ciphertext))
        }
        Err(e) => {
            tracing::error!("Failed to encrypt token: {}", e);
            plaintext.to_string()
        }
    }
}

/// Decrypt a token string. If it starts with "enc:", parse nonce:ciphertext.
/// Otherwise treat as plaintext (backwards-compatible with unencrypted tokens).
pub(crate) fn decrypt_token(stored: &str) -> Result<String, String> {
    if !stored.starts_with("enc:") {
        return Ok(stored.to_string());
    }
    let Some(key_bytes) = get_encryption_key() else {
        return Err("Encrypted token in DB but no encryption key configured".into());
    };
    let parts: Vec<&str> = stored.splitn(3, ':').collect();
    if parts.len() != 3 {
        return Err("Malformed encrypted token format".into());
    }
    let nonce_bytes = hex::decode(parts[1])
        .map_err(|e| format!("Invalid nonce hex: {}", e))?;
    let ciphertext = hex::decode(parts[2])
        .map_err(|e| format!("Invalid ciphertext hex: {}", e))?;
    if nonce_bytes.len() != 12 {
        return Err(format!("Invalid nonce length: {} (expected 12)", nonce_bytes.len()));
    }
    let cipher = Aes256Gcm::new_from_slice(&key_bytes)
        .expect("AES-256-GCM key is always 32 bytes");
    let nonce = Nonce::from_slice(&nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|e| format!("Decryption failed (wrong key?): {}", e))?;
    String::from_utf8(plaintext).map_err(|e| format!("Decrypted token is not valid UTF-8: {}", e))
}

// ── Google OAuth 2.0 constants ───────────────────────────────────────────

const GOOGLE_AUTHORIZE_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v2/userinfo";
const SCOPE: &str = "https://www.googleapis.com/auth/cloud-platform https://www.googleapis.com/auth/userinfo.email https://www.googleapis.com/auth/userinfo.profile";
const TOKEN_EXPIRY_BUFFER_SECS: i64 = 300;

/// Read Google OAuth client credentials from env vars.
/// Returns None if not configured (OAuth option hidden in UI).
fn google_oauth_credentials() -> Option<(String, String)> {
    let client_id = std::env::var("GOOGLE_OAUTH_CLIENT_ID").ok()?;
    let client_secret = std::env::var("GOOGLE_OAUTH_CLIENT_SECRET").ok()?;
    if client_id.is_empty() || client_secret.is_empty() {
        return None;
    }
    Some((client_id, client_secret))
}

/// Build the redirect URI based on backend port.
fn redirect_uri() -> String {
    let port = std::env::var("PORT").unwrap_or_else(|_| "8081".to_string());
    format!("http://localhost:{}/api/auth/google/redirect", port)
}

// ── DB row ─────────────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct GoogleAuthRow {
    auth_method: String,
    access_token: String,
    refresh_token: String,
    expires_at: i64,
    api_key_encrypted: String,
    user_email: String,
    user_name: String,
}

#[derive(Deserialize)]
struct GoogleTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: i64,
}

#[derive(Deserialize)]
struct GoogleUserInfo {
    email: Option<String>,
    name: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════
//  Handlers
// ═══════════════════════════════════════════════════════════════════════

/// GET /api/auth/status — combined auth status for all methods
pub async fn auth_status(State(state): State<AppState>) -> Json<Value> {
    let oauth_available = google_oauth_credentials().is_some();

    // Check DB first
    if let Some(row) = get_auth_row(&state).await {
        if row.auth_method == "oauth" && !row.access_token.is_empty() {
            let now = chrono::Utc::now().timestamp();
            let expired = now >= row.expires_at - TOKEN_EXPIRY_BUFFER_SECS;
            return Json(json!({
                "authenticated": true,
                "method": "oauth",
                "expired": expired,
                "expires_at": row.expires_at,
                "user_email": row.user_email,
                "user_name": row.user_name,
                "oauth_available": oauth_available,
            }));
        }
        if row.auth_method == "api_key" && !row.api_key_encrypted.is_empty() {
            return Json(json!({
                "authenticated": true,
                "method": "api_key",
                "oauth_available": oauth_available,
            }));
        }
    }

    // Check env var fallback
    let has_env_key = state.runtime.read().await.api_keys.contains_key("google");
    if has_env_key {
        return Json(json!({
            "authenticated": true,
            "method": "env",
            "oauth_available": oauth_available,
        }));
    }

    Json(json!({
        "authenticated": false,
        "oauth_available": oauth_available,
    }))
}

/// POST /api/auth/login — start Google OAuth PKCE flow
pub async fn auth_login(
    State(state): State<AppState>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let (client_id, _) = google_oauth_credentials().ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Google OAuth not configured (GOOGLE_OAUTH_CLIENT_ID / GOOGLE_OAUTH_CLIENT_SECRET env vars missing)" })),
        )
    })?;

    let code_verifier = random_base64url(32);
    let code_challenge = sha256_base64url(&code_verifier);
    let oauth_state = random_base64url(32);

    {
        let mut pkce = state.oauth_pkce.write().await;
        *pkce = Some(crate::state::OAuthPkceState {
            code_verifier,
            state: oauth_state.clone(),
        });
    }

    let mut auth_url = url::Url::parse(GOOGLE_AUTHORIZE_URL)
        .expect("GOOGLE_AUTHORIZE_URL is a valid hardcoded URL");
    auth_url
        .query_pairs_mut()
        .append_pair("client_id", &client_id)
        .append_pair("redirect_uri", &redirect_uri())
        .append_pair("response_type", "code")
        .append_pair("scope", SCOPE)
        .append_pair("code_challenge", &code_challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", &oauth_state)
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent");

    Ok(Json(json!({
        "auth_url": auth_url.to_string(),
        "state": oauth_state,
    })))
}

/// Query params from Google OAuth redirect
#[derive(Deserialize)]
pub struct GoogleRedirectParams {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

/// GET /api/auth/google/redirect — Google redirects here after user consent.
/// Exchanges code for tokens, stores in DB, returns HTML success page.
pub async fn google_redirect(
    State(state): State<AppState>,
    Query(params): Query<GoogleRedirectParams>,
) -> impl IntoResponse {
    // Handle error from Google
    if let Some(error) = params.error {
        return Html(format!(
            r#"<!DOCTYPE html><html><head><title>Auth Error</title></head>
            <body style="font-family:monospace;background:#0a0a0a;color:#ff4444;display:flex;align-items:center;justify-content:center;height:100vh;margin:0">
            <div style="text-align:center"><h2>Authentication Failed</h2><p>{}</p><p>You can close this tab.</p></div>
            </body></html>"#,
            error
        ));
    }

    let (code, oauth_state) = match (params.code, params.state) {
        (Some(c), Some(s)) => (c, s),
        _ => {
            return Html(
                r#"<!DOCTYPE html><html><head><title>Auth Error</title></head>
                <body style="font-family:monospace;background:#0a0a0a;color:#ff4444;display:flex;align-items:center;justify-content:center;height:100vh;margin:0">
                <div style="text-align:center"><h2>Missing Parameters</h2><p>No authorization code received.</p></div>
                </body></html>"#.to_string(),
            );
        }
    };

    // Verify PKCE state
    let code_verifier = {
        let pkce = state.oauth_pkce.read().await;
        match pkce.as_ref() {
            Some(p) if p.state == oauth_state => p.code_verifier.clone(),
            _ => {
                return Html(
                    r#"<!DOCTYPE html><html><head><title>Auth Error</title></head>
                    <body style="font-family:monospace;background:#0a0a0a;color:#ff4444;display:flex;align-items:center;justify-content:center;height:100vh;margin:0">
                    <div style="text-align:center"><h2>Invalid State</h2><p>OAuth state mismatch. Please try again.</p></div>
                    </body></html>"#.to_string(),
                );
            }
        }
    };

    let (client_id, client_secret) = match google_oauth_credentials() {
        Some(creds) => creds,
        None => {
            return Html("OAuth not configured".to_string());
        }
    };

    // Exchange code for tokens (Google requires client_secret even with PKCE)
    let token_resp = state
        .client
        .post(GOOGLE_TOKEN_URL)
        .form(&[
            ("code", code.as_str()),
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("redirect_uri", &redirect_uri()),
            ("grant_type", "authorization_code"),
            ("code_verifier", code_verifier.as_str()),
        ])
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await;

    let resp = match token_resp {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Google token exchange request failed: {}", e);
            return Html(format!(
                r#"<!DOCTYPE html><html><head><title>Auth Error</title></head>
                <body style="font-family:monospace;background:#0a0a0a;color:#ff4444;display:flex;align-items:center;justify-content:center;height:100vh;margin:0">
                <div style="text-align:center"><h2>Token Exchange Failed</h2><p>{}</p></div>
                </body></html>"#,
                e
            ));
        }
    };

    if !resp.status().is_success() {
        let err = resp.text().await.unwrap_or_default();
        tracing::error!("Google rejected token exchange: {}", err);
        return Html(format!(
            r#"<!DOCTYPE html><html><head><title>Auth Error</title></head>
            <body style="font-family:monospace;background:#0a0a0a;color:#ff4444;display:flex;align-items:center;justify-content:center;height:100vh;margin:0">
            <div style="text-align:center"><h2>Token Exchange Rejected</h2><p>{}</p></div>
            </body></html>"#,
            html_escape(&err)
        ));
    }

    let tokens: GoogleTokenResponse = match resp.json().await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("Invalid token response from Google: {}", e);
            return Html("Invalid token response".to_string());
        }
    };

    // Fetch user info
    let (user_email, user_name) = fetch_user_info(&state.client, &tokens.access_token).await;

    let now = chrono::Utc::now().timestamp();
    let expires_at = now + tokens.expires_in;

    // Encrypt and store
    let encrypted_access = encrypt_token(&tokens.access_token);
    let encrypted_refresh = encrypt_token(tokens.refresh_token.as_deref().unwrap_or(""));

    if let Err(e) = sqlx::query(
        "INSERT INTO gh_google_auth (id, auth_method, access_token, refresh_token, expires_at, user_email, user_name, updated_at) \
         VALUES (1, 'oauth', $1, $2, $3, $4, $5, NOW()) \
         ON CONFLICT (id) DO UPDATE SET \
         auth_method = 'oauth', access_token = $1, refresh_token = $2, expires_at = $3, \
         api_key_encrypted = '', user_email = $4, user_name = $5, updated_at = NOW()",
    )
    .bind(&encrypted_access)
    .bind(&encrypted_refresh)
    .bind(expires_at)
    .bind(&user_email)
    .bind(&user_name)
    .execute(&state.db)
    .await
    {
        tracing::error!("Failed to store Google OAuth tokens: {}", e);
        return Html("Failed to store tokens".to_string());
    }

    // Clear PKCE state
    *state.oauth_pkce.write().await = None;

    tracing::info!("Google OAuth login successful for {}", user_email);

    Html(format!(
        r#"<!DOCTYPE html><html><head><title>Authenticated</title></head>
        <body style="font-family:monospace;background:#0a0a0a;color:#00ff41;display:flex;align-items:center;justify-content:center;height:100vh;margin:0">
        <div style="text-align:center">
        <h2 style="font-size:2rem">&#10003; Connected</h2>
        <p>Signed in as <strong>{}</strong></p>
        <p style="color:#888">You can close this tab and return to GeminiHydra.</p>
        </div></body></html>"#,
        html_escape(&user_email)
    ))
}

#[derive(Deserialize)]
pub struct SaveApiKeyRequest {
    pub api_key: String,
}

/// POST /api/auth/apikey — validate and store a Google API key
pub async fn save_api_key(
    State(state): State<AppState>,
    Json(req): Json<SaveApiKeyRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let key = req.api_key.trim();
    if key.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "API key cannot be empty" })),
        ));
    }

    // Validate by listing models
    let resp = state
        .client
        .get("https://generativelanguage.googleapis.com/v1beta/models")
        .header("x-goog-api-key", key)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": format!("Validation request failed: {}", e) })),
            )
        })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let err = resp.text().await.unwrap_or_default();
        tracing::warn!("API key validation failed: {} — {}", status, err);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Invalid API key", "valid": false })),
        ));
    }

    // Encrypt and store
    let encrypted = encrypt_token(key);
    sqlx::query(
        "INSERT INTO gh_google_auth (id, auth_method, api_key_encrypted, access_token, refresh_token, updated_at) \
         VALUES (1, 'api_key', $1, '', '', NOW()) \
         ON CONFLICT (id) DO UPDATE SET \
         auth_method = 'api_key', api_key_encrypted = $1, access_token = '', refresh_token = '', \
         expires_at = 0, user_email = '', user_name = '', updated_at = NOW()",
    )
    .bind(&encrypted)
    .execute(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to store API key: {}", e) })),
        )
    })?;

    // Inject into runtime for immediate use
    {
        let mut rt = state.runtime.write().await;
        rt.api_keys.insert("google".to_string(), key.to_string());
    }

    tracing::info!("Google API key saved and validated");

    Ok(Json(json!({
        "status": "ok",
        "authenticated": true,
        "valid": true,
    })))
}

/// DELETE /api/auth/apikey — remove stored API key
pub async fn delete_api_key(
    State(state): State<AppState>,
) -> Json<Value> {
    sqlx::query("DELETE FROM gh_google_auth WHERE id = 1")
        .execute(&state.db)
        .await
        .ok();

    // Remove from runtime (env var will still be checked as fallback)
    // Only remove if no env var exists
    if std::env::var("GOOGLE_API_KEY").is_err() && std::env::var("GEMINI_API_KEY").is_err() {
        let mut rt = state.runtime.write().await;
        rt.api_keys.remove("google");
    }

    tracing::info!("Google auth credentials deleted");
    Json(json!({ "status": "ok" }))
}

/// POST /api/auth/logout — alias for delete (clears OAuth tokens or API key)
pub async fn auth_logout(State(state): State<AppState>) -> Json<Value> {
    delete_api_key(State(state)).await
}

// ═══════════════════════════════════════════════════════════════════════
//  Credential resolution (used by handlers)
// ═══════════════════════════════════════════════════════════════════════

/// Get the effective Google API credential for Gemini API calls.
/// Priority: 1) OAuth access token → 2) DB API key → 3) env var
/// Returns `(credential, is_oauth_token)`.
/// When `is_oauth_token=true`, use `Authorization: Bearer` header.
/// When `is_oauth_token=false`, use `x-goog-api-key` header.
pub async fn get_google_credential(state: &AppState) -> Option<(String, bool)> {
    // 1. Check DB
    if let Some(row) = get_auth_row(state).await {
        // OAuth token
        if row.auth_method == "oauth" && !row.access_token.is_empty() {
            let now = chrono::Utc::now().timestamp();

            // Decrypt access token
            let access_token = match decrypt_token(&row.access_token) {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("Failed to decrypt OAuth access token: {}", e);
                    // Fall through to other methods
                    return try_db_api_key(state, &row).await.or_else(|| try_env_key(state));
                }
            };

            // Token still valid
            if now < row.expires_at - TOKEN_EXPIRY_BUFFER_SECS {
                return Some((access_token, true));
            }

            // Try refresh
            if let Some(refreshed) = refresh_google_token(state, &row).await {
                return Some((refreshed, true));
            }

            // Refresh failed — fall through to API key
            tracing::warn!("OAuth token expired and refresh failed, trying API key fallback");
        }

        // DB API key
        if let Some((key, is_oauth)) = try_db_api_key(state, &row).await {
            return Some((key, is_oauth));
        }
    }

    // 3. Env var fallback
    try_env_key(state)
}

/// Apply Google credential to a reqwest RequestBuilder.
pub fn apply_google_auth(
    builder: reqwest::RequestBuilder,
    credential: &str,
    is_oauth: bool,
) -> reqwest::RequestBuilder {
    if is_oauth {
        builder.header("Authorization", format!("Bearer {}", credential))
    } else {
        builder.header("x-goog-api-key", credential)
    }
}

// ── Internal helpers ─────────────────────────────────────────────────────

async fn try_db_api_key(state: &AppState, row: &GoogleAuthRow) -> Option<(String, bool)> {
    if !row.api_key_encrypted.is_empty() {
        match decrypt_token(&row.api_key_encrypted) {
            Ok(key) if !key.is_empty() => {
                // Also inject into runtime cache
                let mut rt = state.runtime.write().await;
                rt.api_keys.insert("google".to_string(), key.clone());
                return Some((key, false));
            }
            Ok(_) => {}
            Err(e) => tracing::error!("Failed to decrypt stored API key: {}", e),
        }
    }
    None
}

fn try_env_key(_state: &AppState) -> Option<(String, bool)> {
    std::env::var("GOOGLE_API_KEY")
        .ok()
        .or_else(|| std::env::var("GEMINI_API_KEY").ok())
        .filter(|k| !k.is_empty())
        .map(|k| (k, false))
}

async fn refresh_google_token(state: &AppState, row: &GoogleAuthRow) -> Option<String> {
    let refresh_token = match decrypt_token(&row.refresh_token) {
        Ok(t) if !t.is_empty() => t,
        _ => return None,
    };

    let (client_id, client_secret) = google_oauth_credentials()?;

    tracing::info!("Refreshing expired Google OAuth token...");

    let resp = state
        .client
        .post(GOOGLE_TOKEN_URL)
        .form(&[
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("refresh_token", refresh_token.as_str()),
            ("grant_type", "refresh_token"),
        ])
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        tracing::error!("Google OAuth token refresh failed: {}", resp.status());
        return None;
    }

    let token_resp: GoogleTokenResponse = resp.json().await.ok()?;
    let now = chrono::Utc::now().timestamp();
    let expires_at = now + token_resp.expires_in;
    let new_refresh = token_resp.refresh_token.unwrap_or(refresh_token);

    let encrypted_access = encrypt_token(&token_resp.access_token);
    let encrypted_refresh = encrypt_token(&new_refresh);

    sqlx::query(
        "UPDATE gh_google_auth SET access_token = $1, refresh_token = $2, \
         expires_at = $3, updated_at = NOW() WHERE id = 1",
    )
    .bind(&encrypted_access)
    .bind(&encrypted_refresh)
    .bind(expires_at)
    .execute(&state.db)
    .await
    .ok()?;

    tracing::info!("Google OAuth token refreshed successfully");
    Some(token_resp.access_token)
}

async fn fetch_user_info(client: &reqwest::Client, access_token: &str) -> (String, String) {
    match client
        .get(GOOGLE_USERINFO_URL)
        .header("Authorization", format!("Bearer {}", access_token))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            let info: GoogleUserInfo = resp.json().await.unwrap_or(GoogleUserInfo {
                email: None,
                name: None,
            });
            (
                info.email.unwrap_or_default(),
                info.name.unwrap_or_default(),
            )
        }
        _ => (String::new(), String::new()),
    }
}

async fn get_auth_row(state: &AppState) -> Option<GoogleAuthRow> {
    sqlx::query_as::<_, GoogleAuthRow>(
        "SELECT auth_method, access_token, refresh_token, expires_at, api_key_encrypted, user_email, user_name \
         FROM gh_google_auth WHERE id = 1",
    )
    .fetch_optional(&state.db)
    .await
    .ok()?
}

fn random_base64url(len: usize) -> String {
    let buf: Vec<u8> = (0..len).map(|_| rand::random::<u8>()).collect();
    URL_SAFE_NO_PAD.encode(&buf)
}

fn sha256_base64url(input: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(input.as_bytes()))
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
