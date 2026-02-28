/** Jaskier Shared Pattern — Google Auth status hook (API Key + OAuth) */

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { apiDelete, apiGet, apiPost } from '@/shared/api/client';
import type { AuthLoginResponse, AuthStatus, SaveApiKeyResponse } from '@/shared/api/schemas';

const AUTH_DISMISSED_KEY = 'jaskier_auth_dismissed';
const AUTH_QUERY_KEY = ['auth-status'] as const;

export type AuthPhase = 'idle' | 'oauth_pending' | 'saving_key' | 'authenticated' | 'error';

export interface UseAuthStatusReturn {
  status: AuthStatus | undefined;
  isLoading: boolean;
  phase: AuthPhase;
  authMethod: 'oauth' | 'api_key' | 'env' | null;
  isDismissed: boolean;
  dismiss: () => void;
  login: () => void;
  saveApiKey: (key: string) => void;
  deleteApiKey: () => void;
  logout: () => void;
  cancel: () => void;
  authUrl: string | null;
  errorMessage: string | null;
  isMutating: boolean;
}

function readDismissed(): boolean {
  try {
    return localStorage.getItem(AUTH_DISMISSED_KEY) === 'true';
  } catch {
    return false;
  }
}

export function useAuthStatus(): UseAuthStatusReturn {
  const { t } = useTranslation();
  const qc = useQueryClient();

  const [localPhase, setLocalPhase] = useState<AuthPhase>('idle');
  const [isDismissed, setIsDismissed] = useState(readDismissed);
  const [authUrl, setAuthUrl] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // Stop polling helper
  const stopPolling = useCallback(() => {
    if (pollRef.current) {
      clearInterval(pollRef.current);
      pollRef.current = null;
    }
  }, []);

  // Cleanup on unmount
  useEffect(() => stopPolling, [stopPolling]);

  const { data: status, isLoading } = useQuery<AuthStatus>({
    queryKey: AUTH_QUERY_KEY,
    queryFn: () => apiGet<AuthStatus>('/api/auth/status'),
    staleTime: 60_000,
    refetchInterval: 300_000,
    retry: 1,
  });

  // Derive phase from backend status + local state
  const phase: AuthPhase = status?.authenticated && !status.expired ? 'authenticated' : localPhase;
  const authMethod = status?.method ?? null;

  // Stop polling when authenticated
  useEffect(() => {
    if (status?.authenticated && localPhase === 'oauth_pending') {
      stopPolling();
      setLocalPhase('idle');
      setAuthUrl(null);
      toast.success(t('auth.loginSuccess'));
    }
  }, [status?.authenticated, localPhase, stopPolling, t]);

  // ── Google OAuth flow ──────────────────────────────────────────────
  const loginMutation = useMutation({
    mutationFn: () => apiPost<AuthLoginResponse>('/api/auth/login'),
    onSuccess: (data) => {
      setAuthUrl(data.auth_url);
      setErrorMessage(null);
      setLocalPhase('oauth_pending');

      // Open Google consent in new window
      const win = window.open(data.auth_url, '_blank', 'noopener');
      if (!win) {
        toast.info(t('auth.popupBlocked'));
      }

      // Start polling /api/auth/status every 2s
      stopPolling();
      pollRef.current = setInterval(() => {
        qc.invalidateQueries({ queryKey: AUTH_QUERY_KEY });
      }, 2000);
    },
    onError: (err) => {
      const msg = err instanceof Error ? err.message : t('auth.loginError');
      setErrorMessage(msg);
      toast.error(t('auth.loginError'));
      setLocalPhase('error');
    },
  });

  // ── API Key flow ───────────────────────────────────────────────────
  const saveKeyMutation = useMutation({
    mutationFn: (key: string) => apiPost<SaveApiKeyResponse>('/api/auth/apikey', { api_key: key }),
    onMutate: () => {
      setLocalPhase('saving_key');
      setErrorMessage(null);
    },
    onSuccess: () => {
      setLocalPhase('idle');
      qc.invalidateQueries({ queryKey: AUTH_QUERY_KEY });
      toast.success(t('auth.apiKeySaved'));
    },
    onError: (err) => {
      const msg = err instanceof Error ? err.message : t('auth.invalidApiKey');
      setErrorMessage(msg);
      toast.error(t('auth.invalidApiKey'));
      setLocalPhase('error');
    },
  });

  const deleteKeyMutation = useMutation({
    mutationFn: () => apiDelete('/api/auth/apikey'),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: AUTH_QUERY_KEY });
      toast.success(t('auth.apiKeyDeleted'));
    },
  });

  // ── Logout ─────────────────────────────────────────────────────────
  const logoutMutation = useMutation({
    mutationFn: () => apiPost('/api/auth/logout'),
    onSuccess: () => {
      setLocalPhase('idle');
      setAuthUrl(null);
      setErrorMessage(null);
      qc.invalidateQueries({ queryKey: AUTH_QUERY_KEY });
      toast.success(t('auth.logoutSuccess'));
    },
  });

  const login = useCallback(() => {
    setErrorMessage(null);
    loginMutation.mutate();
  }, [loginMutation]);

  const saveApiKey = useCallback(
    (key: string) => {
      saveKeyMutation.mutate(key);
    },
    [saveKeyMutation],
  );

  const deleteApiKey = useCallback(() => {
    deleteKeyMutation.mutate();
  }, [deleteKeyMutation]);

  const logout = useCallback(() => {
    logoutMutation.mutate();
  }, [logoutMutation]);

  const cancel = useCallback(() => {
    stopPolling();
    setLocalPhase('idle');
    setAuthUrl(null);
    setErrorMessage(null);
  }, [stopPolling]);

  const dismiss = useCallback(() => {
    try {
      localStorage.setItem(AUTH_DISMISSED_KEY, 'true');
    } catch {
      /* ignore */
    }
    setIsDismissed(true);
  }, []);

  return {
    status,
    isLoading,
    phase,
    authMethod,
    isDismissed,
    dismiss,
    login,
    saveApiKey,
    deleteApiKey,
    logout,
    cancel,
    authUrl,
    errorMessage,
    isMutating:
      loginMutation.isPending || saveKeyMutation.isPending || deleteKeyMutation.isPending || logoutMutation.isPending,
  };
}
