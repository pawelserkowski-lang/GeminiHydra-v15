# Architecture

## System Overview

```mermaid
graph LR
    subgraph Browser
        A[React 19 Frontend<br/>:5176]
    end

    subgraph Server
        B[Vite Dev Proxy<br/>/api -> :8081]
        C[Rust/Axum Backend<br/>:8081]
    end

    subgraph External
        D[Google Gemini API]
        E[Anthropic API<br/>optional]
    end

    A -->|HTTP /api/*| B
    B -->|Proxy| C
    C -->|REST| D
    C -.->|Future| E
```

In production the Vite proxy is replaced by a static file server or reverse proxy (e.g. nginx) pointing `/api` at the Rust binary.

## Agent Classification Pipeline

Every user prompt flows through a deterministic keyword-based classification before reaching the LLM.

```mermaid
sequenceDiagram
    participant U as User
    participant FE as React Frontend
    participant BE as Rust Backend
    participant CL as Classifier
    participant GM as Gemini API

    U->>FE: Submit prompt
    FE->>BE: POST /api/execute { prompt, mode }
    BE->>CL: classify_prompt(prompt)
    CL-->>BE: (agent_id, confidence, reasoning)
    BE->>GM: generateContent (model, prompt)
    GM-->>BE: LLM response text
    BE->>BE: Store in history
    BE-->>FE: ExecuteResponse { id, result, plan, duration_ms }
    FE-->>U: Render response with agent attribution
```

### Classification Rules

The classifier (`handlers.rs::classify_prompt`) applies ordered keyword matching:

1. **Architecture** keywords -> Yennefer
2. **Testing** keywords -> Vesemir
3. **Security** keywords -> Geralt
4. **Monitoring** keywords -> Philippa
5. **Data/Analytics** keywords -> Triss
6. **Documentation** keywords -> Jaskier
7. **Performance** keywords -> Ciri
8. **Strategy** keywords -> Dijkstra
9. **DevOps** keywords -> Lambert
10. **Backend/API** keywords -> Eskel
11. **Research** keywords -> Regis
12. **Frontend/UI** keywords -> Zoltan

When no keywords match, the prompt defaults to **Dijkstra** (Strategy & Planning) with a lower confidence score (0.4 vs 0.85).

## Swarm Orchestration Flow

```mermaid
graph TD
    A[User Prompt] --> B{Keyword Classifier}
    B -->|Security| C[Geralt - Commander]
    B -->|Architecture| D[Yennefer - Commander]
    B -->|Testing| E[Vesemir - Commander]
    B -->|Data| F[Triss - Coordinator]
    B -->|Docs| G[Jaskier - Coordinator]
    B -->|Perf| H[Ciri - Coordinator]
    B -->|Strategy| I[Dijkstra - Coordinator]
    B -->|DevOps| J[Lambert - Executor]
    B -->|Backend| K[Eskel - Executor]
    B -->|Research| L[Regis - Executor]
    B -->|Frontend| M[Zoltan - Executor]
    B -->|Monitoring| N[Philippa - Executor]
    B -->|No match| I

    C & D & E & F & G & H & I & J & K & L & M & N --> O[Gemini API Call]
    O --> P[Response + Agent Attribution]
    P --> Q[Store in History]
    Q --> R[Return to Frontend]
```

### Agent Tier Hierarchy

| Tier        | Agents                             | Responsibility               |
|-------------|-------------------------------------|------------------------------|
| Commander   | Geralt, Yennefer, Vesemir          | Strategic domain leadership  |
| Coordinator | Triss, Jaskier, Ciri, Dijkstra     | Cross-cutting coordination   |
| Executor    | Lambert, Eskel, Regis, Zoltan, Philippa | Task-level execution    |

## Session and Tab Management

The frontend manages sessions and tabs entirely in Zustand with `persist` middleware backed by `localStorage`.

```mermaid
graph TD
    A[viewStore - Zustand 5] --> B[Sessions - max 50]
    A --> C[Tabs - linked to sessions]
    A --> D[Chat History - per session, max 500 msgs]
    A --> E[View State - current view, sidebar]

    B --> F[createSession]
    B --> G[deleteSession]
    B --> H[selectSession]
    B --> I[updateSessionTitle - auto-title from 1st msg]

    C --> J[openTab - reuses existing]
    C --> K[closeTab - respects pin]
    C --> L[switchTab]
    C --> M[reorderTabs]
    C --> N[togglePinTab]

    D --> O[addMessage]
    D --> P[updateLastMessage - streaming append]
    D --> Q[clearHistory]
```

### Key Constraints

- **Max 50 sessions**: oldest sessions (and their history) are evicted when the limit is exceeded
- **Max 500 messages per session**: oldest messages are trimmed when exceeded
- **Auto-titling**: the first user message (up to 30 characters + ellipsis) becomes the session title
- **Tab pinning**: pinned tabs cannot be closed via `closeTab`
- **Tab deduplication**: opening a tab for an already-tabbed session reuses the existing tab

## Backend State Architecture

```mermaid
graph TD
    A[AppState] --> B[AppSettings]
    A --> C[Vec of WitcherAgent - 12]
    A --> D[Vec of ChatMessage - history]
    A --> E[HashMap of API keys]
    A --> F[reqwest::Client]
    A --> G[Vec of MemoryEntry]
    A --> H[Knowledge Graph - nodes + edges]

    B --> B1[temperature: 0.7]
    B --> B2[max_tokens: 8192]
    B --> B3[default_model: gemini-2.0-flash]
    B --> B4[language: en]
    B --> B5[theme: dark]
```

The `AppState` is wrapped in `Arc<Mutex<...>>` and shared across all Axum handlers. The lock is dropped before any async HTTP calls to avoid holding it across await points.
