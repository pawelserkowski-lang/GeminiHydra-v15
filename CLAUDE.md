# GeminiHydra v15 — Multi-Agent AI Swarm

## Quick Start
- `npm run dev` — port 5176
- `npx tsc --noEmit` — type check

## Architecture
- Pure Vite SPA (no Tauri) — React 19 + Zustand 5
- Views: home, chat, agents, history, settings, status, logs
- Lazy-loaded via React.lazy in `src/main.tsx`
- Sidebar: `src/components/organisms/Sidebar.tsx` (grouped nav, session list, theme/lang toggles)

## Key Files
- `src/features/home/components/WelcomeScreen.tsx` — wzorcowy home view (ported to CH/Tissaia)
- `src/shared/hooks/useViewTheme.ts` — canonical theme hook (identical in all 3 apps)
- `src/stores/viewStore.ts` — sessions, chatHistory, tabs, navigation
- `src/components/atoms/` — Badge, Button (shared API across apps)

## Sidebar Logo Gotcha
- Logo + tekst "GeminiHydra" renderowane OBOK SIEBIE (flex row) — ogranicza max logo size
- Expanded: `h-36` max (h-48 powoduje overflow tekstu poza sidebar)
- Collapsed sidebar width: 64px → logo max `w-16 h-16`

## Backend (Rust/Axum)
- Port: 8081 | Prod: geminihydra-v15-backend.fly.dev
- Stack: Rust + Axum 0.8 + SQLx + PostgreSQL 17 (pgvector)
- Route syntax: `{id}` (NOT `:id` — axum 0.8 breaking change)
- Entry point: `backend/src/lib.rs` → `create_router()` builds all API routes
- Key modules: `handlers.rs` (system prompt + tool defs), `state.rs` (AppState + LogRingBuffer), `sessions.rs`, `logs.rs` (4 log endpoints — backend/audit/flyio/activity), `tools/` (mod.rs + fs_tools.rs + pdf_tools.rs + zip_tools.rs + image_tools.rs + git_tools.rs + github_tools.rs + vercel_tools.rs + fly_tools.rs), `files.rs`, `analysis.rs` (tree-sitter code analysis), `model_registry.rs` (auto-fetches models from providers at startup, selects best chat/thinking/image model), `oauth.rs` (Anthropic OAuth PKCE), `oauth_google.rs` (Google OAuth PKCE + API key), `oauth_github.rs` (GitHub OAuth), `oauth_vercel.rs` (Vercel OAuth), `service_tokens.rs` (Fly.io PAT), `mcp/` (client.rs + server.rs + config.rs), `a2a.rs` (A2A v0.3 protocol)
- DB: `geminihydra` on localhost:5432 (user: gemini, pass: gemini_local)
- Tables: gh_settings, gh_chat_messages, gh_sessions, gh_memories, gh_knowledge_nodes, gh_knowledge_edges, gh_agents, gh_rag_documents, gh_rag_chunks, gh_model_pins, gh_oauth_tokens, gh_google_auth, gh_oauth_github, gh_oauth_vercel, gh_service_tokens, gh_mcp_servers, gh_mcp_discovered_tools, gh_a2a_tasks, gh_a2a_messages, gh_a2a_artifacts, gh_audit_log

## Backend Local Dev
- Wymaga Docker Desktop (PostgreSQL container)
- Image: `pgvector/pgvector:pg16` (NOT standard postgres — pgvector extension required for embeddings locally)
- Start: `docker run -d --name geminihydra-pg -e POSTGRES_USER=gemini -e POSTGRES_PASSWORD=gemini_local -e POSTGRES_DB=geminihydra -p 5432:5432 pgvector/pgvector:pg16`
- Backend: `DATABASE_URL="postgresql://gemini:gemini_local@localhost:5432/geminihydra" cargo run --release`
- Env vars: `DATABASE_URL` (required), `GOOGLE_API_KEY` or `GEMINI_API_KEY`, `ANTHROPIC_API_KEY` (optional), `GOOGLE_OAUTH_CLIENT_ID` + `GOOGLE_OAUTH_CLIENT_SECRET` (optional — enables Google OAuth button), `GITHUB_CLIENT_ID` + `GITHUB_CLIENT_SECRET` (optional), `VERCEL_CLIENT_ID` + `VERCEL_CLIENT_SECRET` (optional), `PORT` (default 8081)

