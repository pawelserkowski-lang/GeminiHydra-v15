use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
use std::time::Instant;

use crate::models::{ExecuteRequest, ExecuteResponse, ExecutePlan, WsClientMessage, WsServerMessage};
use crate::state::AppState;
use crate::core::execution::{execute_streaming, prepare_execution, ws_send};

pub async fn execute(State(state): State<AppState>, Json(body): Json<ExecuteRequest>) -> Json<Value> {
    let start = Instant::now();
    let ctx = prepare_execution(&state, &body.prompt, body.model.clone(), None).await;
    if ctx.api_key.is_empty() { return Json(json!({ "error": "No API Key" })); }

    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}", ctx.model, ctx.api_key);
    let gem_body = json!({ "systemInstruction": { "parts": [{ "text": ctx.system_prompt }] }, "contents": [{ "parts": [{ "text": ctx.final_user_prompt }] }] });

    let res = state.client.post(&url).json(&gem_body).send().await;
    let text = match res {
        Ok(r) if r.status().is_success() => {
            let j: Value = r.json().await.unwrap_or_default();
            j["candidates"][0]["content"]["parts"][0]["text"].as_str().unwrap_or("Error").to_string()
        }
        _ => "API Error".to_string(),
    };

    Json(json!(ExecuteResponse {
        id: Uuid::new_v4().to_string(),
        result: text,
        plan: Some(ExecutePlan { agent: Some(ctx.agent_id), steps: ctx.steps, estimated_time: None }),
        duration_ms: start.elapsed().as_millis() as u64,
        mode: body.mode,
        files_loaded: ctx.files_loaded,
    }))
}

pub async fn ws_execute(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let cancel = CancellationToken::new();

    while let Some(Ok(WsMessage::Text(text))) = receiver.next().await {
        let client_msg: WsClientMessage = match serde_json::from_str(&text) {
            Ok(m) => m,
            Err(e) => {
                let _ = ws_send(&mut sender, &WsServerMessage::Error { message: e.to_string(), code: Some("PARSE_ERROR".into()) }).await;
                continue;
            }
        };

        match client_msg {
            WsClientMessage::Ping => { let _ = ws_send(&mut sender, &WsServerMessage::Pong).await; }
            WsClientMessage::Cancel => { cancel.cancel(); }
            WsClientMessage::Execute { prompt, model, session_id, .. } => {
                execute_streaming(&mut sender, &state, &prompt, model, session_id, cancel.child_token()).await;
            }
        }
    }
}
