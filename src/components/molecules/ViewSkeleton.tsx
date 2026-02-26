// src/components/molecules/ViewSkeleton.tsx
/**
 * ViewSkeleton - Suspense Fallback for Lazy-Loaded Views
 * =======================================================
 * Displays a full-view skeleton loading placeholder with a
 * header region and content shimmer area. Used as `<Suspense fallback>`.
 */

import { useTranslation } from 'react-i18next';
import { Skeleton } from '@/components/atoms/Skeleton';

export function ViewSkeleton() {
  const { t } = useTranslation();
  return (
    // biome-ignore lint/a11y/useSemanticElements: div with role=status is appropriate for loading skeletons
    <div
      className="flex flex-col h-full w-full overflow-hidden animate-in fade-in duration-300"
      role="status"
      aria-busy="true"
      aria-live="polite"
      aria-label={t('common.loadingView', 'Loading view')}
    >
      {/* Header skeleton */}
      <div className="px-6 py-4 border-b border-[var(--matrix-border)]/30 space-y-3">
        <Skeleton shape="line" width="40%" height="1.5rem" />
        <Skeleton shape="line" width="60%" height="0.75rem" />
      </div>

      {/* Content area skeleton */}
      <div className="flex-1 p-6 space-y-4">
        {/* Row blocks simulating content */}
        <div className="flex items-center gap-3">
          <Skeleton shape="circle" width="2.5rem" height="2.5rem" />
          <div className="flex-1 space-y-2">
            <Skeleton shape="line" width="70%" height="0.875rem" />
            <Skeleton shape="line" width="50%" height="0.75rem" />
          </div>
        </div>

        <Skeleton shape="rectangle" width="100%" height="6rem" />

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          <Skeleton shape="rectangle" width="100%" height="5rem" />
          <Skeleton shape="rectangle" width="100%" height="5rem" />
          <Skeleton shape="rectangle" width="100%" height="5rem" />
        </div>

        <div className="space-y-2">
          <Skeleton shape="line" width="90%" height="0.875rem" />
          <Skeleton shape="line" width="75%" height="0.875rem" />
          <Skeleton shape="line" width="85%" height="0.875rem" />
          <Skeleton shape="line" width="60%" height="0.875rem" />
        </div>
      </div>
    </div>
  );
}

export default ViewSkeleton;
