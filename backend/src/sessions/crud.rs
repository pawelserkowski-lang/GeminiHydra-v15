//! Session CRUD handlers: create, get, list, update, delete, rename,
//! working directory, unlock agent, generate title, and message rating.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde_json::{json, Value};

use crate::models::{
    CreateSessionRequest, RatingRequest, RatingResponse, Session, SessionRow, SessionSummary,
    SessionSummaryRow, UnlockAgentResponse, UpdateSessionRequest, UpdateWorkingDirectoryRequest,
};
use crate::state::AppState;

use super::{PaginationParams, MAX_TITLE_LENGTH};

// ============================================================================
// Session CRUD handlers
// ============================================================================

/// GET /api/sessions?limit=100&offset=0
/// Also supports cursor-based pagination: ?after=<session_id>&limit=20
#[utoipa::path(get, path = "/api/sessions", tag = "sessions",
    params(
        ("limit" = Option<i64>, Query, description = "Max sessions to return (default 100, max 500)"),
        ("offset" = Option<i64>, Query, description = "Number of sessions to skip (default 0)"),
        ("after" = Option<String>, Query, description = "Cursor: return sessions after this session ID (by updated_at)"),
    ),
    responses((status = 200, description = "List of session summaries", body = Vec<SessionSummary>))
)]
pub async fn list_sessions(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Value>, StatusCode> {
    let limit = params.limit.unwrap_or(100).clamp(1, 500);

    // Cursor-based pagination: when `after` is provided, use it instead of offset
    if let Some(ref after_id) = params.after {
        let cursor_id = uuid::Uuid::parse_str(after_id).map_err(|_| StatusCode::BAD_REQUEST)?;

        let rows = sqlx::query_as::<_, SessionSummaryRow>(
            "SELECT s.id, s.title, s.created_at, s.working_directory, s.agent_id, \
             (SELECT COUNT(*) FROM gh_chat_messages WHERE session_id = s.id) as message_count \
             FROM gh_sessions s \
             WHERE s.updated_at < (SELECT updated_at FROM gh_sessions WHERE id = $1) \
             ORDER BY s.updated_at DESC \
             LIMIT $2",
        )
        .bind(cursor_id)
        .bind(limit)
        .fetch_all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let summaries: Vec<SessionSummary> = rows
            .iter()
            .map(|r| SessionSummary {
                id: r.id.to_string(),
                title: r.title.clone(),
                created_at: r.created_at.to_rfc3339(),
                message_count: r.message_count as usize,
                working_directory: r.working_directory.clone(),
                agent_id: r.agent_id.clone(),
            })
            .collect();

        let has_more = summaries.len() as i64 == limit;
        let next_cursor = summaries.last().map(|s| s.id.clone());

        return Ok(Json(
            serde_json::to_value(serde_json::json!({
                "sessions": summaries,
                "has_more": has_more,
                "next_cursor": next_cursor,
            }))
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        ));
    }

    // Offset-based pagination (backwards compatible)
    let offset = params.offset.unwrap_or(0).max(0);

    let rows = sqlx::query_as::<_, SessionSummaryRow>(
        "SELECT s.id, s.title, s.created_at, s.working_directory, s.agent_id, \
         (SELECT COUNT(*) FROM gh_chat_messages WHERE session_id = s.id) as message_count \
         FROM gh_sessions s ORDER BY s.updated_at DESC \
         LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let summaries: Vec<SessionSummary> = rows
        .into_iter()
        .map(|r| SessionSummary {
            id: r.id.to_string(),
            title: r.title,
            created_at: r.created_at.to_rfc3339(),
            message_count: r.message_count as usize,
            working_directory: r.working_directory,
            agent_id: r.agent_id,
        })
        .collect();

    Ok(Json(
        serde_json::to_value(summaries).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    ))
}

/// POST /api/sessions
#[utoipa::path(post, path = "/api/sessions", tag = "sessions",
    request_body = CreateSessionRequest,
    responses((status = 201, description = "Session created", body = Session))
)]
pub async fn create_session(
    State(state): State<AppState>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<(StatusCode, Json<Value>), StatusCode> {
    if req.title.len() > MAX_TITLE_LENGTH {
        tracing::warn!(
            "create_session: title exceeds {} chars (got {})",
            MAX_TITLE_LENGTH,
            req.title.len()
        );
        return Err(StatusCode::BAD_REQUEST);
    }

    let row = sqlx::query_as::<_, SessionRow>(
        "INSERT INTO gh_sessions (title) VALUES ($1) \
         RETURNING id, title, created_at, updated_at, working_directory",
    )
    .bind(&req.title)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let session = Session {
        id: row.id.to_string(),
        title: row.title,
        created_at: row.created_at.to_rfc3339(),
        messages: Vec::new(),
        working_directory: row.working_directory,
    };

    Ok((
        StatusCode::CREATED,
        Json(serde_json::to_value(session).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?),
    ))
}

/// GET /api/sessions/:id?limit=200&offset=0
#[utoipa::path(get, path = "/api/sessions/{id}", tag = "sessions",
    params(
        ("id" = String, Path, description = "Session UUID"),
        ("limit" = Option<i64>, Query, description = "Max messages to return (default 200, max 500)"),
        ("offset" = Option<i64>, Query, description = "Number of messages to skip (default 0)"),
    ),
    responses(
        (status = 200, description = "Session with messages", body = Session),
        (status = 404, description = "Session not found")
    )
)]
pub async fn get_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Value>, StatusCode> {
    let session_id: uuid::Uuid = id.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
    let msg_limit = params.limit.unwrap_or(200).clamp(1, 500);
    let msg_offset = params.offset.unwrap_or(0).max(0);

    let session_row = sqlx::query_as::<_, SessionRow>(
        "SELECT id, title, created_at, updated_at, working_directory FROM gh_sessions WHERE id = $1",
    )
    .bind(session_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    // Total message count for pagination metadata
    let total_messages: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM gh_chat_messages WHERE session_id = $1",
    )
    .bind(session_id)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Fetch the most recent N messages (subquery DESC, then re-sort ASC)
    let message_rows = sqlx::query_as::<_, crate::models::ChatMessageRow>(
        "SELECT * FROM (\
            SELECT id, role, content, model, agent, created_at \
            FROM gh_chat_messages WHERE session_id = $1 \
            ORDER BY created_at DESC LIMIT $2 OFFSET $3\
        ) sub ORDER BY created_at ASC",
    )
    .bind(session_id)
    .bind(msg_limit)
    .bind(msg_offset)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let messages: Vec<crate::models::ChatMessage> =
        message_rows.into_iter().map(super::row_to_message).collect();

    let session = Session {
        id: session_row.id.to_string(),
        title: session_row.title,
        created_at: session_row.created_at.to_rfc3339(),
        messages,
        working_directory: session_row.working_directory,
    };

    let mut result =
        serde_json::to_value(session).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    // Extend session JSON with pagination metadata
    if let Some(obj) = result.as_object_mut() {
        obj.insert("total_messages".to_string(), json!(total_messages));
        obj.insert("limit".to_string(), json!(msg_limit));
        obj.insert("offset".to_string(), json!(msg_offset));
    }

    Ok(Json(result))
}

