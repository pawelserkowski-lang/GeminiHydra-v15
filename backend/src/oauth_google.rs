// Jaskier Shared Pattern — Google OAuth PKCE + API Key management
// Ported from ClaudeHydra-v4. Adapted for GeminiHydra-v15.
// Table: gh_google_auth (singleton). Default port: 8081.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::oauth::{decrypt_token, encrypt_token, html_escape, random_base64url, sha256_base64url};
use crate::state::AppState;

// ── Google OAuth 2.0 constants ───────────────────────────────────────────

const GOOGLE_AUTHORIZE_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v2/userinfo";
const SCOPE: &str = "https://www.googleapis.com/auth/cloud-platform https://www.googleapis.com/auth/generative-language.retriever https://www.googleapis.com/auth/generative-language.tuning https://www.googleapis.com/auth/userinfo.email https://www.googleapis.com/auth/userinfo.profile";
const TOKEN_EXPIRY_BUFFER_SECS: i64 = 300;

/// Read Google OAuth client credentials from env vars.
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

pub async fn google_auth_status(State(state): State<AppState>) -> Json<Value> {
    let oauth_available = google_oauth_credentials().is_some();

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

    let has_env_key = std::env::var("GOOGLE_API_KEY")
        .ok()
        .or_else(|| std::env::var("GEMINI_API_KEY").ok())
        .filter(|k| !k.is_empty())
        .is_some();

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

pub async fn google_auth_login(
    State(state): State<AppState>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let (client_id, _) = google_oauth_credentials().ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Google OAuth not configured" })),
        )
    })?;

    let code_verifier = random_base64url(32);
    let code_challenge = sha256_base64url(&code_verifier);
    let oauth_state = random_base64url(32);

    {
        let mut pkce = state.google_oauth_pkce.write().await;
        *pkce = Some(crate::state::OAuthPkceState {
            code_verifier,
            state: oauth_state.clone(),
        });
    }

    let mut auth_url = url::Url::parse(GOOGLE_AUTHORIZE_URL).unwrap();
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

#[derive(Deserialize)]
pub struct GoogleRedirectParams {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

pub async fn google_redirect(
    State(state): State<AppState>,
    Query(params): Query<GoogleRedirectParams>,
) -> impl IntoResponse {
    if let Some(error) = params.error {
        return Html(format!("Authentication Failed: {}", html_escape(&error)));
    }

    let (code, oauth_state) = match (params.code, params.state) {
        (Some(c), Some(s)) => (c, s),
        _ => return Html("Missing Parameters".to_string()),
    };

    let code_verifier = {
        let pkce = state.google_oauth_pkce.read().await;
        match pkce.as_ref() {
            Some(p) if p.state == oauth_state => p.code_verifier.clone(),
            _ => return Html("Invalid State".to_string()),
        }
    };

    let (client_id, client_secret) = match google_oauth_credentials() {
        Some(creds) => creds,
        None => return Html("OAuth not configured".to_string()),
    };

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
        Err(e) => return Html(format!("Token Exchange Failed: {}", html_escape(&e.to_string()))),
    };

    if !resp.status().is_success() {
        let err = resp.text().await.unwrap_or_default();
        return Html(format!("Token Exchange Rejected: {}", html_escape(&err)));
    }

    let tokens: GoogleTokenResponse = match resp.json().await {
        Ok(t) => t,
        Err(_) => return Html("Invalid token response".to_string()),
    };

    let (user_email, user_name) = fetch_user_info(&state.client, &tokens.access_token).await;
    let expires_at = chrono::Utc::now().timestamp() + tokens.expires_in;

    let encrypted_access = encrypt_token(&tokens.access_token);
    let encrypted_refresh = encrypt_token(tokens.refresh_token.as_deref().unwrap_or(""));

