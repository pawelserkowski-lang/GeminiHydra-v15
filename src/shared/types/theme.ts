// src/shared/types/theme.ts
/**
 * Theme mode selection.
 * - dark: Matrix Glass dark theme (default)
 * - light: White Wolf light theme
 * - system: Follow OS preference
 */
export type Theme = 'dark' | 'light' | 'system';

/** Resolved theme after system preference evaluation. */
export type ResolvedTheme = 'dark' | 'light';
