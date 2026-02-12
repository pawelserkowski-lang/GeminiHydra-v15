import { memo } from 'react';

/**
 * LayeredBackground Component
 * 3-layer composited background from GeminiHydra legacy.
 *
 * Layer 1: Background image (bg.webp / backgroundlight.webp)
 *          - Dark: opacity-60, Light: opacity-50
 * Layer 2: Gradient overlay (reduced for more bg visibility)
 *          - Dark: from-matrix-bg-primary/30 via-matrix-bg-primary/15 to-matrix-bg-secondary/35
 *          - Light: from-white/30 via-white/15 to-slate-100/40
 * Layer 3: Radial vignette (softened)
 *          - Dark: from-transparent via-black/5 to-black/25
 *          - Light: from-transparent via-white/5 to-white/20
 *
 * Ported pixel-perfect from GeminiHydra legacy App.tsx.
 */

interface LayeredBackgroundProps {
  resolvedTheme: 'dark' | 'light';
}

export const LayeredBackground = memo(({ resolvedTheme }: LayeredBackgroundProps) => {
  const isLight = resolvedTheme === 'light';

  return (
    <>
      {/* Background Layer 1 - Image (higher opacity for visibility) */}
      <div
        className={`absolute inset-0 z-[1] bg-cover bg-center bg-no-repeat transition-opacity duration-1000 pointer-events-none ${
          isLight ? "bg-[url('/backgroundlight.webp')] opacity-50" : "bg-[url('/background.webp')] opacity-60"
        }`}
      />
      {/* Background Layer 2 - Gradient overlay (reduced for more bg visibility) */}
      <div
        className={`absolute inset-0 z-[2] pointer-events-none transition-opacity duration-1000 ${
          isLight
            ? 'bg-gradient-to-b from-white/30 via-white/15 to-slate-100/40'
            : 'bg-gradient-to-b from-matrix-bg-primary/30 via-matrix-bg-primary/15 to-matrix-bg-secondary/35'
        }`}
      />
      {/* Background Layer 3 - Radial vignette (softened) */}
      <div
        className={`absolute inset-0 z-[2] pointer-events-none bg-[radial-gradient(ellipse_at_center,_var(--tw-gradient-stops))] ${
          isLight ? 'from-transparent via-white/5 to-white/20' : 'from-transparent via-black/5 to-black/25'
        }`}
      />
    </>
  );
});

LayeredBackground.displayName = 'LayeredBackground';