    // Assuming we created gh_google_auth table matching ch_google_auth schema
    if let Err(e) = sqlx::query(
        "INSERT INTO gh_google_auth (id, auth_method, access_token, refresh_token, expires_at, user_email, user_name, updated_at) \
         VALUES (1, 'oauth', $1, $2, $3, $4, $5, NOW()) \
         ON CONFLICT (id) DO UPDATE SET \
         auth_method = 'oauth', access_token = $1, refresh_token = $2, expires_at = $3, \
         api_key_encrypted = '', user_email = $4, user_name = $5, updated_at = NOW()"
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

    *state.google_oauth_pkce.write().await = None;

    Html(format!(
        r#"<!DOCTYPE html><html><head><title>Authenticated</title></head>
        <body style="font-family:monospace;background:#0a0a0a;color:#00ff41;display:flex;align-items:center;justify-content:center;height:100vh;margin:0">
        <div style="text-align:center"><h2>&#10003; Connected</h2><p>Signed in as <strong>{}</strong></p></div></body></html>"#,
        html_escape(&user_email)
    ))
}

#[derive(Deserialize)]
pub struct SaveApiKeyRequest {
    pub api_key: String,
}

pub async fn google_save_api_key(
    State(state): State<AppState>,
    Json(req): Json<SaveApiKeyRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let key = req.api_key.trim();
    if key.is_empty() {
        return Err((StatusCode::BAD_REQUEST, Json(json!({ "error": "API key cannot be empty" }))));
    }

    let encrypted = encrypt_token(key);
    sqlx::query(
        "INSERT INTO gh_google_auth (id, auth_method, api_key_encrypted, access_token, refresh_token, updated_at) \
         VALUES (1, 'api_key', $1, '', '', NOW()) \
         ON CONFLICT (id) DO UPDATE SET \
         auth_method = 'api_key', api_key_encrypted = $1, access_token = '', refresh_token = '', \
         expires_at = 0, user_email = '', user_name = '', updated_at = NOW()"
    )
    .bind(&encrypted)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": format!("Failed: {}", e) }))))?;

    Ok(Json(json!({ "status": "ok", "authenticated": true })))
}

pub async fn google_delete_api_key(State(state): State<AppState>) -> Json<Value> {
    sqlx::query("DELETE FROM gh_google_auth WHERE id = 1").execute(&state.db).await.ok();
    Json(json!({ "status": "ok" }))
}

pub async fn google_auth_logout(State(state): State<AppState>) -> Json<Value> {
    google_delete_api_key(State(state)).await
}

pub async fn get_google_credential(state: &AppState) -> Option<(String, bool)> {
    if let Some(row) = get_auth_row(state).await {
        if row.auth_method == "oauth" && !row.access_token.is_empty() {
            let access_token = decrypt_token(&row.access_token).ok()?;
            if chrono::Utc::now().timestamp() < row.expires_at - TOKEN_EXPIRY_BUFFER_SECS {
                return Some((access_token, true));
            }
            if let Some(refreshed) = refresh_google_token(state, &row).await {
                return Some((refreshed, true));
            }
        }
        if let Some(result) = try_db_api_key(&row) {
            return Some(result);
        }
    }
    try_env_key()
}

fn try_db_api_key(row: &GoogleAuthRow) -> Option<(String, bool)> {
    if !row.api_key_encrypted.is_empty() {
        if let Ok(key) = decrypt_token(&row.api_key_encrypted) {
            if !key.is_empty() {
                return Some((key, false));
            }
        }
    }
    None
}

fn try_env_key() -> Option<(String, bool)> {
    std::env::var("GOOGLE_API_KEY")
        .ok()
        .or_else(|| std::env::var("GEMINI_API_KEY").ok())
        .filter(|k| !k.is_empty())
        .map(|k| (k, false))
}

async fn refresh_google_token(state: &AppState, row: &GoogleAuthRow) -> Option<String> {
    let refresh_token = decrypt_token(&row.refresh_token).ok().filter(|t| !t.is_empty())?;
    let (client_id, client_secret) = google_oauth_credentials()?;

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
        return None;
    }

    let token_resp: GoogleTokenResponse = resp.json().await.ok()?;
    let expires_at = chrono::Utc::now().timestamp() + token_resp.expires_in;
    let new_refresh = token_resp.refresh_token.unwrap_or(refresh_token);

    let encrypted_access = encrypt_token(&token_resp.access_token);
    let encrypted_refresh = encrypt_token(&new_refresh);

    sqlx::query(
        "UPDATE gh_google_auth SET access_token = $1, refresh_token = $2, expires_at = $3, updated_at = NOW() WHERE id = 1",
    )
    .bind(&encrypted_access).bind(&encrypted_refresh).bind(expires_at)
    .execute(&state.db).await.ok()?;

    Some(token_resp.access_token)
}

async fn fetch_user_info(client: &reqwest::Client, access_token: &str) -> (String, String) {
    match client.get(GOOGLE_USERINFO_URL).header("Authorization", format!("Bearer {}", access_token)).timeout(std::time::Duration::from_secs(10)).send().await {
        Ok(resp) if resp.status().is_success() => {
            let info: GoogleUserInfo = resp.json().await.unwrap_or(GoogleUserInfo { email: None, name: None });
            (info.email.unwrap_or_default(), info.name.unwrap_or_default())
        }
        _ => (String::new(), String::new()),
    }
}

async fn get_auth_row(state: &AppState) -> Option<GoogleAuthRow> {
    sqlx::query_as::<_, GoogleAuthRow>(
        "SELECT auth_method, access_token, refresh_token, expires_at, api_key_encrypted, user_email, user_name FROM gh_google_auth WHERE id = 1",
    ).fetch_optional(&state.db).await.ok()?
}
