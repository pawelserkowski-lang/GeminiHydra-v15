import { memo, useEffect, useRef } from 'react';

/**
 * WitcherRunes Component
 * Renders falling white/silver runes effect (Elder Futhark alphabet).
 * Inspired by Matrix Rain but using runic characters in white/silver tones.
 * Active in both dark and light modes with adapted opacity.
 *
 * Ported pixel-perfect from GeminiHydra legacy.
 */

interface WitcherRunesProps {
  isDark: boolean;
}

export const WitcherRunes = memo(({ isDark }: WitcherRunesProps) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // Respect prefers-reduced-motion
    const prefersReducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    if (prefersReducedMotion) return;

    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    // Configuration
    const fontSize = 14;
    let columns = 0;
    let drops: number[] = [];

    // Elder Futhark Runes + some mystical symbols
    const alphabet =
      '\u16A0\u16A2\u16A6\u16A8\u16B1\u16B2\u16B7\u16B9\u16BA\u16BE\u16C1\u16C3\u16C7\u16C8\u16C9\u16CA\u16CF\u16D2\u16D6\u16D7\u16DA\u16DC\u16DE\u16DF';

    // White/Silver theme - elegant falling runes
    const trailColor = isDark
      ? 'rgba(10, 14, 20, 0.07)' // dark bg fade
      : 'rgba(245, 248, 245, 0.09)'; // light bg fade

    // White/silver glow
    const glowColor = isDark ? 'rgba(255, 255, 255, 0.3)' : 'rgba(120, 140, 155, 0.2)';

    const resize = () => {
      if (canvas && containerRef.current) {
        canvas.width = containerRef.current.offsetWidth;
        canvas.height = containerRef.current.offsetHeight;

        columns = Math.floor(canvas.width / fontSize);
        if (drops.length !== columns) {
          drops = new Array(columns).fill(0).map(() => Math.random() * -50);
        }
      }
    };

    // Initial resize
    resize();
    window.addEventListener('resize', resize);

    // Drawing Loop
    const draw = () => {
      // Trail fade effect
      ctx.fillStyle = trailColor;
      ctx.fillRect(0, 0, canvas.width, canvas.height);

      ctx.font = `${fontSize}px 'JetBrains Mono', monospace`;
      ctx.shadowBlur = isDark ? 4 : 2;
      ctx.shadowColor = glowColor;

      for (let i = 0; i < drops.length; i++) {
        const text = alphabet.charAt(Math.floor(Math.random() * alphabet.length));
        const x = i * fontSize;
        const y = (drops[i] ?? 0) * fontSize;

        // Vary opacity per character for depth effect
        const charOpacity = 0.3 + Math.random() * 0.7;
        ctx.fillStyle = isDark
          ? `rgba(255, 255, 255, ${charOpacity * 0.5})`
          : `rgba(120, 140, 160, ${charOpacity * 0.4})`;

        ctx.fillText(text, x, y);

        // Reset drop randomly after reaching bottom
        if (y > canvas.height && Math.random() > 0.975) {
          drops[i] = 0;
        }

        drops[i] = (drops[i] ?? 0) + 1;
      }
    };

    // ~14fps for subtle background effect -- saves CPU
    const intervalId = setInterval(draw, 70);

    // GPU compositing hint
    canvas.style.willChange = 'transform';

    return () => {
      clearInterval(intervalId);
      canvas.style.willChange = 'auto';
      window.removeEventListener('resize', resize);
    };
  }, [isDark]);

  return (
    <div
      ref={containerRef}
      className={`absolute inset-0 pointer-events-none overflow-hidden z-0 transition-[opacity] duration-500 ease-[cubic-bezier(0.4,0,0.2,1)] ${
        isDark ? 'opacity-[0.18] mix-blend-screen' : 'opacity-[0.20]'
      }`}
    >
      <canvas ref={canvasRef} className="block w-full h-full" />
    </div>
  );
});

WitcherRunes.displayName = 'WitcherRunes';
