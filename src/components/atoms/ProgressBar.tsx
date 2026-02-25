// src/components/atoms/ProgressBar.tsx
/**
 * ProgressBar Atom
 * ================
 * Animated progress bar with determinate and indeterminate modes.
 * Uses `motion` for smooth value transitions and the theme accent color.
 */

import { motion } from 'motion/react';
import { cn } from '@/shared/utils/cn';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type ProgressBarSize = 'sm' | 'md';

interface ProgressBarProps {
  /**
   * Progress value 0-100. When `undefined`, the bar enters indeterminate
   * (loading) mode with an animated sweep.
   */
  value?: number;
  /** Size variant. */
  size?: ProgressBarSize;
  /** Whether to display the percentage label beside the bar. */
  label?: boolean;
  /** Extra CSS classes on the outermost wrapper. */
  className?: string;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const heightMap: Record<ProgressBarSize, string> = {
  sm: 'h-1',
  md: 'h-2.5',
};

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function ProgressBar({ value, size = 'md', label = false, className = '' }: ProgressBarProps) {
  const isIndeterminate = value === undefined;
  const clamped = isIndeterminate ? 0 : Math.min(100, Math.max(0, value));

  return (
    <div className={cn('flex items-center gap-3', className)}>
      {/* Track */}
      <div
        className={cn('relative w-full overflow-hidden rounded-full bg-[var(--matrix-border)]/50', heightMap[size])}
        role="progressbar"
        aria-valuenow={isIndeterminate ? undefined : clamped}
        aria-valuemin={0}
        aria-valuemax={100}
      >
        {isIndeterminate ? (
          /* Indeterminate sweep */
          <motion.div
            className="absolute inset-y-0 left-0 w-1/3 rounded-full bg-[var(--matrix-accent)]"
            animate={{ x: ['-100%', '400%'] }}
            transition={{
              duration: 1.8,
              repeat: Infinity,
              ease: 'easeInOut',
            }}
          />
        ) : (
          /* Determinate fill */
          <motion.div
            className={cn(heightMap[size], 'rounded-full bg-[var(--matrix-accent)]')}
            initial={{ width: 0 }}
            animate={{ width: `${clamped}%` }}
            transition={{ duration: 0.4, ease: 'easeOut' }}
          />
        )}
      </div>

      {/* Optional percentage label */}
      {label && !isIndeterminate && (
        <span className="min-w-[3ch] text-right text-xs font-mono text-[var(--matrix-text-secondary)]">
          {Math.round(clamped)}%
        </span>
      )}
    </div>
  );
}
