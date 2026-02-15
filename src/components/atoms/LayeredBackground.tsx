import { memo } from 'react';

/**
 * LayeredBackground Component
 * 4-layer composited background with smooth crossfade between themes.
 *
 * Layer 1a: Dark background image (/background.webp) — always mounted
 * Layer 1b: Light background image (/backgroundlight.webp) — always mounted
 *           Both layers use CSS opacity transition for smooth crossfade.
 * Layer 2:  Gradient overlay (theme-aware, crossfades)
 * Layer 3:  Radial vignette (theme-aware, crossfades)
 */

interface LayeredBackgroundProps {
  resolvedTheme: 'dark' | 'light';
}

export const LayeredBackground = memo(({ resolvedTheme }: LayeredBackgroundProps) => {
  const isLight = resolvedTheme === 'light';

  return (
    <>
      {/* Layer 1a — Dark background (always mounted, fades via opacity) */}
      <div
        className="absolute inset-0 z-[1] bg-cover bg-center bg-no-repeat pointer-events-none bg-[url('/background.webp')] transition-opacity duration-1000 ease-in-out"
        style={{ opacity: isLight ? 0 : 0.4 }}
      />

      {/* Layer 1b — Light background (always mounted, fades via opacity) */}
      <div
        className="absolute inset-0 z-[1] bg-cover bg-center bg-no-repeat pointer-events-none bg-[url('/backgroundlight.webp')] transition-opacity duration-1000 ease-in-out"
        style={{ opacity: isLight ? 0.35 : 0 }}
      />

      {/* Layer 2 — Gradient overlay (crossfades between dark/light) */}
      <div className="absolute inset-0 z-[2] pointer-events-none">
        {/* Dark gradient */}
        <div
          className="absolute inset-0 bg-gradient-to-b from-black/40 via-transparent to-black/60 transition-opacity duration-1000 ease-in-out"
          style={{ opacity: isLight ? 0 : 1 }}
        />
        {/* Light gradient */}
        <div
          className="absolute inset-0 bg-gradient-to-b from-white/30 via-transparent to-slate-100/50 transition-opacity duration-1000 ease-in-out"
          style={{ opacity: isLight ? 1 : 0 }}
        />
      </div>

      {/* Layer 3 — Radial glow from center (crossfades between dark/light) */}
      <div className="absolute inset-0 z-[2] pointer-events-none">
        {/* Dark radial glow */}
        <div
          className="absolute inset-0 bg-[radial-gradient(ellipse_at_center,_var(--tw-gradient-stops))] from-white/5 via-transparent to-transparent transition-opacity duration-1000 ease-in-out"
          style={{ opacity: isLight ? 0 : 1 }}
        />
        {/* Light radial glow */}
        <div
          className="absolute inset-0 bg-[radial-gradient(ellipse_at_center,_var(--tw-gradient-stops))] from-emerald-500/5 via-transparent to-transparent transition-opacity duration-1000 ease-in-out"
          style={{ opacity: isLight ? 1 : 0 }}
        />
      </div>
    </>
  );
});

LayeredBackground.displayName = 'LayeredBackground';
