# Testing

## Overview

| Layer    | Framework      | Test Count | Command              |
|----------|----------------|------------|----------------------|
| Frontend | Vitest 4       | 66 tests   | `pnpm test`         |
| Backend  | Cargo test     | 14 tests   | `cd backend && cargo test` |
| E2E      | Playwright     | 4 stubs    | `npx playwright test`|

---

## Frontend Tests (Vitest)

### Configuration

Vitest is configured in `vitest.config.ts`:

- **Environment**: `jsdom` (browser-like DOM via jsdom 28)
- **Globals**: enabled (`describe`, `it`, `expect` available without import)
- **Setup file**: `src/test/setup.ts`
- **Pattern**: `src/**/*.test.ts` and `src/**/*.test.tsx`
- **Path alias**: `@` maps to `src/`

### Running

```bash
# Run all tests
pnpm test

# Watch mode
pnpm vitest

# Run specific file
pnpm vitest src/stores/__tests__/viewStore.test.ts

# With coverage
pnpm vitest --coverage
```

### Test Suites

#### `cn` utility tests (`src/shared/utils/__tests__/cn.test.ts`) -- 18 tests

Tests the `cn()` class name merge utility (clsx + tailwind-merge):

- Basic string merging
- Falsy value filtering
- Conditional object syntax (`{ active: true }`)
- Array syntax
- Mixed input types
- Tailwind conflict resolution (padding, margin, text color, background, font size, display, border-radius)
- Non-conflicting class preservation
- Real-world component variant patterns
- Conditional disabled state pattern

#### `viewStore` tests (`src/stores/__tests__/viewStore.test.ts`) -- 47 tests

Comprehensive Zustand store tests covering:

**Initial state** (1 test)
- Default values for all store fields

**View actions** (5 tests)
- `setCurrentView` for all valid views (home, chat, agents, history, settings, status)
- `toggleSidebar` and `setSidebarCollapsed`

**Session CRUD** (10 tests)
- `createSession` -- creates and sets as current, prepends (newest first), initializes chat history
- `deleteSession` -- removes session and history, selects next session, handles last session, closes linked tabs
- `selectSession` -- selects existing, ignores non-existent
- `updateSessionTitle` -- updates title, truncates at 100 chars, defaults empty to "New Chat", updates matching tab

**Session limits** (2 tests)
- Max 50 sessions enforced, oldest evicted
- Chat history cleaned for evicted sessions

**Tab management** (12 tests)
- `openTab` -- creates tab, reuses existing for same session, sets currentSessionId
- `closeTab` -- closes unpinned, skips pinned, activates next tab, ignores non-existent
- `switchTab` -- sets activeTabId and currentSessionId, sets view to chat, ignores non-existent
- `reorderTabs` -- swaps positions, ignores out-of-bounds
- `togglePinTab` -- toggles pin state

**Message actions** (8 tests)
- `addMessage` -- adds to current session, no-op without session, preserves model field
- `updateLastMessage` -- appends content, no-op without messages or session
- `clearHistory` -- clears current session only, does not affect others

**Auto-titling** (5 tests)
- Sets title from first user message
- Truncates to 30 chars + ellipsis
- Does not change on subsequent messages
- Does not auto-title on system messages
- Updates matching tab title

**Message limits** (1 test)
- Max 500 messages per session, oldest trimmed

#### localStorage test (`__test_ls.test.ts`) -- 1 test

Verifies jsdom localStorage availability for Zustand persist.

---

## Backend Tests (Cargo)

### Configuration

Integration tests in `backend/tests/api_tests.rs` use:

- `tower::ServiceExt::oneshot` for in-process request handling (no network port binding)
- Fresh `AppState` per test via helper `fn app()`
- `http-body-util` for response body collection

### Running

```bash
cd backend

# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test health_returns_200
```

### Test Cases (14 tests)

**Health endpoint** (2 tests)
- `health_returns_200` -- GET /api/health returns 200
- `health_has_correct_fields` -- Response contains `status`, `version`, `app`, `uptime_seconds`, `providers`

**Agents endpoint** (3 tests)
- `agents_returns_200` -- GET /api/agents returns 200
- `agents_returns_12_agents` -- Response contains exactly 12 agents
- `agents_have_required_fields` -- Each agent has `id`, `name`, `role`, `tier`, `status`

**Settings endpoint** (5 tests)
- `get_settings_returns_200` -- GET /api/settings returns 200
- `get_settings_default_values` -- Defaults: language=en, theme=dark, model=gemini-2.0-flash
- `patch_settings_partial_update` -- PATCH /api/settings updates only provided fields
- `patch_settings_persists_changes` -- Changes persist in shared state
- `reset_settings_returns_200` -- POST /api/settings/reset returns 200
- `reset_settings_restores_defaults` -- Reset reverts all fields to defaults

**History endpoint** (2 tests)
- `history_returns_200` -- GET /api/history returns 200 with empty messages array
- `clear_history_returns_200` -- DELETE /api/history returns `{ cleared: true }`

**Error handling** (1 test)
- `unknown_route_returns_404` -- Unknown routes return 404

---

## E2E Tests (Playwright)

### Configuration

Playwright is configured in `playwright.config.ts`. Tests are in `e2e/`.

### Status

E2E tests are currently **stubs** that outline the intended scenarios. They are structured but rely on the full app being running.

### Test Cases (`e2e/chat-flow.spec.ts`) -- 4 stubs

- `should load the home view by default` -- Navigates to `/`, verifies body is visible
- `should create a new chat session` -- Clicks "New Chat" button
- `should send a message and see a response area` -- Fills input, clicks send, verifies message appears
- `should preserve session across page navigation` -- Creates session, reloads, checks persistence

### Running

```bash
# Install browsers (first time)
npx playwright install

# Run e2e tests (requires both frontend and backend running)
npx playwright test

# Run with UI
npx playwright test --ui

# Run headed
npx playwright test --headed
```

---

## Writing New Tests

### Frontend

1. Create `*.test.ts` or `*.test.tsx` file alongside the source (or in a `__tests__/` directory)
2. Use `describe`/`it`/`expect` (globally available)
3. For component tests, use `@testing-library/react` and `@testing-library/user-event`
4. Reset store state in `beforeEach` when testing Zustand stores

### Backend

1. Add `#[tokio::test]` functions to `backend/tests/api_tests.rs` (or create new test files)
2. Use the `app()` helper for a fresh router instance
3. Use `tower::ServiceExt::oneshot` to send requests without binding a port
4. Use `body_json()` helper to parse response bodies