/// PATCH /api/sessions/:id
#[utoipa::path(patch, path = "/api/sessions/{id}", tag = "sessions",
    params(("id" = String, Path, description = "Session UUID")),
    request_body = UpdateSessionRequest,
    responses(
        (status = 200, description = "Session updated", body = SessionSummary),
        (status = 404, description = "Session not found")
    )
)]
pub async fn update_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateSessionRequest>,
) -> Result<Json<Value>, StatusCode> {
    if req.title.len() > MAX_TITLE_LENGTH {
        tracing::warn!(
            "update_session: title exceeds {} chars (got {})",
            MAX_TITLE_LENGTH,
            req.title.len()
        );
        return Err(StatusCode::BAD_REQUEST);
    }

    let session_id: uuid::Uuid = id.parse().map_err(|_| StatusCode::BAD_REQUEST)?;

    let row = sqlx::query_as::<_, SessionRow>(
        "UPDATE gh_sessions SET title = $1, updated_at = NOW() WHERE id = $2 \
         RETURNING id, title, created_at, updated_at, working_directory",
    )
    .bind(&req.title)
    .bind(session_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let summary = SessionSummary {
        id: row.id.to_string(),
        title: row.title,
        created_at: row.created_at.to_rfc3339(),
        message_count: 0,
        working_directory: row.working_directory,
        agent_id: None,
    };

    Ok(Json(
        serde_json::to_value(summary).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    ))
}

/// DELETE /api/sessions/:id
#[utoipa::path(delete, path = "/api/sessions/{id}", tag = "sessions",
    params(("id" = String, Path, description = "Session UUID")),
    responses(
        (status = 200, description = "Session deleted", body = Value),
        (status = 404, description = "Session not found")
    )
)]
pub async fn delete_session(
    State(state): State<AppState>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let session_id: uuid::Uuid = id.parse().map_err(|_| StatusCode::BAD_REQUEST)?;

    let result = sqlx::query("DELETE FROM gh_sessions WHERE id = $1")
        .bind(session_id)
        .execute(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if result.rows_affected() > 0 {
        crate::audit::log_audit(
            &state.db,
            "delete_session",
            serde_json::json!({ "session_id": id }),
            Some(&addr.ip().to_string()),
        )
        .await;
    }

    // Idempotent: return success even if session was already gone
    Ok(Json(json!({ "status": "deleted", "id": id })))
}

/// PATCH /api/sessions/:id/working-directory
///
/// Update the per-session working directory. Empty string = inherit from global settings.
#[utoipa::path(patch, path = "/api/sessions/{id}/working-directory", tag = "sessions",
    params(("id" = String, Path, description = "Session UUID")),
    request_body = UpdateWorkingDirectoryRequest,
    responses(
        (status = 200, description = "Working directory updated", body = Value),
        (status = 400, description = "Invalid path or session ID"),
        (status = 404, description = "Session not found")
    )
)]
pub async fn update_session_working_directory(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateWorkingDirectoryRequest>,
) -> Result<Json<Value>, StatusCode> {
    let session_id: uuid::Uuid = id.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
    let wd = req.working_directory.trim().to_string();

    if !wd.is_empty() && !std::path::Path::new(&wd).is_dir() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let result = sqlx::query(
        "UPDATE gh_sessions SET working_directory = $1, updated_at = NOW() WHERE id = $2",
    )
    .bind(&wd)
    .bind(session_id)
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(Json(json!({ "working_directory": wd })))
}

