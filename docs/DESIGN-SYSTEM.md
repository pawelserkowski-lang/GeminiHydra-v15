# Design System -- Tissaia

GeminiHydra v15 uses the **Tissaia Design System**, a glass-morphism theme with two modes: a white/silver dark mode and a forest green light mode. All design tokens are defined as CSS custom properties in `src/styles/globals.css`.

---

## Color Palette

### Dark Mode (Default)

The dark theme uses a deep blue-black base with white/silver accents.

| Token                        | Value                       | Usage                        |
|------------------------------|-----------------------------|------------------------------|
| `--matrix-bg-primary`        | `#0f1419`                  | Page background              |
| `--matrix-bg-secondary`      | `#0a0e13`                  | Secondary surfaces           |
| `--matrix-bg-tertiary`       | `#141a21`                  | Elevated surfaces            |
| `--matrix-accent`            | `#ffffff`                  | Primary accent (white)       |
| `--matrix-accent-dim`        | `#d1d5db`                  | Muted accent (silver)        |
| `--matrix-accent-glow`       | `#e5e7eb`                  | Glow/highlight accent        |
| `--matrix-text-primary`      | `#e5e7eb`                  | Body text                    |
| `--matrix-text-secondary`    | `#9ca3af`                  | Muted/secondary text         |
| `--matrix-border`            | `#1e2a35`                  | Borders and dividers         |
| `--matrix-error`             | `#f87171`                  | Error state                  |
| `--matrix-warning`           | `#facc15`                  | Warning state                |
| `--matrix-success`           | `#4ade80`                  | Success state                |

### Light Mode (The White Wolf)

The light theme uses soft greens inspired by forest environments, activated via `.light` class or `[data-theme='light']`.

| Token                        | Value                       | Usage                        |
|------------------------------|-----------------------------|------------------------------|
| `--matrix-bg-primary`        | `#f5f8f5`                  | Page background              |
| `--matrix-bg-secondary`      | `#e8ede8`                  | Secondary surfaces           |
| `--matrix-accent`            | `#2d6a4f`                  | Primary accent (forest green)|
| `--matrix-accent-dim`        | `#1b4332`                  | Deep green accent            |
| `--matrix-accent-glow`       | `#40916c`                  | Light green glow             |
| `--matrix-text-primary`      | `#1a2a1a`                  | Body text                    |
| `--matrix-text-secondary`    | `#2d3d2d`                  | Muted text                   |
| `--matrix-border`            | `#d0e0d0`                  | Borders                      |
| `--matrix-error`             | `#dc2626`                  | Error state                  |
| `--matrix-warning`           | `#b45309`                  | Warning state                |
| `--matrix-success`           | `#047857`                  | Success state                |

### Witcher-Themed Accents

Used sparingly for decorative elements:

| Token                  | Value     | Usage                |
|------------------------|-----------|----------------------|
| `--color-witcher-silver` | `#c0c0c0` | Silver sword accents |
| `--color-witcher-gold`   | `#ffd700` | Gold/coin highlights |
| `--color-witcher-blood`  | `#8b0000` | Danger/blood accents |
| `--color-witcher-dark`   | `#1a1a2e` | Deep dark overlay    |
| `--color-witcher-steel`  | `#4a5568` | Steel sword accents  |

---

## Typography

| Token          | Value                                           | Usage           |
|----------------|-------------------------------------------------|-----------------|
| `--font-mono`  | `'JetBrains Mono', 'Fira Code', 'Consolas', monospace` | Primary font (body, code, UI) |
| `--font-sans`  | `'Inter', system-ui, -apple-system, sans-serif` | Fallback/headings |

The entire UI uses monospace as the primary font family, reinforcing the terminal/hacker aesthetic.

---

## Glass Morphism

The core visual treatment across the application. Three levels of glass panels are defined:

### Glass Panel (Standard)

```css
.glass-panel {
  background: rgba(15, 25, 40, 0.65);       /* dark mode */
  backdrop-filter: blur(20px);
  border-radius: 1rem;
  border: 1px solid rgba(255, 255, 255, 0.12);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.25);
}
```

Light mode variant:
```css
background: rgba(255, 255, 255, 0.8);
border-color: rgba(45, 106, 79, 0.15);
```

### Glass Card

Same blur treatment as `.glass-panel` but used for card-sized containers within views.

### Glass Input

```css
.glass-input {
  background: rgba(10, 20, 30, 0.6);
  backdrop-filter: blur(12px);
  border: 1px solid rgba(255, 255, 255, 0.15);
}
```

### Glass Button

```css
.glass-button {
  background: rgba(255, 255, 255, 0.1);
  backdrop-filter: blur(8px);
  border: 1px solid rgba(255, 255, 255, 0.2);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}
```

Hover state adds glow: `box-shadow: 0 0 20px rgba(255, 255, 255, 0.1)`.

### Physics Constants

| Token              | Value                            |
|--------------------|----------------------------------|
| `--glass-blur`     | `blur(16px) saturate(180%)`     |
| `--glass-radius`   | `12px`                          |

---

## Shadows

| Token                    | Value                                    | Usage           |
|--------------------------|------------------------------------------|-----------------|
| `--shadow-matrix-glow`   | `0 0 20px rgba(255, 255, 255, 0.15)`    | Large glow      |
| `--shadow-matrix-glow-sm`| `0 0 10px rgba(255, 255, 255, 0.1)`     | Small glow      |
| `--shadow-glass`         | `0 8px 32px rgba(0, 0, 0, 0.3)`         | Glass elevation  |

---

## Message Bubbles

Three bubble styles for chat messages, each with glass treatment:

| Class                       | Background (dark)                  | Color    |
|-----------------------------|------------------------------------|----------|
| `.message-bubble-user`      | `rgba(255, 255, 255, 0.85)`      | `#1a1a1a`|
| `.message-bubble-assistant` | `rgba(200, 200, 210, 0.75)`      | `#1a1a1a`|
| `.message-bubble-system`    | `rgba(82, 82, 91, 0.8)`          | `#ffffff`|

All bubbles use `backdrop-filter: blur(16px) saturate(180%)`.

---

## Animations

| Name             | Duration | Effect                              |
|------------------|----------|-------------------------------------|
| `glow`           | 2s       | Pulsing box-shadow glow             |
| `shimmer`        | --       | translateX sweep (skeleton loading) |
| `pulse-slow`     | 3s       | Slow opacity pulse                  |
| `welcomeFadeIn`  | 0.4s     | Fade-in + slide-up for welcome      |

All animations honor `prefers-reduced-motion: reduce` by collapsing to 0.01ms duration.

---

## Scrollbar

Custom scrollbar styling:
- **Dark mode**: white gradient thumb on transparent track
- **Light mode**: green gradient thumb on transparent track
- Width: 6px
- Border radius: 10px

---

## Accessibility

- **Focus-visible outlines**: 2px solid `var(--matrix-accent)` with 2px offset on all interactive elements
- **Skip link**: `.skip-link` hidden off-screen, revealed on focus for keyboard navigation
- **Selection colors**: dark mode uses white bg/black text, light mode uses mint green bg/dark green text
- **User selection control**: UI chrome is `user-select: none`; content areas are `user-select: text`
