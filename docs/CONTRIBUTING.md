# Contributing

## Code Style

### Frontend (TypeScript/React)

- **Linter/Formatter**: Biome 2 (replaces ESLint + Prettier)
  ```bash
  pnpm lint          # Check
  pnpm lint:fix      # Auto-fix
  pnpm format        # Format
  ```
- **TypeScript**: Strict mode enabled. No `any` types. Use Zod for runtime validation.
- **Imports**: Use `@/` path alias for all project imports (maps to `src/`).
- **Components**: Follow atomic design -- place new components in `atoms/`, `molecules/`, or `organisms/` under `src/components/`. Feature-level views go in `src/features/`.
- **Naming**:
  - Components: `PascalCase.tsx`
  - Utilities/hooks: `camelCase.ts`
  - Test files: `*.test.ts` or `*.test.tsx` in `__tests__/` directories
- **Styling**: Use Tailwind CSS 4 utility classes. Use the `cn()` utility for conditional/merged classes. Avoid inline styles. Theme-specific styles go in `globals.css` with CSS custom properties.
- **State**: Use Zustand stores for global state. Use TanStack Query for server state. Avoid `useState` for data that could be derived.

### Backend (Rust)

- **Formatter**: `cargo fmt` (rustfmt defaults)
- **Linter**: `cargo clippy`
  ```bash
  cd backend
  cargo fmt
  cargo clippy -- -D warnings
  ```
- **Edition**: Rust 2024
- **Error handling**: Use `anyhow` for application errors, `thiserror` for library errors. Never `unwrap()` in handler code -- return proper error responses.
- **Async safety**: Always drop `Mutex` guards before `.await` points. Extract needed data from locked state, drop the guard, then perform async operations.
- **Naming**: Follow Rust conventions -- `snake_case` for functions/variables, `PascalCase` for types, `SCREAMING_SNAKE_CASE` for constants.
- **Tests**: Add integration tests to `backend/tests/api_tests.rs`. Use `tower::ServiceExt::oneshot` for in-process testing.

---

## Pull Request Rules

1. **Branch naming**: `feature/description`, `fix/description`, or `refactor/description`
2. **Commits**: Use conventional commit messages:
   - `feat: add new agent memory endpoint`
   - `fix: resolve session deletion race condition`
   - `refactor: extract classification logic to separate module`
   - `test: add viewStore tab management tests`
   - `docs: update API reference for memory endpoints`
3. **Tests required**: All PRs must pass existing tests. New features require new tests.
   - Frontend: `pnpm test`
   - Backend: `cd backend && cargo test`
4. **Lint clean**: No lint warnings. Run `pnpm lint` and `cargo clippy` before submitting.
5. **One concern per PR**: Keep PRs focused. Split large changes into multiple PRs.
6. **Description**: Explain what changed and why. Include screenshots for UI changes.
7. **Review**: At least one approval required before merge.

---

## Witcher Agent Conventions

When adding or modifying agents, follow these rules:

### Agent Definition

Each agent is defined in `backend/src/state.rs` as a `WitcherAgent` struct:

```rust
WitcherAgent {
    id: "agent_id".into(),       // lowercase, single word
    name: "AgentName".into(),    // display name, PascalCase
    role: "Domain & Specialty".into(),
    tier: "Commander|Coordinator|Executor".into(),
    status: "online".into(),     // always "online" at startup
    description: "The [Title] -- one-sentence role description.".into(),
}
```

### Naming Rules

- **id**: Lowercase Witcher character name (e.g., `geralt`, `yennefer`)
- **name**: Proper-cased character name
- **description**: Must start with `"The [Witcher Title] -- "` followed by a concise role description
- **tier**: One of `Commander`, `Coordinator`, or `Executor`

### Classification Rules

Agent classification keywords are defined in `backend/src/handlers.rs::classify_prompt()`. When adding a new agent:

1. Add keyword patterns to the `rules` array in `classify_prompt()`
2. Place more specific patterns **before** more general ones (first match wins)
3. Target a confidence score of `0.85` for keyword matches
4. Provide clear `reasoning` strings that explain why the agent was selected

```rust
(&["keyword1", "keyword2", "keyword3"], "agent_id", "Reasoning text"),
```

### Frontend Integration

When adding an agent to the frontend:

1. The agent list is fetched dynamically from `GET /api/agents` -- no frontend changes needed for new agents
2. If adding agent-specific UI (icons, colors), update the relevant feature component in `src/features/agents/`
3. Agent status is displayed via the `StatusIndicator` molecule

### Tier Guidelines

| Tier        | Count Target | Responsibility                         |
|-------------|-------------|----------------------------------------|
| Commander   | 2--4        | High-level domain strategy             |
| Coordinator | 3--5        | Cross-cutting coordination             |
| Executor    | 4--6        | Focused task execution                 |

Keep the total agent count manageable (12 is the current sweet spot). Adding agents increases classification ambiguity.

---

## Development Workflow

```bash
# 1. Fork and clone
git clone <your-fork>
cd GeminiHydra-v15

# 2. Install dependencies
pnpm install

# 3. Create feature branch
git checkout -b feature/my-change

# 4. Start backend
cd backend && cargo run &

# 5. Start frontend
pnpm dev

# 6. Make changes and test
pnpm test
cd backend && cargo test

# 7. Lint
pnpm lint
cd backend && cargo clippy

# 8. Commit and push
git add -A
git commit -m "feat: describe your change"
git push origin feature/my-change

# 9. Open PR
```