// ============================================================================
// AI title generation — Jaskier Shared Pattern
// ============================================================================

/// POST /api/sessions/:id/generate-title
///
/// Reads the first user message from the session and asks Gemini Flash
/// to produce a concise 3-7 word title. Updates the DB and returns the title.
#[utoipa::path(post, path = "/api/sessions/{id}/generate-title", tag = "sessions",
    params(("id" = String, Path, description = "Session UUID")),
    responses(
        (status = 200, description = "AI-generated title", body = Value),
        (status = 404, description = "Session not found or no user messages"),
        (status = 503, description = "No API key configured")
    )
)]
pub async fn generate_session_title(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let session_id: uuid::Uuid = id.parse().map_err(|_| StatusCode::BAD_REQUEST)?;

    // Fetch first user message
    let first_msg = sqlx::query_scalar::<_, String>(
        "SELECT content FROM gh_chat_messages \
         WHERE session_id = $1 AND role = 'user' \
         ORDER BY created_at ASC LIMIT 1",
    )
    .bind(session_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    // Get Google credential (API key or OAuth token)
    let (api_key, is_oauth) = match crate::oauth::get_google_credential(&state).await {
        Some(cred) => cred,
        None => {
            tracing::warn!("generate_session_title: no Google credential");
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
    };

    // Truncate message to ~500 chars for the prompt (safe UTF-8 boundary)
    let snippet: &str = if first_msg.len() > 500 {
        let end = first_msg
            .char_indices()
            .take_while(|(i, _)| *i < 500)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(500.min(first_msg.len()));
        &first_msg[..end]
    } else {
        &first_msg
    };
    let prompt = format!(
        "Generate a concise 3-7 word title for a chat that starts with this message. \
         Return ONLY the title text, no quotes, no explanation.\n\nMessage: {}",
        snippet
    );

    let model = "gemini-2.0-flash";
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
        model
    );
    let parsed_url = match reqwest::Url::parse(&url) {
        Ok(u) if u.scheme() == "https" => u,
        _ => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    let body = json!({
        "contents": [{ "parts": [{ "text": prompt }] }],
        "generationConfig": { "temperature": 1.0, "maxOutputTokens": 256 }
    });

    let res = crate::oauth::apply_google_auth(state.client.post(parsed_url), &api_key, is_oauth)
        .json(&body)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| {
            tracing::error!("generate_session_title: API call failed: {}", e);
            StatusCode::BAD_GATEWAY
        })?;

    if !res.status().is_success() {
        tracing::error!("generate_session_title: API returned {}", res.status());
        return Err(StatusCode::BAD_GATEWAY);
    }

    let json_resp: Value = res.json().await.map_err(|_| StatusCode::BAD_GATEWAY)?;
    let raw_title = json_resp
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c0| c0.get("content"))
        .and_then(|ct| ct.get("parts"))
        .and_then(|p| p.get(0))
        .and_then(|p0| p0.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("");
    let raw_title = raw_title.trim().trim_matches('"').trim();

    if raw_title.is_empty() {
        tracing::warn!(
            "generate_session_title: Gemini response missing text — {}",
            crate::handlers::gemini_diagnose(&json_resp)
        );
        return Err(StatusCode::BAD_GATEWAY);
    }

    // Sanitize: cap at MAX_TITLE_LENGTH
    let title: String = raw_title.chars().take(MAX_TITLE_LENGTH).collect();

    // Update session title in DB
    sqlx::query("UPDATE gh_sessions SET title = $1, updated_at = NOW() WHERE id = $2")
        .bind(&title)
        .bind(session_id)
        .execute(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tracing::info!(
        "generate_session_title: session {} → {:?}",
        session_id,
        title
    );
    Ok(Json(json!({ "title": title })))
}

// ============================================================================
// Agent unlock & message rating
// ============================================================================

/// Unlock a session's locked agent so the next message gets reclassified.
pub async fn unlock_session_agent(
    State(state): State<AppState>,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> Result<Json<Value>, (axum::http::StatusCode, Json<Value>)> {
    let sid: uuid::Uuid = session_id.parse().map_err(|_| {
        (
            axum::http::StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid session ID"})),
        )
    })?;

    let prev = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT agent_id FROM gh_sessions WHERE id = $1",
    )
    .bind(sid)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("DB error: {}", e)})),
        )
    })?
    .and_then(|(a,)| a);

    sqlx::query("UPDATE gh_sessions SET agent_id = NULL WHERE id = $1")
        .bind(sid)
        .execute(&state.db)
        .await
        .map_err(|e| {
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("DB error: {}", e)})),
            )
        })?;

    Ok(Json(serde_json::json!(UnlockAgentResponse {
        session_id: session_id,
        previous_agent: prev,
        unlocked: true,
    })))
}

