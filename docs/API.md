# API Reference

Base URL: `http://localhost:8081` (backend direct) or `http://localhost:5176/api` (via Vite proxy)

All endpoints are prefixed with `/api`. Responses are JSON.

---

## Health

### GET /api/health

Basic health check.

```bash
curl http://localhost:8081/api/health
```

```json
{
  "status": "ok",
  "version": "15.0.0",
  "app": "GeminiHydra",
  "uptime_seconds": 42,
  "providers": [
    { "name": "Google Gemini", "available": true, "model": "gemini-2.0-flash" },
    { "name": "Anthropic Claude", "available": false, "model": "claude-sonnet-4-20250514" }
  ]
}
```

### GET /api/health/detailed

Extended health check with system metrics.

```bash
curl http://localhost:8081/api/health/detailed
```

```json
{
  "status": "ok",
  "version": "15.0.0",
  "app": "GeminiHydra",
  "uptime_seconds": 42,
  "providers": [ ... ],
  "memory_usage_mb": 1024.5,
  "cpu_usage_percent": 12.3,
  "platform": "windows"
}
```

---

## Agents

### GET /api/agents

List all 12 Witcher agents.

```bash
curl http://localhost:8081/api/agents
```

```json
{
  "agents": [
    {
      "id": "geralt",
      "name": "Geralt",
      "role": "Security & Protection",
      "tier": "Commander",
      "status": "online",
      "description": "The White Wolf -- leads security strategy..."
    }
  ]
}
```

### POST /api/agents/classify

Classify a prompt to determine which agent should handle it.

```bash
curl -X POST http://localhost:8081/api/agents/classify \
  -H "Content-Type: application/json" \
  -d '{"prompt": "How do I optimize database queries?"}'
```

```json
{
  "agent": "triss",
  "confidence": 0.85,
  "reasoning": "Prompt relates to data and analytics"
}
```

---

## Execute

### POST /api/execute

Send a prompt through the agent classification pipeline and Gemini API.

```bash
curl -X POST http://localhost:8081/api/execute \
  -H "Content-Type: application/json" \
  -d '{"prompt": "Explain microservices architecture", "mode": "chat"}'
```

**Request body:**

| Field    | Type   | Required | Description                          |
|----------|--------|----------|--------------------------------------|
| `prompt` | string | yes      | The user prompt                      |
| `mode`   | string | yes      | Execution mode (e.g. `"chat"`)       |
| `model`  | string | no       | Override model (default: `gemini-2.0-flash`) |

**Response:**

```json
{
  "id": "a1b2c3d4-...",
  "result": "Microservices architecture is a design approach...",
  "plan": {
    "agent": "yennefer",
    "steps": [
      "classify prompt",
      "route to agent (confidence 85%)",
      "call Gemini model gemini-2.0-flash",
      "return result"
    ],
    "estimated_time": "1200ms"
  },
  "duration_ms": 1200,
  "mode": "chat"
}
```

---

## Gemini Models

### GET /api/gemini/models

List available Gemini models that support `generateContent`.

```bash
curl http://localhost:8081/api/gemini/models
```

```json
{
  "models": [
    {
      "name": "models/gemini-2.0-flash",
      "display_name": "Gemini 2.0 Flash",
      "supported_generation_methods": ["generateContent", "countTokens"]
    }
  ]
}
```

Returns `{"models": [], "error": "..."}` if no API key is configured.

---

## System Stats

### GET /api/system/stats

Real-time system resource usage.

```bash
curl http://localhost:8081/api/system/stats
```

```json
{
  "cpu_usage_percent": 15.2,
  "memory_used_mb": 8192.0,
  "memory_total_mb": 32768.0,
  "platform": "windows"
}
```

---

## History

### GET /api/history

Retrieve chat history. Returns the most recent messages.

```bash
curl "http://localhost:8081/api/history?limit=20"
```

| Param   | Type | Default | Description             |
|---------|------|---------|-------------------------|
| `limit` | int  | 50      | Max messages to return  |

```json
{
  "messages": [
    {
      "id": "uuid-...",
      "role": "user",
      "content": "Hello",
      "model": "gemini-2.0-flash",
      "timestamp": "2025-01-15T10:30:00Z",
      "agent": "dijkstra"
    }
  ],
  "total": 100,
  "returned": 20
}
```

### POST /api/history

Add a message to history.

```bash
curl -X POST http://localhost:8081/api/history \
  -H "Content-Type: application/json" \
  -d '{"role": "user", "content": "Test message", "model": "gemini-2.0-flash", "agent": "geralt"}'
```

### DELETE /api/history

Clear all chat history.

```bash
curl -X DELETE http://localhost:8081/api/history
```

```json
{ "cleared": true }
```

### GET /api/history/search

Search chat history by content substring.

```bash
curl "http://localhost:8081/api/history/search?q=security"
```

```json
{
  "query": "security",
  "results": [ ... ],
  "count": 3
}
```

---

## Settings

### GET /api/settings

Get current application settings.

```bash
curl http://localhost:8081/api/settings
```

```json
{
  "temperature": 0.7,
  "max_tokens": 8192,
  "default_model": "gemini-2.0-flash",
  "language": "en",
  "theme": "dark"
}
```

### PATCH /api/settings

Partially update settings. Only provided fields are changed.

```bash
curl -X PATCH http://localhost:8081/api/settings \
  -H "Content-Type: application/json" \
  -d '{"temperature": 0.9, "theme": "light"}'
```

### POST /api/settings/reset

Reset all settings to defaults.

```bash
curl -X POST http://localhost:8081/api/settings/reset
```

---

## Memory

### GET /api/memory/memories

List agent memories, optionally filtered by agent. Sorted by importance (descending).

```bash
curl "http://localhost:8081/api/memory/memories?agent=geralt&topK=5"
```

| Param   | Type   | Default | Description                    |
|---------|--------|---------|--------------------------------|
| `agent` | string | all     | Filter by agent name           |
| `topK`  | int    | 10      | Max memories to return         |

```json
{
  "memories": [
    {
      "id": "uuid-...",
      "agent": "geralt",
      "content": "User prefers security-first approaches",
      "importance": 0.9,
      "timestamp": "2025-01-15T10:30:00Z"
    }
  ],
  "count": 1
}
```

### POST /api/memory/memories

Add a memory entry.

```bash
curl -X POST http://localhost:8081/api/memory/memories \
  -H "Content-Type: application/json" \
  -d '{"agent": "geralt", "content": "Prefers Rust for backend", "importance": 0.8}'
```

### DELETE /api/memory/memories

Clear memories. Optionally filter by agent.

```bash
curl -X DELETE "http://localhost:8081/api/memory/memories?agent=geralt"
```

---

## Knowledge Graph

### GET /api/memory/graph

Get the full knowledge graph (nodes and edges).

```bash
curl http://localhost:8081/api/memory/graph
```

```json
{
  "nodes": [
    { "id": "n1", "node_type": "concept", "label": "Rust" }
  ],
  "edges": [
    { "source": "n1", "target": "n2", "label": "used_by" }
  ]
}
```

### POST /api/memory/graph/nodes

Add a node to the knowledge graph.

```bash
curl -X POST http://localhost:8081/api/memory/graph/nodes \
  -H "Content-Type: application/json" \
  -d '{"id": "n1", "node_type": "concept", "label": "Rust"}'
```

### POST /api/memory/graph/edges

Add an edge to the knowledge graph.

```bash
curl -X POST http://localhost:8081/api/memory/graph/edges \
  -H "Content-Type: application/json" \
  -d '{"source": "n1", "target": "n2", "label": "depends_on"}'
```
