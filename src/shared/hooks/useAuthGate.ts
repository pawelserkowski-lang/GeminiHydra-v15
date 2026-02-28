/** Auth gate — redirects to login view when not authenticated. */

import { useEffect } from 'react';
import { useViewStore } from '@/stores/viewStore';
import { useAuthStatus } from './useAuthStatus';

/**
 * Checks auth status on mount. If not authenticated, redirects to the login view.
 * If already authenticated, does nothing (user stays on whatever view they're on).
 */
export function useAuthGate() {
  const { status, isLoading } = useAuthStatus();
  const setCurrentView = useViewStore((s) => s.setCurrentView);
  const currentView = useViewStore((s) => s.currentView);

  useEffect(() => {
    if (isLoading) return;

    if (!status?.authenticated && currentView !== 'login') {
      setCurrentView('login');
    }
  }, [status?.authenticated, isLoading, currentView, setCurrentView]);

  return { isLoading, isAuthenticated: !!status?.authenticated };
}