/// Rate an AI message for quality feedback.
pub async fn rate_message(
    State(state): State<AppState>,
    Json(body): Json<RatingRequest>,
) -> Result<Json<Value>, (axum::http::StatusCode, Json<Value>)> {
    if body.rating < 1 || body.rating > 5 {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Rating must be between 1 and 5"})),
        ));
    }

    let mid: uuid::Uuid = body.message_id.parse().map_err(|_| {
        (
            axum::http::StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid message ID"})),
        )
    })?;
    let sid: uuid::Uuid = body.session_id.parse().map_err(|_| {
        (
            axum::http::StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid session ID"})),
        )
    })?;

    // Get agent/model from the message
    let msg_info = sqlx::query_as::<_, (Option<String>, Option<String>)>(
        "SELECT agent, model FROM gh_chat_messages WHERE id = $1",
    )
    .bind(mid)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("DB error: {}", e)})),
        )
    })?;

    let (agent_id, model) = msg_info.unwrap_or((None, None));

    sqlx::query(
        "INSERT INTO gh_ratings (message_id, session_id, rating, feedback, agent_id, model) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(mid)
    .bind(sid)
    .bind(body.rating)
    .bind(&body.feedback)
    .bind(&agent_id)
    .bind(&model)
    .execute(&state.db)
    .await
    .map_err(|e| {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("DB error: {}", e)})),
        )
    })?;

    Ok(Json(serde_json::json!(RatingResponse {
        success: true,
        message_id: body.message_id,
    })))
}
