use serde_json::{json, Value};
use uuid::Uuid;
use crate::core::agent::classify_prompt;

pub async fn resolve_session_agent(db: &sqlx::PgPool, sid: &Uuid, prompt: &str) -> (String, f64, String) {
    if let Ok(Some((Some(aid),))) = sqlx::query_as::<_, (Option<String>,)>("SELECT agent_id FROM gh_sessions WHERE id = $1")
        .bind(sid)
        .fetch_optional(db)
        .await 
    {
        if !aid.is_empty() {
            return (aid, 0.95, "Locked".into());
        }
    }
    let (aid, conf, reas) = classify_prompt(prompt);
    let _ = sqlx::query("UPDATE gh_sessions SET agent_id = $1 WHERE id = $2")
        .bind(&aid)
        .bind(sid)
        .execute(db)
        .await;
    (aid, conf, reas)
}

pub async fn load_session_history(db: &sqlx::PgPool, sid: &Uuid) -> Vec<Value> {
    sqlx::query_as::<_, (String, String)>("SELECT role, content FROM gh_chat_messages WHERE session_id = $1 ORDER BY created_at DESC LIMIT 20")
        .bind(sid)
        .fetch_all(db)
        .await
        .unwrap_or_default()
        .into_iter()
        .rev()
        .map(|(r, c)| json!({ "role": if r == "assistant" { "model" } else { "user" }, "parts": [{ "text": c }] }))
        .collect()
}

pub async fn store_messages(
    db: &sqlx::PgPool, 
    sid: Option<Uuid>, 
    rid: Uuid, 
    prompt: &str, 
    result: &str, 
    agent_id: &str, 
    model: &str, 
    reasoning: &str
) {
    let _ = sqlx::query("INSERT INTO gh_chat_messages (id, role, content, model, agent, session_id) VALUES ($1, 'user', $2, $3, $4, $5)")
        .bind(rid)
        .bind(prompt)
        .bind(Some(model))
        .bind(Some(agent_id))
        .bind(sid)
        .execute(db)
        .await;
    
    if !result.is_empty() {
        let _ = sqlx::query("INSERT INTO gh_chat_messages (id, role, content, model, agent, session_id) VALUES ($1, 'assistant', $2, $3, $4, $5)")
            .bind(Uuid::new_v4())
            .bind(result)
            .bind(Some(model))
            .bind(Some(reasoning))
            .bind(sid)
            .execute(db)
            .await;
    }
}
