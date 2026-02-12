// src/components/molecules/DataSkeleton.tsx
/**
 * DataSkeleton - Reusable Skeleton for Data Loading States
 * =========================================================
 * Provides 'list', 'grid', and 'detail' variants for common
 * data-loading patterns. Built on the Skeleton atom.
 */

import type { HTMLAttributes } from 'react';
import { Skeleton } from '@/components/atoms/Skeleton';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type DataSkeletonVariant = 'list' | 'grid' | 'detail';

export interface DataSkeletonProps extends HTMLAttributes<HTMLDivElement> {
  /** Layout variant for the skeleton placeholder. */
  variant: DataSkeletonVariant;
  /** Number of repeated items for list/grid variants. Defaults to 4. */
  count?: number;
}

// ---------------------------------------------------------------------------
// List Variant
// ---------------------------------------------------------------------------

function ListSkeleton({ count }: { count: number }) {
  return (
    <div className="space-y-3">
      {Array.from({ length: count }, (_, i) => (
        <div key={`list-skeleton-${i.toString()}`} className="flex items-center gap-3 p-3 rounded-xl">
          <Skeleton shape="circle" width="2rem" height="2rem" />
          <div className="flex-1 space-y-1.5">
            <Skeleton shape="line" width={`${70 + (i % 3) * 10}%`} height="0.75rem" />
            <Skeleton shape="line" width={`${40 + (i % 2) * 15}%`} height="0.625rem" />
          </div>
          <Skeleton shape="rectangle" width="3rem" height="1.25rem" className="rounded-md" />
        </div>
      ))}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Grid Variant
// ---------------------------------------------------------------------------

function GridSkeleton({ count }: { count: number }) {
  return (
    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
      {Array.from({ length: count }, (_, i) => (
        <div
          key={`grid-skeleton-${i.toString()}`}
          className="space-y-3 p-4 rounded-xl border border-[var(--matrix-border)]/20"
        >
          <div className="flex items-center gap-2">
            <Skeleton shape="rectangle" width="2.5rem" height="2.5rem" className="rounded-lg" />
            <div className="flex-1 space-y-1">
              <Skeleton shape="line" width="60%" height="0.75rem" />
              <Skeleton shape="line" width="40%" height="0.625rem" />
            </div>
          </div>
          <Skeleton shape="line" width="90%" height="0.625rem" />
          <Skeleton shape="line" width="70%" height="0.625rem" />
          <Skeleton shape="rectangle" width="100%" height="1.5rem" className="rounded-md" />
        </div>
      ))}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Detail Variant
// ---------------------------------------------------------------------------

function DetailSkeleton() {
  return (
    <div className="space-y-6">
      {/* Title area */}
      <div className="space-y-2">
        <Skeleton shape="line" width="50%" height="1.5rem" />
        <Skeleton shape="line" width="30%" height="0.75rem" />
      </div>

      {/* Main content block */}
      <Skeleton shape="rectangle" width="100%" height="12rem" />

      {/* Metadata rows */}
      <div className="space-y-3">
        <div className="flex items-center gap-3">
          <Skeleton shape="line" width="8rem" height="0.75rem" />
          <Skeleton shape="line" width="12rem" height="0.75rem" />
        </div>
        <div className="flex items-center gap-3">
          <Skeleton shape="line" width="8rem" height="0.75rem" />
          <Skeleton shape="line" width="10rem" height="0.75rem" />
        </div>
        <div className="flex items-center gap-3">
          <Skeleton shape="line" width="8rem" height="0.75rem" />
          <Skeleton shape="line" width="14rem" height="0.75rem" />
        </div>
      </div>

      {/* Action area */}
      <div className="flex gap-3">
        <Skeleton shape="rectangle" width="6rem" height="2rem" className="rounded-lg" />
        <Skeleton shape="rectangle" width="6rem" height="2rem" className="rounded-lg" />
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main Component
// ---------------------------------------------------------------------------

export function DataSkeleton({ variant, count = 4, className = '', ...rest }: DataSkeletonProps) {
  return (
    <div className={`animate-in fade-in duration-300 ${className}`} aria-hidden="true" {...rest}>
      {variant === 'list' && <ListSkeleton count={count} />}
      {variant === 'grid' && <GridSkeleton count={count} />}
      {variant === 'detail' && <DetailSkeleton />}
    </div>
  );
}

export default DataSkeleton;
