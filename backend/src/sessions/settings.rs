//! Settings endpoints: get, update (partial PATCH), and reset to defaults.

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;

use crate::models::{AppSettings, SettingsRow};
use crate::state::AppState;

use super::PartialSettings;

// ============================================================================
// Settings handlers
// ============================================================================

/// GET /api/settings
#[utoipa::path(get, path = "/api/settings", tag = "settings",
    responses((status = 200, description = "Current application settings", body = AppSettings))
)]
pub async fn get_settings(
    State(state): State<AppState>,
) -> Result<Json<AppSettings>, StatusCode> {
    let row = sqlx::query_as::<_, SettingsRow>(
        "SELECT temperature, max_tokens, default_model, language, theme, welcome_message, \
         use_docker_sandbox, top_p, response_style, max_iterations, thinking_level, working_directory \
         FROM gh_settings WHERE id = 1",
    )
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(super::row_to_settings(row)))
}

/// PATCH /api/settings — partial update (read-modify-write)
#[utoipa::path(patch, path = "/api/settings", tag = "settings",
    responses((status = 200, description = "Updated settings", body = AppSettings))
)]
pub async fn update_settings(
    State(state): State<AppState>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    Json(patch): Json<PartialSettings>,
) -> Result<Json<AppSettings>, StatusCode> {
    // Limit string field sizes to prevent uncontrolled memory allocation
    if patch
        .welcome_message
        .as_ref()
        .is_some_and(|s| s.len() > 10_000)
        || patch
            .default_model
            .as_ref()
            .is_some_and(|s| s.len() > 200)
        || patch.response_style.as_ref().is_some_and(|s| {
            !["concise", "balanced", "detailed", "technical"].contains(&s.as_str())
        })
        || patch.thinking_level.as_ref().is_some_and(|s| {
            !["none", "minimal", "low", "medium", "high"].contains(&s.as_str())
        })
    {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }

    let current = sqlx::query_as::<_, SettingsRow>(
        "SELECT temperature, max_tokens, default_model, language, theme, welcome_message, \
         use_docker_sandbox, top_p, response_style, max_iterations, thinking_level, working_directory \
         FROM gh_settings WHERE id = 1",
    )
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let temperature = patch.temperature.unwrap_or(current.temperature);
    let max_tokens = patch
        .max_tokens
        .map(|v| v as i32)
        .unwrap_or(current.max_tokens);
    let default_model = patch.default_model.unwrap_or(current.default_model);
    let language = patch.language.unwrap_or(current.language);
    let theme = patch.theme.unwrap_or(current.theme);
    let welcome_message = patch.welcome_message.unwrap_or(current.welcome_message);
    let use_docker_sandbox = patch
        .use_docker_sandbox
        .unwrap_or(current.use_docker_sandbox);
    let top_p = patch.top_p.unwrap_or(current.top_p);
    let response_style = patch.response_style.unwrap_or(current.response_style);
    let max_iterations = patch.max_iterations.unwrap_or(current.max_iterations);
    let thinking_level = patch.thinking_level.unwrap_or(current.thinking_level);
    let working_directory = patch
        .working_directory
        .unwrap_or(current.working_directory);

    // Validate working_directory if non-empty
    if !working_directory.is_empty() && !std::path::Path::new(&working_directory).is_dir() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let row = sqlx::query_as::<_, SettingsRow>(
        "UPDATE gh_settings SET temperature=$1, max_tokens=$2, default_model=$3, \
         language=$4, theme=$5, welcome_message=$6, use_docker_sandbox=$7, \
         top_p=$8, response_style=$9, max_iterations=$10, thinking_level=$11, \
         working_directory=$12, updated_at=NOW() WHERE id=1 \
         RETURNING temperature, max_tokens, default_model, language, theme, welcome_message, \
         use_docker_sandbox, top_p, response_style, max_iterations, thinking_level, working_directory",
    )
    .bind(temperature)
    .bind(max_tokens)
    .bind(&default_model)
    .bind(&language)
    .bind(&theme)
    .bind(&welcome_message)
    .bind(use_docker_sandbox)
    .bind(top_p)
    .bind(&response_style)
    .bind(max_iterations)
    .bind(&thinking_level)
    .bind(&working_directory)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    crate::audit::log_audit(
        &state.db,
        "update_settings",
        serde_json::json!({
            "temperature": temperature,
            "max_tokens": max_tokens,
            "default_model": default_model,
            "language": language,
            "theme": theme,
            "top_p": top_p,
            "response_style": response_style,
            "max_iterations": max_iterations,
            "thinking_level": thinking_level,
            "working_directory": working_directory,
        }),
        Some(&addr.ip().to_string()),
    )
    .await;

    Ok(Json(super::row_to_settings(row)))
}

/// POST /api/settings/reset — restore defaults (picks best model from cache)
#[utoipa::path(post, path = "/api/settings/reset", tag = "settings",
    responses((status = 200, description = "Settings reset to defaults", body = AppSettings))
)]
pub async fn reset_settings(
    State(state): State<AppState>,
) -> Result<Json<AppSettings>, StatusCode> {
    let best_model = crate::model_registry::get_model_id(&state, "chat").await;

    let row = sqlx::query_as::<_, SettingsRow>(
        "UPDATE gh_settings SET temperature=1.0, max_tokens=65536, \
         default_model=$1, language='en', theme='dark', \
         welcome_message='', use_docker_sandbox=FALSE, \
         top_p=0.95, response_style='balanced', max_iterations=20, \
         thinking_level='medium', working_directory='', updated_at=NOW() WHERE id=1 \
         RETURNING temperature, max_tokens, default_model, language, theme, welcome_message, \
         use_docker_sandbox, top_p, response_style, max_iterations, thinking_level, working_directory",
    )
    .bind(&best_model)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(super::row_to_settings(row)))
}
