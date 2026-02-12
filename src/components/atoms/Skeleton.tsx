// src/components/atoms/Skeleton.tsx
/**
 * Skeleton Loader Atom
 * ====================
 * Shimmer-animated loading placeholder with shape variants.
 * Uses the `shimmer` keyframe from globals.css for the sweep effect
 * and theme background colors for the base surface.
 */

import type { HTMLAttributes } from 'react';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type SkeletonShape = 'line' | 'circle' | 'rectangle';

interface SkeletonProps extends HTMLAttributes<HTMLDivElement> {
  /** Visual shape of the skeleton placeholder. */
  shape?: SkeletonShape;
  /** Width — any CSS value (e.g. '100%', '200px', 80). Numbers are treated as px. */
  width?: string | number;
  /** Height — any CSS value. Numbers are treated as px. */
  height?: string | number;
  /** Extra CSS classes. */
  className?: string;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const toCss = (v: string | number): string => (typeof v === 'number' ? `${v}px` : v);

const shapeClasses: Record<SkeletonShape, string> = {
  line: 'rounded-md',
  circle: 'rounded-full',
  rectangle: 'rounded-lg',
};

const shapeDefaults: Record<SkeletonShape, { width: string; height: string }> = {
  line: { width: '100%', height: '0.875rem' },
  circle: { width: '2.5rem', height: '2.5rem' },
  rectangle: { width: '100%', height: '4rem' },
};

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function Skeleton({ shape = 'rectangle', width, height, className = '', style, ...rest }: SkeletonProps) {
  const defaults = shapeDefaults[shape];

  return (
    <div
      className={`relative overflow-hidden bg-[var(--matrix-border)]/50 ${shapeClasses[shape]} ${className}`}
      style={{
        width: width != null ? toCss(width) : defaults.width,
        height: height != null ? toCss(height) : defaults.height,
        ...style,
      }}
      aria-hidden="true"
      {...rest}
    >
      {/* Shimmer sweep — uses the shimmer keyframe from globals.css */}
      <div
        className="absolute inset-0"
        style={{
          background: 'linear-gradient(90deg, transparent 0%, rgba(255,255,255,0.08) 50%, transparent 100%)',
          animation: 'shimmer 1.5s ease-in-out infinite',
        }}
      />
    </div>
  );
}