## Fly.io Deploy
- App: `geminihydra-v15-backend` | Region: `arn` | VM: shared-cpu-1x 256MB
- Deploy: `cd backend && fly deploy`
- Dockerfile: multi-stage (rust builder → debian:trixie-slim runtime)
- DB: Fly Postgres `jaskier-db` → database `geminihydra_v15_backend` (NOT `geminihydra`!)
- Shared DB cluster `jaskier-db` hosts: geminihydra_v15_backend, claudehydra_v4_backend, tissaia_v4_backend
- Secrets: `DATABASE_URL`, `GOOGLE_API_KEY`, `ANTHROPIC_API_KEY`, `AUTH_SECRET` (set via `fly secrets set`)
- auto_stop_machines=stop, auto_start_machines=true, min_machines=0 (scales to zero)
- Connect to prod DB: `fly pg connect -a jaskier-db -d geminihydra_v15_backend`
- Logs: `fly logs --no-tail` or `fly logs`
- Health: `curl https://geminihydra-v15-backend.fly.dev/api/health`

## Migrations
- Folder: `backend/migrations/`
- SQLx sorts by filename prefix — each migration MUST have a unique date prefix
- Current order: 20260214_001 → 20260215_002 → 20260216_003 → 20260217_004 → 20260218_005 → 20260219_006 → 20260220_007 → 20260221_008 → 20260222_009 → 20260224_010 → 20260225_011
- All migrations MUST be idempotent (IF NOT EXISTS, ON CONFLICT DO NOTHING) — SQLx checks checksums
- All migration files MUST use LF line endings (not CRLF) — `.gitattributes` with `*.sql text eol=lf` enforces this
- Migration 009: pgvector wrapped in DO/EXCEPTION block — skips gracefully if extension unavailable
- Migration 010: model_pins table for pinning preferred models per role
- Migration 011: oauth_tokens table (singleton row, PKCE tokens for Anthropic Claude MAX)

## Migrations Gotchas (learned the hard way)
- **Checksum mismatch on deploy**: SQLx stores SHA-256 checksum per migration. If line endings change (CRLF→LF between Windows and Docker), checksum won't match → `VersionMismatch` panic. Fix: reset `_sqlx_migrations` table
- **Duplicate date prefixes**: Multiple files with same prefix (e.g. `20260218_005`, `20260218_006`) cause `duplicate key` error on fresh DB init. Each file MUST have unique prefix
- **pgvector not on fly.io**: Fly Postgres (`jaskier-db`) does NOT have pgvector extension. Migration 009 uses `DO $$ ... EXCEPTION WHEN OTHERS` to skip embeddings table creation gracefully
- **Reset prod DB migrations**: `fly pg connect -a jaskier-db -d geminihydra_v15_backend` then `DROP TABLE _sqlx_migrations CASCADE;` + drop all gh_* tables, then redeploy

## Dynamic Model Registry
- At startup `model_registry::startup_sync()` fetches all models from Google + Anthropic APIs
- Caches them in `AppState.model_cache` (TTL 1h, refreshed on demand via `/api/models/refresh`)
- Currently: 22 models cached (22 Google, keys: `["google"]`)
- Auto-selects best model per use case using `version_key()` sort (highest version + date wins):
  - **chat**: latest `pro` (excludes lite, image, tts, thinking, computer, robotics, customtools) → `gemini-3.1-pro-preview`
  - **thinking**: latest `flash thinking` (fallback to chat) → `gemini-3.1-pro-preview`
  - **image**: latest model with `image` in ID (excludes robotics, computer) → `gemini-3-pro-image-preview`
- Persists chosen chat model into `gh_settings.default_model` at startup
- No hardcoded model list — adapts automatically when Google releases new models
- Pin override: `POST /api/models/pin` saves to `gh_model_pins` (priority 1, above auto-selection)
- API endpoints: `GET /api/models`, `POST /api/models/refresh`, `POST /api/models/pin`, `DELETE /api/models/pin/{use_case}`, `GET /api/models/pins`
- Health check (`/api/health`) shows dynamic provider list from cache (not static)
- `reset_settings` uses `get_model_id()` instead of hardcoded model name

