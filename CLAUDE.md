# GeminiHydra v15 — Multi-Agent AI Swarm

## Quick Start
- `npm run dev` — port 5176
- `npx tsc --noEmit` — type check

## Architecture
- Pure Vite SPA (no Tauri) — React 19 + Zustand 5
- Views: home, chat, agents, history, settings, status
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
- Key modules: `handlers.rs` (system prompt + tool defs), `state.rs` (AppState), `sessions.rs`, `tools.rs`, `files.rs`, `analysis.rs` (tree-sitter code analysis)
- DB: `geminihydra` on localhost:5432 (user: gemini, pass: gemini_local)
- Tables: gh_settings, gh_chat_messages, gh_sessions, gh_memories, gh_knowledge_nodes, gh_knowledge_edges, gh_agents, gh_rag_documents, gh_rag_chunks

## Backend Local Dev
- Wymaga Docker Desktop (PostgreSQL container)
- Image: `pgvector/pgvector:pg16` (NOT standard postgres — pgvector extension required by migration 009)
- Start: `docker run -d --name geminihydra-pg -e POSTGRES_USER=gemini -e POSTGRES_PASSWORD=gemini_local -e POSTGRES_DB=geminihydra -p 5432:5432 pgvector/pgvector:pg16`
- Backend: `DATABASE_URL="postgresql://gemini:gemini_local@localhost:5432/geminihydra" cargo run --release`
- Env vars: `DATABASE_URL` (required), `GOOGLE_API_KEY` or `GEMINI_API_KEY`, `ANTHROPIC_API_KEY` (optional), `PORT` (default 8081)

## Migrations
- Folder: `backend/migrations/`
- SQLx sorts by filename prefix — each migration MUST have a unique date prefix
- Current order: 20260214_001 → 20260215_002 → 20260216_003 → 20260217_004 → 20260218_005 → 20260219_006 → 20260220_007 → 20260221_008 → 20260222_009
- Migration 009 requires pgvector extension (`CREATE EXTENSION IF NOT EXISTS vector`)

## Agent System Prompt
- Defined in `backend/src/handlers.rs` → `build_system_prompt()`
- Contains "CRITICAL: Local Machine Access" section — tells agents they run on user's LOCAL machine with FULL filesystem access
- Tool definitions in `build_tools()` — explicit local filesystem descriptions with Windows path examples
- Without this section, Gemini models default to "I can't access your files"

## Agent Tools (all tested & working)
- `read_file` — reads local files by absolute path
- `write_file` — creates/overwrites local files
- `list_directory` — lists directory contents with sizes
- `execute_command` — runs shell commands (build, test, git, etc.)

## Tree-sitter (Code Analysis)
- tree-sitter v0.24+ with streaming-iterator crate
- Languages: Rust, TypeScript, JavaScript, Python, Go
- API: `LANGUAGE` constants (not `language()` functions), `StreamingIterator` (not `Iterator`)

## Conventions
- motion/react (NOT framer-motion) for animations
- Biome for linting (not ESLint)
- npm (not pnpm) as package manager

## Dead Code Cleanup (2026-02-24)
- Removed 18 files, -380 lines of unused code
- Deleted: useHistory.ts (4 hooks), useGeminiModelsQuery, useClassifyMutation, useExecuteMutation, useFileListMutation, useHealthQuery, useSessionQuery, clearHistory action, selectCurrentMessages/selectSortedSessions/selectMessageCount selectors, DataSkeleton component, 6 barrel index.ts files, empty workers/ dir
- Schema types made private: fileEntrySchema, geminiModelSchema, historyMessageSchema, memoryEntrySchema, knowledgeNodeSchema, knowledgeEdgeSchema

## Knowledge Base (SQLite)
- Plik: `C:\Users\BIURODOM\Desktop\jaskier_knowledge.db`
- Zawiera kompletną wiedzę o 4 projektach: Regis, ClaudeHydra-v4, GeminiHydra-v15, Tissaia-v4
- Tabele: projects, dependencies, components, views, stores, hooks, theme_tokens, i18n_keys, api_endpoints, scripts, public_assets, shared_patterns, store_api_diff, unique_features, source_files, metadata
- 479 rekordów, wygenerowane 2026-02-15
- Query: `py -c "import sqlite3; c=sqlite3.connect(r'C:\Users\BIURODOM\Desktop\jaskier_knowledge.db'); [print(r) for r in c.execute('SELECT * FROM projects')]"`
