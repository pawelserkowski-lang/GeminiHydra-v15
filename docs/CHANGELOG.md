# Changelog

All notable changes to GeminiHydra are documented in this file.

---

## [15.0.0] -- 2025

### Major Rewrite

This release is a ground-up rewrite of the GeminiHydra platform. Every layer of the stack was replaced to improve performance, maintainability, and deployment simplicity.

### Changed

- **Backend: TypeScript/Fastify replaced with Rust/Axum**
  - Complete port from Node.js/Fastify to Rust with Axum 0.8 and Tokio async runtime
  - All API endpoints reimplemented: health, agents, classify, execute, models, stats, history, settings, memory, knowledge graph
  - Type-safe request/response models via Serde derive macros
  - In-process integration tests using `tower::ServiceExt::oneshot` (no port binding needed)
  - System stats via `sysinfo` crate (CPU, memory, platform)
  - Structured logging via `tracing` + `tracing-subscriber` with env-filter support

- **Desktop app: Tauri removed in favor of pure web**
  - Eliminated Tauri dependency and native window management
  - Application is now a standard SPA served by any web server
  - Vite dev server proxies `/api` requests to the Rust backend
  - Simpler deployment: static files + single binary

- **AI provider: llama.cpp replaced with Gemini API proxy**
  - Removed local llama.cpp model inference
  - Backend now proxies requests to Google Gemini API (`generativelanguage.googleapis.com`)
  - Dynamic model listing via `GET /api/gemini/models`
  - Default model: `gemini-2.0-flash`
  - Optional Anthropic Claude provider detection (key presence only, not yet used for inference)

### Added

- **12 Witcher Agent swarm** with keyword-based prompt classification
  - Geralt (Security), Yennefer (Architecture), Vesemir (Testing), Triss (Data), Jaskier (Docs), Ciri (Performance), Dijkstra (Strategy), Lambert (DevOps), Eskel (Backend), Regis (Research), Zoltan (Frontend), Philippa (Monitoring)
  - Three-tier hierarchy: Commander, Coordinator, Executor
  - Agent classification returns agent ID, confidence score, and reasoning
- **Session and tab management** in Zustand 5 with persist middleware
  - Max 50 sessions, max 500 messages per session
  - Tab pinning, reordering, deduplication
  - Auto-titling from first user message
- **Memory system** with per-agent memories ranked by importance
- **Knowledge graph** with nodes and edges API
- **Tissaia Design System** -- glass-morphism theme with dark (white/silver) and light (forest green) modes
- **Atomic component library** -- atoms, molecules, organisms following atomic design
- **Internationalization** via i18next + react-i18next
- **Accessibility**: focus-visible outlines, skip link, reduced-motion support, semantic selection control
- **66 frontend tests** (Vitest) covering utility functions, Zustand store, and edge cases
- **14 backend integration tests** (Cargo) covering all core endpoints
- **Playwright e2e stubs** for chat flow scenarios

### Tech Stack (v15)

| Layer    | v14 (Previous)               | v15 (Current)                        |
|----------|------------------------------|--------------------------------------|
| Backend  | TypeScript + Fastify         | Rust + Axum 0.8                      |
| Runtime  | Node.js                      | Tokio (native async)                 |
| Desktop  | Tauri                        | Web SPA (no native shell)            |
| AI       | llama.cpp (local)            | Google Gemini API (cloud)            |
| Frontend | React + Vite                 | React 19 + Vite 7 + TypeScript 5.9  |
| State    | Zustand                      | Zustand 5 + TanStack Query 5        |
| CSS      | Tailwind                     | Tailwind CSS 4 (v4 theme config)     |
| Lint     | ESLint                       | Biome 2                              |
