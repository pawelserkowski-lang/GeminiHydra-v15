// src/shared/hooks/useIsMobile.ts
/**
 * useIsMobile — Reactive mobile breakpoint detection
 * =====================================================
 * Returns true when window width is below the given breakpoint (default 768px).
 * Updates on window resize.
 */

import { useEffect, useState } from 'react';

export function useIsMobile(breakpoint = 768): boolean {
  const [isMobile, setIsMobile] = useState(typeof window !== 'undefined' ? window.innerWidth < breakpoint : false);

  useEffect(() => {
    const handler = () => setIsMobile(window.innerWidth < breakpoint);
    window.addEventListener('resize', handler);
    return () => window.removeEventListener('resize', handler);
  }, [breakpoint]);

  return isMobile;
}