## OAuth / Authentication (Anthropic Claude MAX Plan)
- Ported from ClaudeHydra-v4 (identical PKCE flow, adapted table prefix `gh_`)
- Backend module: `backend/src/oauth.rs` — handlers: `auth_status`, `auth_login`, `auth_callback`, `auth_logout`
- State: `OAuthPkceState` in `state.rs` → `AppState.oauth_pkce: Arc<RwLock<Option<OAuthPkceState>>>`
- DB table: `gh_oauth_tokens` (singleton row with `id=1` CHECK constraint)
- Token auto-refresh: `get_valid_access_token()` refreshes expired tokens automatically
- API endpoints: `GET /api/auth/status`, `POST /api/auth/login`, `POST /api/auth/callback`, `POST /api/auth/logout`
- Frontend: `src/features/settings/components/OAuthSection.tsx` — 3-step PKCE flow (idle → waiting_code → exchanging)
- Integrated in `SettingsView.tsx` as "Authentication" Card section
- Cargo deps: `sha2`, `base64`, `rand`, `url`

## OAuth / Authentication (Google OAuth PKCE + API Key)
- Backend module: `backend/src/oauth_google.rs` — Google OAuth 2.0 redirect-based PKCE flow + API key management
- DB table: `gh_google_auth` (singleton row, id=1 CHECK) — stores auth_method, access_token, refresh_token, expires_at, api_key_encrypted, user_email, user_name
- `get_google_credential(state)` → credential resolution: DB OAuth token → DB API key → env var (`GOOGLE_API_KEY`/`GEMINI_API_KEY`)
- `apply_google_auth(builder, credential, is_oauth)` — sets `Authorization: Bearer` (OAuth) or `x-goog-api-key` (API key)
- API endpoints: `GET /api/auth/status` (includes `oauth_available`), `POST /api/auth/login`, `GET /api/auth/google/redirect`, `POST /api/auth/logout`, `POST/DELETE /api/auth/apikey`
- Env vars: `GOOGLE_OAUTH_CLIENT_ID`, `GOOGLE_OAUTH_CLIENT_SECRET` (optional — OAuth button hidden if not set)
- Google Cloud Console: app "Jaskier", redirect URI: `http://localhost:8081/api/auth/google/redirect`
- Frontend: `OAuthSection.tsx` + `useAuthStatus.ts` — polling-based (2s interval during OAuth pending), API key input + Google OAuth button

## OAuth — GitHub + Vercel + Fly.io
- `oauth_github.rs` — GitHub OAuth code exchange, DB table `gh_oauth_github`, endpoints `/api/auth/github/*`
- `oauth_vercel.rs` — Vercel OAuth code exchange, DB table `gh_oauth_vercel`, endpoints `/api/auth/vercel/*`
- `service_tokens.rs` — encrypted PAT storage (AES-256-GCM), DB table `gh_service_tokens`, endpoints `/api/tokens`
- Used by: `github_tools.rs`, `vercel_tools.rs`, `fly_tools.rs`

## Agent System Prompt
- Defined in `backend/src/handlers.rs` → `build_system_prompt()`
- Contains "CRITICAL: Local Machine Access" section — tells agents they run on user's LOCAL machine with FULL filesystem access
- Contains "Tool Selection Rules" section — forces Gemini to use `list_directory`/`read_file`/`write_file` instead of `execute_command` for file ops
- Tool definitions in `build_tools()` — explicit local filesystem descriptions with Windows path examples
- Without "Local Machine Access" section, Gemini models default to "I can't access your files"
- Without "Tool Selection Rules", Gemini wastes iterations using `execute_command` with Linux commands on Windows

## Agent Tools (31+ tools, all tested & working)
- **Filesystem** (fs_tools.rs): `list_directory`, `read_file`, `search_files`, `get_code_structure`, `write_file`, `edit_file`, `read_file_section`, `find_file`, `diff_files`, `execute_command` (LAST RESORT, cmd.exe, 30s timeout)
- **PDF/ZIP** (pdf_tools.rs, zip_tools.rs): `read_pdf`, `list_zip`, `extract_zip_file`
- **Image** (image_tools.rs): `analyze_image` (Gemini Vision API)
- **Git** (git_tools.rs): `git_status`, `git_log`, `git_diff`, `git_branch`, `git_commit` (NO push)
- **GitHub** (github_tools.rs): `github_list_repos`, `github_get_repo`, `github_list_issues`, `github_get_issue`, `github_create_issue`, `github_create_pr`
- **Vercel** (vercel_tools.rs): `vercel_list_projects`, `vercel_deploy`, `vercel_get_deployment`
- **Fly.io** (fly_tools.rs): `fly_list_apps`, `fly_get_status`, `fly_get_logs` (read-only)
- **A2A** (a2a.rs): `call_agent` — inter-agent delegation via A2A v0.3 protocol
- **MCP proxy**: `mcp_{server}_{tool}` — routed via `state.mcp_client.call_tool()`

