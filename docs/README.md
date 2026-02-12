# GeminiHydra v15.0.0 -- Multi-Agent AI Swarm

A 12-agent AI swarm system themed around The Witcher universe. Each agent specializes in a distinct domain and is automatically selected via keyword-based prompt classification. The swarm routes user prompts through a Rust/Axum backend to the Google Gemini API and returns structured, agent-attributed responses.

## Quick Start

### Prerequisites

- **Node.js** >= 20 and **pnpm** >= 9
- **Rust** >= 1.82 (2024 edition)
- A **Google Gemini API key** (`GOOGLE_API_KEY` or `GEMINI_API_KEY`)
- Optional: `ANTHROPIC_API_KEY` for Claude provider availability

### 1. Clone and install

```bash
git clone <repo-url> GeminiHydra-v15
cd GeminiHydra-v15
pnpm install
```

### 2. Configure environment

Create a `.env` file in `backend/`:

```env
GOOGLE_API_KEY=your-gemini-key-here
ANTHROPIC_API_KEY=your-anthropic-key-here   # optional
PORT=8081                                    # optional, default 8081
```

### 3. Start the backend (Rust/Axum)

```bash
cd backend
cargo run
```

The backend listens on **http://localhost:8081**.

### 4. Start the frontend (React/Vite)

```bash
pnpm dev
```

The frontend is served at **http://localhost:5176** and proxies `/api` requests to the backend.

### 5. Run tests

```bash
pnpm test           # 66 Vitest tests (frontend)
cd backend && cargo test   # 14 integration tests (backend)
```

## The 12 Witcher Agents

| Agent        | Role                         | Tier        | Description                                                  |
|-------------|------------------------------|-------------|--------------------------------------------------------------|
| **Geralt**   | Security & Protection        | Commander   | The White Wolf -- threat analysis and protective measures     |
| **Yennefer** | Architecture & Design        | Commander   | The Sorceress -- system architecture and design patterns      |
| **Vesemir**  | Testing & Quality            | Commander   | The Elder Witcher -- QA strategy and code reviews             |
| **Triss**    | Data & Analytics             | Coordinator | The Merigold -- data pipelines and insight extraction          |
| **Jaskier**  | Documentation & Communication| Coordinator | The Bard -- documentation and knowledge sharing               |
| **Ciri**     | Performance & Optimization   | Coordinator | The Lion Cub -- profiling, optimization, benchmarking         |
| **Dijkstra** | Strategy & Planning          | Coordinator | The Spymaster -- roadmaps, prioritization, project strategy   |
| **Lambert**  | DevOps & Infrastructure      | Executor    | The Hothead -- CI/CD, Docker, infrastructure management       |
| **Eskel**    | Backend & APIs               | Executor    | The Reliable -- REST APIs and server-side logic               |
| **Regis**    | Research & Knowledge         | Executor    | The Higher Vampire -- research and deep analysis              |
| **Zoltan**   | Frontend & UI                | Executor    | The Dwarf -- UI components and user experiences               |
| **Philippa** | Security & Monitoring        | Executor    | The Owl -- security audits, monitoring, incident response     |

Agents are organized into three tiers: **Commander** (strategic leads), **Coordinator** (domain coordinators), and **Executor** (task-level specialists).

## Tech Stack

| Layer     | Technology                                                    |
|-----------|---------------------------------------------------------------|
| Frontend  | React 19, Vite 7, TypeScript 5.9, Tailwind CSS 4, Zustand 5, TanStack Query 5 |
| Backend   | Rust (2024 edition), Axum 0.8, Tokio, Reqwest, Serde         |
| AI        | Google Gemini API (gemini-2.0-flash default), Anthropic Claude (optional) |
| Testing   | Vitest 4 (frontend), Cargo test (backend), Playwright (e2e stubs) |
| Tooling   | Biome (lint/format), pnpm, dotenvy                            |

## Project Structure

```
GeminiHydra-v15/
  backend/              # Rust/Axum backend
    src/
      main.rs           # Server entrypoint (port 8081)
      lib.rs            # Router factory
      handlers.rs       # Core API handlers
      sessions.rs       # History, settings, memory, knowledge graph
      models.rs         # Shared types
      state.rs          # AppState with 12 agents
    tests/
      api_tests.rs      # 14 integration tests
  src/                  # React frontend
    components/
      atoms/            # Button, Card, Badge, Input, ProgressBar, Skeleton, LayeredBackground, WitcherRunes
      molecules/        # CodeBlock, ModelSelector, StatusIndicator, ViewSkeleton, DataSkeleton
      organisms/        # AppShell, Sidebar, TabBar, StatusFooter, ErrorBoundary
    features/           # home, chat, agents, history, settings, status, health
    stores/             # Zustand viewStore (sessions, tabs, chat history)
    styles/             # globals.css (Tissaia Design System)
  e2e/                  # Playwright e2e stubs
  docs/                 # This documentation
```
