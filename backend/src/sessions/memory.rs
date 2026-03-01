//! Memory and knowledge-graph handlers.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde_json::{json, Value};

use crate::models::{KnowledgeEdgeRow, KnowledgeNodeRow, MemoryRow};
use crate::state::AppState;

use super::{
    AddMemoryRequest, ClearMemoryParams, KnowledgeEdge, KnowledgeNode, MemoryQueryParams,
};

// ============================================================================
// Memory handlers
// ============================================================================

/// GET /api/memory/memories?agent=Geralt&topK=10
#[utoipa::path(get, path = "/api/memory/memories", tag = "memory",
    responses((status = 200, description = "Agent memories", body = Value))
)]
pub async fn list_memories(
    State(state): State<AppState>,
    Query(params): Query<MemoryQueryParams>,
) -> Result<Json<Value>, StatusCode> {
    let top_k = params.top_k.unwrap_or(10) as i64;

    let rows = match &params.agent {
        Some(agent) => {
            sqlx::query_as::<_, MemoryRow>(
                "SELECT id, agent, content, importance, created_at \
                 FROM gh_memories WHERE LOWER(agent) = LOWER($1) \
                 ORDER BY importance DESC LIMIT $2",
            )
            .bind(agent)
            .bind(top_k)
            .fetch_all(&state.db)
            .await
        }
        None => {
            sqlx::query_as::<_, MemoryRow>(
                "SELECT id, agent, content, importance, created_at \
                 FROM gh_memories ORDER BY importance DESC LIMIT $1",
            )
            .bind(top_k)
            .fetch_all(&state.db)
            .await
        }
    }
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let memories: Vec<super::MemoryEntry> = rows.into_iter().map(super::row_to_memory).collect();
    let count = memories.len();

    Ok(Json(json!({
        "memories": memories,
        "count": count,
    })))
}

/// POST /api/memory/memories
#[utoipa::path(post, path = "/api/memory/memories", tag = "memory",
    responses((status = 200, description = "Memory added", body = Value))
)]
pub async fn add_memory(
    State(state): State<AppState>,
    Json(body): Json<AddMemoryRequest>,
) -> Result<Json<Value>, StatusCode> {
    let importance = body.importance.clamp(0.0, 1.0);

    let row = sqlx::query_as::<_, MemoryRow>(
        "INSERT INTO gh_memories (agent, content, importance) VALUES ($1, $2, $3) \
         RETURNING id, agent, content, importance, created_at",
    )
    .bind(&body.agent)
    .bind(&body.content)
    .bind(importance)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let entry = super::row_to_memory(row);
    Ok(Json(json!(entry)))
}

/// DELETE /api/memory/memories?agent=Geralt
#[utoipa::path(delete, path = "/api/memory/memories", tag = "memory",
    responses((status = 200, description = "Memories cleared", body = Value))
)]
pub async fn clear_memories(
    State(state): State<AppState>,
    Query(params): Query<ClearMemoryParams>,
) -> Result<Json<Value>, StatusCode> {
    match &params.agent {
        Some(agent) => {
            sqlx::query("DELETE FROM gh_memories WHERE LOWER(agent) = LOWER($1)")
                .bind(agent)
                .execute(&state.db)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok(Json(json!({ "cleared": true, "agent": agent })))
        }
        None => {
            sqlx::query("DELETE FROM gh_memories")
                .execute(&state.db)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok(Json(json!({ "cleared": true, "agent": null })))
        }
    }
}

// ============================================================================
// Knowledge-graph handlers
// ============================================================================

/// GET /api/memory/graph
#[utoipa::path(get, path = "/api/memory/graph", tag = "memory",
    responses((status = 200, description = "Knowledge graph nodes and edges", body = Value))
)]
pub async fn get_knowledge_graph(
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    let node_rows = sqlx::query_as::<_, KnowledgeNodeRow>(
        "SELECT id, node_type, label FROM gh_knowledge_nodes",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let edge_rows = sqlx::query_as::<_, KnowledgeEdgeRow>(
        "SELECT source, target, label FROM gh_knowledge_edges",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let nodes: Vec<KnowledgeNode> = node_rows.into_iter().map(super::row_to_node).collect();
    let edges: Vec<KnowledgeEdge> = edge_rows.into_iter().map(super::row_to_edge).collect();

    Ok(Json(json!({
        "nodes": nodes,
        "edges": edges,
    })))
}

/// POST /api/memory/graph/nodes
#[utoipa::path(post, path = "/api/memory/graph/nodes", tag = "memory",
    responses((status = 200, description = "Knowledge node added/updated", body = Value))
)]
pub async fn add_knowledge_node(
    State(state): State<AppState>,
    Json(node): Json<KnowledgeNode>,
) -> Result<Json<Value>, StatusCode> {
    sqlx::query(
        "INSERT INTO gh_knowledge_nodes (id, node_type, label) VALUES ($1, $2, $3) \
         ON CONFLICT (id) DO UPDATE SET node_type = EXCLUDED.node_type, label = EXCLUDED.label",
    )
    .bind(&node.id)
    .bind(&node.node_type)
    .bind(&node.label)
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!(node)))
}

/// POST /api/memory/graph/edges
#[utoipa::path(post, path = "/api/memory/graph/edges", tag = "memory",
    responses((status = 200, description = "Knowledge edge added", body = Value))
)]
pub async fn add_graph_edge(
    State(state): State<AppState>,
    Json(edge): Json<KnowledgeEdge>,
) -> Result<Json<Value>, StatusCode> {
    sqlx::query(
        "INSERT INTO gh_knowledge_edges (source, target, label) VALUES ($1, $2, $3) \
         ON CONFLICT DO NOTHING",
    )
    .bind(&edge.source)
    .bind(&edge.target)
    .bind(&edge.label)
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!(edge)))
}