## Tree-sitter (Code Analysis)
- tree-sitter v0.24+ with streaming-iterator crate
- Languages: Rust, TypeScript, JavaScript, Python, Go
- API: `LANGUAGE` constants (not `language()` functions), `StreamingIterator` (not `Iterator`)

## Conventions
- motion/react (NOT framer-motion) for animations
- Biome for linting (not ESLint)
- npm (not pnpm) as package manager

## WebSocket Streaming Fix (2026-02-24)
- **Problem**: Tool-call responses were empty — WebSocket killed by heartbeat before tokens arrived
- **Root cause**: Frontend heartbeat (30s ping + 10s pong timeout = 40s max) < tool execution time (60s+). Backend blocks on `execute_streaming().await` and can't respond to pings during tool loops
- **Fix 1 — Frontend** (`src/shared/hooks/useWebSocketChat.ts`): Heartbeat paused during streaming via `isStreamingRef`, reset on ANY incoming WS message, resumed after `complete`/`error`
- **Fix 2 — Backend** (`handlers.rs` handle_ws): Changed from sequential while loop to `tokio::select!` with explicit WebSocket Ping/Pong frame handling concurrent with message processing
- **Fix 3 — Backend** (`handlers.rs` build_system_prompt): Added "Tool Selection Rules" — forces `list_directory`/`read_file`/`write_file` over `execute_command` for file ops, declares Windows environment
- **Gotcha**: Zod `wsServerMessageSchema` only has start/token/plan/complete/error/pong — `tool_call`/`tool_result` types silently dropped by safeParse (tool output rendered via Token messages instead)

## Dead Code Cleanup (2026-02-24)
- Removed 18 files, -380 lines of unused code
- Deleted: useHistory.ts (4 hooks), useGeminiModelsQuery, useClassifyMutation, useExecuteMutation, useFileListMutation, useHealthQuery, useSessionQuery, clearHistory action, selectCurrentMessages/selectSortedSessions/selectMessageCount selectors, DataSkeleton component, 6 barrel index.ts files, empty workers/ dir
- Schema types made private: fileEntrySchema, geminiModelSchema, historyMessageSchema, memoryEntrySchema, knowledgeNodeSchema, knowledgeEdgeSchema

## Logs View (F21)
- Frontend: `src/features/logs/` — `LogsView.tsx` (4 tabs: Backend/Audit/Fly.io/Activity) + `useLogs.ts` (TanStack Query hooks, 5s polling)
- Backend: `logs.rs` — 4 endpoints (`/api/logs/backend`, `/api/logs/audit`, `/api/logs/flyio`, `/api/logs/activity`)
- `LogRingBuffer` in `state.rs` — in-memory ring buffer (capacity 1000) with `std::sync::Mutex`
- `LogBufferLayer` in `main.rs` — custom tracing Layer capturing events into ring buffer
- Sidebar: `ScrollText` icon, i18n keys `nav.logs`, `logs.*`
- View type: `| 'logs'` in `src/stores/types.ts`

## Workspace CLAUDE.md (canonical reference)
- Full Jaskier ecosystem docs: `C:\Users\BIURODOM\Desktop\ClaudeDesktop\CLAUDE.md`
- Covers: shared patterns, cross-project conventions, backend safety rules, OAuth details, MCP, A2A, ONNX pipeline, fly.io infra
- This file is a project-scoped summary; workspace CLAUDE.md is the source of truth
- Last synced: 2026-03-01 (F21)

## Knowledge Base (SQLite)
- Plik: `C:\Users\BIURODOM\Desktop\ClaudeDesktop\jaskier_knowledge.db`
- Zawiera kompletną wiedzę o 4 projektach
- Tabele: projects, dependencies, components, views, stores, hooks, theme_tokens, i18n_keys, api_endpoints, scripts, public_assets, shared_patterns, store_api_diff, unique_features, source_files
- 535 rekordów, ostatni sync: 2026-02-24 17:38
- Query: `py -c "import sqlite3; c=sqlite3.connect(r'C:\Users\BIURODOM\Desktop\ClaudeDesktop\jaskier_knowledge.db'); [print(r) for r in c.execute('SELECT * FROM projects')]"`
