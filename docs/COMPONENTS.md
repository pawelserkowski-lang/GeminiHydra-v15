# Component Library

GeminiHydra v15 follows **Atomic Design** methodology. Components are organized into four layers: atoms, molecules, organisms, and features.

All components live under `src/components/` (atoms, molecules, organisms) and `src/features/` (feature views).

---

## Atoms (`src/components/atoms/`)

The smallest, reusable UI primitives. No business logic.

### Button (`Button.tsx`)

General-purpose button with variant support via `class-variance-authority`.

| Prop       | Type                        | Default     | Description            |
|------------|-----------------------------|-------------|------------------------|
| `variant`  | `"default" \| "ghost" \| "danger"` | `"default"` | Visual style    |
| `size`     | `"sm" \| "md" \| "lg"`     | `"md"`      | Size preset            |
| `disabled` | `boolean`                   | `false`     | Disabled state         |
| `children` | `ReactNode`                 | --          | Button content         |

### Card (`Card.tsx`)

Glass-morphism card container. Applies `.glass-card` styles with backdrop blur.

| Prop       | Type        | Description             |
|------------|-------------|-------------------------|
| `children` | `ReactNode` | Card content            |
| `className`| `string`    | Additional CSS classes  |

### Badge (`Badge.tsx`)

Status/label badge with color variants.

| Prop      | Type                                    | Description      |
|-----------|-----------------------------------------|------------------|
| `variant` | `"default" \| "success" \| "warning" \| "error"` | Color theme |
| `children`| `ReactNode`                             | Badge label      |

### Input (`Input.tsx`)

Styled text input with `.matrix-input` glass treatment.

| Prop          | Type     | Description              |
|---------------|----------|--------------------------|
| `placeholder` | `string` | Placeholder text         |
| `value`       | `string` | Controlled value         |
| `onChange`    | `func`   | Change handler           |

### ProgressBar (`ProgressBar.tsx`)

Animated progress bar using `.progress-bar` and `.progress-bar-fill` CSS.

| Prop    | Type     | Description                    |
|---------|----------|--------------------------------|
| `value` | `number` | Progress percentage (0--100)   |

### Skeleton (`Skeleton.tsx`)

Loading placeholder with shimmer animation.

| Prop        | Type     | Description               |
|-------------|----------|---------------------------|
| `width`     | `string` | CSS width                 |
| `height`    | `string` | CSS height                |
| `className` | `string` | Additional CSS classes    |

### LayeredBackground (`LayeredBackground.tsx`)

Full-viewport layered background with gradient and optional matrix rain pattern. Renders behind all other content.

### WitcherRunes (`WitcherRunes.tsx`)

Decorative SVG rune elements for Witcher-themed visual accents.

---

## Molecules (`src/components/molecules/`)

Composed from atoms. Introduce minor interaction patterns.

### CodeBlock (`CodeBlock.tsx`)

Syntax-highlighted code display with copy-to-clipboard functionality. Wraps `<pre><code>` with `.markdown-body` styles.

| Prop       | Type     | Description            |
|------------|----------|------------------------|
| `code`     | `string` | Source code content    |
| `language` | `string` | Language for highlight |

### ModelSelector (`ModelSelector.tsx`)

Dropdown selector for choosing Gemini models. Fetches available models from `GET /api/gemini/models`.

| Prop       | Type     | Description            |
|------------|----------|------------------------|
| `value`    | `string` | Currently selected model |
| `onChange` | `func`   | Selection handler      |

### StatusIndicator (`StatusIndicator.tsx`)

Dot + label component for displaying connection/agent status (online, offline, error).

| Prop     | Type                                  | Description   |
|----------|---------------------------------------|---------------|
| `status` | `"online" \| "offline" \| "error"`    | Current state |
| `label`  | `string`                              | Display text  |

### ViewSkeleton (`ViewSkeleton.tsx`)

Full-view loading skeleton combining multiple `Skeleton` atoms in a layout pattern. Used as the loading state for feature views.

### DataSkeleton (`DataSkeleton.tsx`)

Table/list-shaped loading skeleton for data-heavy views (agents list, history).

---

## Organisms (`src/components/organisms/`)

Complex UI sections composed of molecules and atoms.

### AppShell (`AppShell.tsx`)

Root layout component. Renders the sidebar, tab bar, main content area, and status footer in a flex layout.

```
+------------------+------------------------------------------+
|                  |  TabBar                                  |
|    Sidebar       +------------------------------------------+
|                  |  Main Content (feature views)            |
|                  |                                          |
+------------------+------------------------------------------+
|  StatusFooter                                               |
+-------------------------------------------------------------+
```

### Sidebar (`Sidebar.tsx`)

Collapsible navigation sidebar. Contains:
- Logo/brand header
- Navigation items (Home, Chat, Agents, History, Settings, Status)
- Session list with create/delete actions
- Collapse toggle

State controlled by `viewStore.sidebarCollapsed`.

### TabBar (`TabBar.tsx`)

Horizontal tab strip for open chat sessions. Supports:
- Tab switching (`switchTab`)
- Tab closing (respects pin state)
- Tab reordering via drag
- Pin indicator

### StatusFooter (`StatusFooter.tsx`)

Persistent bottom bar showing:
- Backend connection status
- Active model name
- Uptime
- CPU/memory stats from `/api/system/stats`

### ErrorBoundary (`ErrorBoundary.tsx`)

React error boundary wrapping the main content area. Catches render errors and displays a recovery UI.

---

## Features (`src/features/`)

Full-page views wired to the Zustand store and backend API.

### WelcomeScreen / Home (`home/`)

Landing page displayed when `currentView === "home"`. Shows:
- GeminiHydra logo and version
- Backend status indicator (online/offline)
- Quick-action cards (new chat, view agents, etc.)
- Keyboard shortcut hints

Uses `.welcome-*` CSS classes from `globals.css`.

### ChatContainer (`chat/`)

The primary chat interface displayed when `currentView === "chat"`. Contains:
- Message list with user/assistant/system bubble styles
- Markdown rendering via `react-markdown` + `remark-gfm` + `rehype-highlight`
- Chat input with model selector
- Agent attribution on responses
- Streaming support via `updateLastMessage`

### AgentsView (`agents/`)

Agent roster displayed when `currentView === "agents"`. Shows all 12 Witcher agents in card layout with:
- Agent name, role, tier, status
- Description text
- Status indicator (online/offline)

Data fetched from `GET /api/agents`.

### HistoryView (`history/`)

Chat history browser displayed when `currentView === "history"`. Supports:
- Paginated message list from `GET /api/history`
- Full-text search via `GET /api/history/search`
- Clear history action

### SettingsView (`settings/`)

Application settings panel. Reads from `GET /api/settings` and patches via `PATCH /api/settings`. Controls:
- Temperature slider
- Max tokens
- Default model
- Language
- Theme (dark/light)
- Reset to defaults

### StatusView (`status/`)

System dashboard displaying real-time stats from `GET /api/system/stats` and `GET /api/health/detailed`:
- CPU usage
- Memory usage
- Platform info
- Provider availability
- Uptime
