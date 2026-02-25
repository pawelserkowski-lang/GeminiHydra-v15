# Contributing to GeminiHydra

Part of the **Jaskier App Family** — a suite of AI-powered tools sharing code patterns and conventions.

## Prerequisites

- **Node.js 22+** with **pnpm** package manager
- **Rust 1.93+** (nightly, edition 2024)
- **PostgreSQL 17 with pgvector** (via Docker or local install)
- **Docker** (recommended for database)

## Quick Start

```bash
# 1. Clone
git clone https://github.com/EPS-AI-SOLUTIONS/GeminiHydra-v15.git
cd GeminiHydra-v15

# 2. Frontend
pnpm install
pnpm dev              # http://localhost:5176

# 3. Backend
cp backend/.env.example backend/.env   # Fill in API keys (GEMINI_API_KEY, GOOGLE_API_KEY)
cd backend
cargo run             # http://localhost:8081
```

## Development Workflow

### Pre-commit Checks

```bash
npx tsc --noEmit           # TypeScript
cd backend && cargo check  # Rust
cd backend && cargo test --lib  # Backend tests
pnpm test                  # Frontend tests
```

### Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

| Prefix | Usage |
|--------|-------|
| `feat:` | New feature |
| `fix:` | Bug fix |
| `chore:` | Maintenance, cleanup |
| `deps:` | Dependency updates |
| `docs:` | Documentation only |
| `test:` | Adding tests |
| `refactor:` | Code restructuring |

Example:
```bash
git commit -m "feat: add cross-session visibility with ClaudeHydra"
git commit -m "fix: pause heartbeat during WebSocket streaming"
```

### Code Style

- **TypeScript**: Follow existing patterns. Use `cn()` for Tailwind classes.
- **Rust**: `cargo clippy` clean. Use `thiserror` for error types.
- **i18n**: All user-facing strings via `useTranslation()` / `t()`.
- **Shared patterns**: Marked with `/** Jaskier Shared Pattern */` (TS) or `// Jaskier Shared Pattern` (Rust).

## Architecture

### Frontend
React 19 · Vite 7 · TypeScript 5.9 · Zustand 5 · TailwindCSS 4 · motion/react · lucide-react · i18next · sonner

### Backend
Rust · Axum 0.8 · SQLx · PostgreSQL 17 + pgvector · tower-http · tower_governor

### Key Directories

```
src/
├── components/atoms/      # Shared atomic components (Card, Button, Badge...)
├── components/organisms/  # Layout components (AppShell, Sidebar...)
├── features/             # Feature modules (chat/, agents/, each with components/)
├── shared/               # Cross-feature utilities, API client, types
├── stores/               # Zustand stores
├── i18n/                 # en.json + pl.json translations
└── styles/               # base.css + globals.css

backend/src/
├── main.rs              # Entry point, middleware stack
├── lib.rs               # Router definition
├── auth.rs              # Auth middleware (Jaskier Shared Pattern)
├── handlers.rs          # Route handlers (chat, models, tools, OAuth)
├── state.rs             # AppState struct
├── model_registry.rs    # Dynamic model cache (Google Gemini, Anthropic Claude)
└── migrations/          # SQLx PostgreSQL migrations
```

## Auth

Backend supports optional `AUTH_SECRET` env var:
- **Not set**: Dev mode (no authentication required)
- **Set**: All protected routes require `Authorization: Bearer <secret>` header

Frontend uses `VITE_AUTH_SECRET` env var to inject the header automatically.

## Environment Variables

### Backend `.env`

```env
# Database
DATABASE_URL=postgresql://postgres:password@localhost/geminihydra
GEMINI_API_KEY=your-gemini-api-key
GOOGLE_API_KEY=your-google-api-key
ANTHROPIC_API_KEY=your-anthropic-api-key

# OAuth (optional — Anthropic MAX Plan)
OAUTH_CLIENT_ID=your-client-id
OAUTH_CLIENT_SECRET=your-client-secret
OAUTH_REDIRECT_URI=http://localhost:8081/api/auth/callback

# Optional
AUTH_SECRET=your-secret-key
RUST_LOG=info
RUST_BACKTRACE=1
```

### Frontend `.env.local`

```env
VITE_BACKEND_URL=http://localhost:8081
VITE_AUTH_SECRET=your-secret-key
```

## Features

- **Multi-Agent Swarm** — Orchestrate agents with specialized roles using Gemini models
- **Chat Streaming** — Real-time responses via WebSocket
- **Tool Calling** — Local file system access (list, read, write, execute)
- **OAuth Integration** — Anthropic Claude MAX Plan for flat-rate API access
- **Cross-Session Visibility** — Read-only view of ClaudeHydra sessions
- **Semantic Search** — pgvector embeddings for intelligent retrieval
- **Dynamic Model Selection** — Auto-select best model per use case

## Deployment

- **Fly.io**:
  ```bash
  cd backend && fly deploy
  ```
- **Vercel**: Frontend auto-deploys from `master` branch
- **Database**: Fly.io Postgres cluster (`geminihydra_v15_backend` database)

## Notes

- **pgvector**: Available locally but NOT on fly.io. Migrations handle gracefully with DO/EXCEPTION blocks.
- **WebSocket Streaming**: Frontend heartbeat MUST pause during tool execution to avoid timeout.
- **Tool Calling**: Agents have local filesystem access (Windows cmd.exe, 30s timeout).
- **Migrations**: All migration files use unique date prefixes (SQLx sorts by prefix). MUST be idempotent. `.gitattributes` enforces LF line endings to prevent checksums breaking between Windows and Docker.

## Testing

```bash
# Frontend unit tests
pnpm test

# Frontend E2E tests
pnpm e2e
pnpm e2e:ui      # Interactive UI
pnpm e2e:headed  # Show browser
pnpm e2e:debug   # Debugger mode

# Backend unit tests
cd backend && cargo test --lib

# Backend integration tests (requires DB)
cd backend && cargo test
```

## Troubleshooting

**TypeScript errors after git pull?**
```bash
npx tsc --noEmit
```

**Cargo build fails?**
```bash
cd backend && cargo clean && cargo build
```

**WebSocket disconnects during streaming?**
- Ensure `isStreamingRef` pauses heartbeat in `useWebSocketChat.ts`
- Backend blocks on `execute_streaming().await` — 60s+ tool execution needs heartbeat paused
- See WebSocket Streaming section in CLAUDE.md

**Database migration errors?**
```bash
# Reset local DB (careful!)
cd backend && sqlx database drop && sqlx database create && sqlx migrate run
```

**Port conflicts?**
```bash
# Check which process uses port 5176 or 8081
netstat -ano | grep LISTENING
```

**pgvector errors on fly.io?**
- pgvector NOT available on production. Migrations use `DO/EXCEPTION` to handle gracefully.
- Semantic search queries fall back to text search in production.

## Questions?

See [CLAUDE.md](/CLAUDE.md) for Jaskier architecture docs or open an issue on GitHub.
