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

## Conventions
- motion/react (NOT framer-motion) for animations
- Biome for linting (not ESLint)
- npm (not pnpm) as package manager
