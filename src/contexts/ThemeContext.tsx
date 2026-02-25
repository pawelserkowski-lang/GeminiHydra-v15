// src/contexts/ThemeContext.tsx
/**
 * GeminiHydra v15 Theme Context
 * =============================
 * Provides theme switching (dark/light/system) across the app.
 * Persists selection to localStorage.
 * Updates document data-theme attribute and meta theme-color tag.
 */
import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  useSyncExternalStore,
} from 'react';

// ============================================
// TYPES
// ============================================

export type Theme = 'dark' | 'light' | 'system';
export type ResolvedTheme = 'dark' | 'light';

// ============================================
// CONTEXT TYPE
// ============================================

interface ThemeContextType {
  /** Current theme mode (dark | light | system) */
  theme: Theme;
  /** Update theme mode and persist to localStorage */
  setTheme: (theme: Theme) => void;
  /** Toggle between dark and light (ignores system) */
  toggleTheme: () => void;
  /** Resolved theme after evaluating system preference */
  resolvedTheme: ResolvedTheme;
}

const ThemeContext = createContext<ThemeContextType | undefined>(undefined);

// ============================================
// CONSTANTS
// ============================================

const META_THEME_COLORS: Record<ResolvedTheme, string> = {
  dark: '#0a0f0d',
  light: '#ffffff',
};

// ============================================
// PROVIDER
// ============================================

interface ThemeProviderProps {
  children: ReactNode;
  defaultTheme?: Theme;
  storageKey?: string;
}

export function ThemeProvider({ children, defaultTheme = 'dark', storageKey = 'geminihydra-theme' }: ThemeProviderProps) {
  const [theme, setThemeState] = useState<Theme>(() => {
    if (typeof window === 'undefined') return defaultTheme;
    return (localStorage.getItem(storageKey) as Theme) || defaultTheme;
  });

  // Use useSyncExternalStore for system preference to avoid setState in effect
  const systemPrefersDark = useSyncExternalStore(
    (callback) => {
      const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
      mediaQuery.addEventListener('change', callback);
      return () => mediaQuery.removeEventListener('change', callback);
    },
    () => window.matchMedia('(prefers-color-scheme: dark)').matches,
    () => true, // Server snapshot defaults to dark
  );

  // Compute resolved theme without extra state
  const resolvedTheme = useMemo<ResolvedTheme>(() => {
    if (theme === 'system') {
      return systemPrefersDark ? 'dark' : 'light';
    }
    return theme;
  }, [theme, systemPrefersDark]);

  // Apply theme to document (side effect only, no setState)
  useEffect(() => {
    const root = window.document.documentElement;

    // Update data-theme attribute
    root.setAttribute('data-theme', resolvedTheme);

    // Also maintain class for compatibility with Tailwind dark: prefix
    root.classList.remove('light', 'dark');
    root.classList.add(resolvedTheme);

    // Update meta theme-color
    let metaThemeColor = document.querySelector('meta[name="theme-color"]');
    if (!metaThemeColor) {
      metaThemeColor = document.createElement('meta');
      metaThemeColor.setAttribute('name', 'theme-color');
      document.head.appendChild(metaThemeColor);
    }
    metaThemeColor.setAttribute('content', META_THEME_COLORS[resolvedTheme]);
  }, [resolvedTheme]);

  const setTheme = useCallback(
    (newTheme: Theme) => {
      localStorage.setItem(storageKey, newTheme);
      setThemeState(newTheme);
    },
    [storageKey],
  );

  const toggleTheme = useCallback(() => {
    const next: Theme = resolvedTheme === 'dark' ? 'light' : 'dark';
    setTheme(next);
  }, [resolvedTheme, setTheme]);

  const value = useMemo<ThemeContextType>(
    () => ({ theme, setTheme, toggleTheme, resolvedTheme }),
    [theme, setTheme, toggleTheme, resolvedTheme],
  );

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}

// ============================================
// HOOK
// ============================================

export function useTheme(): ThemeContextType {
  const context = useContext(ThemeContext);

  if (context === undefined) {
    throw new Error('useTheme must be used within a ThemeProvider');
  }

  return context;
}
