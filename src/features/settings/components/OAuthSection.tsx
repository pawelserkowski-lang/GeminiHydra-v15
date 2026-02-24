/**
 * OAuthSection — Anthropic Claude MAX Plan OAuth login flow.
 * Displays auth status and handles the PKCE OAuth flow:
 *   1. Generate auth URL → open in browser
 *   2. User authorizes → copies code#state
 *   3. Paste code → exchange for tokens
 */

import { CheckCircle, ExternalLink, Key, LogIn, LogOut, Shield } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';

import { Badge, Button, Input } from '@/components/atoms';
import { apiGet, apiPost } from '@/shared/api/client';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface AuthStatus {
  authenticated: boolean;
  expired?: boolean;
  expires_at?: number;
  scope?: string;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function OAuthSection() {
  const [status, setStatus] = useState<AuthStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [step, setStep] = useState<'idle' | 'waiting_code' | 'exchanging'>('idle');
  const [codeInput, setCodeInput] = useState('');
  const [error, setError] = useState<string | null>(null);

  // -- Fetch auth status --

  const fetchStatus = useCallback(async () => {
    try {
      const data = await apiGet<AuthStatus>('/api/auth/status');
      setStatus(data);
    } catch {
      setStatus({ authenticated: false });
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchStatus();
  }, [fetchStatus]);

  // -- Login flow --

  const handleStartLogin = async () => {
    setError(null);
    try {
      const data = await apiPost<{ auth_url: string; state: string }>('/api/auth/login', {});
      setStep('waiting_code');
      window.open(data.auth_url, '_blank');
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to start login');
    }
  };

  const handleCompleteLogin = async () => {
    const trimmed = codeInput.trim();
    if (!trimmed) return;

    setError(null);
    setStep('exchanging');

    // Parse "code#state" format
    const hashIdx = trimmed.indexOf('#');
    if (hashIdx === -1) {
      setError('Invalid format. Expected: code#state (with # separator)');
      setStep('waiting_code');
      return;
    }

    const code = trimmed.slice(0, hashIdx);
    const state = trimmed.slice(hashIdx + 1);

    if (!code || !state) {
      setError('Invalid format. Both code and state are required.');
      setStep('waiting_code');
      return;
    }

    try {
      await apiPost('/api/auth/callback', { code, state });
      await fetchStatus();
      setStep('idle');
      setCodeInput('');
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Login failed');
      setStep('waiting_code');
    }
  };

  const handleLogout = async () => {
    try {
      await apiPost('/api/auth/logout', {});
      setStatus({ authenticated: false });
      setStep('idle');
    } catch {
      // ignore
    }
  };

  const handleCancel = () => {
    setStep('idle');
    setCodeInput('');
    setError(null);
  };

  // -- Render --

  if (loading) return null;

  const isAuthenticated = status?.authenticated && !status?.expired;

  return (
    <div className="space-y-4">
      {/* Status indicator */}
      <div className="flex items-center gap-3">
        <Shield size={18} className={isAuthenticated ? 'text-emerald-400' : 'text-amber-400'} />
        <span className="text-sm text-[var(--matrix-text-primary)]">Anthropic OAuth (MAX Plan)</span>
        {isAuthenticated ? (
          <Badge variant="success" size="sm" dot>
            Authenticated
          </Badge>
        ) : (
          <Badge variant="error" size="sm" dot>
            Not Connected
          </Badge>
        )}
      </div>

      {isAuthenticated ? (
        /* ── Authenticated view ── */
        <div className="space-y-3">
          <p className="text-xs text-[var(--matrix-text-secondary)]">
            Connected via Claude MAX Plan OAuth. Token expires:{' '}
            <span className="text-[var(--matrix-accent)] font-mono">
              {status?.expires_at ? new Date(status.expires_at * 1000).toLocaleString() : 'unknown'}
            </span>
          </p>
          <Button variant="ghost" size="sm" onClick={handleLogout} leftIcon={<LogOut size={12} />}>
            Disconnect
          </Button>
        </div>
      ) : (
        /* ── Login flow ── */
        <div className="space-y-3">
          <p className="text-xs text-[var(--matrix-text-secondary)]">
            Login with your Anthropic Claude MAX subscription ($100/mo or $200/mo) for flat-rate API access without an
            API key.
          </p>

          {error && (
            <div className="text-xs text-red-400 bg-red-400/10 p-2 rounded border border-red-400/20">{error}</div>
          )}

          {step === 'idle' && (
            <Button variant="primary" size="sm" onClick={handleStartLogin} leftIcon={<LogIn size={12} />}>
              Login with Anthropic
            </Button>
          )}

          {step === 'waiting_code' && (
            <div className="space-y-3 p-3 rounded-lg border border-[var(--matrix-accent)]/30 bg-[var(--matrix-accent)]/5">
              <div className="flex items-center gap-2 text-xs text-[var(--matrix-accent)]">
                <ExternalLink size={12} />
                <span className="font-medium">Step 1: Authorize in the browser window</span>
              </div>
              <p className="text-[11px] text-[var(--matrix-text-secondary)]">
                A browser window opened with the Anthropic authorization page. After authorizing, copy the{' '}
                <code className="text-[var(--matrix-accent)] bg-[var(--matrix-bg-secondary)] px-1 rounded">
                  code#state
                </code>{' '}
                value shown on the page.
              </p>

              <div className="flex items-center gap-2 text-xs text-[var(--matrix-accent)]">
                <Key size={12} />
                <span className="font-medium">Step 2: Paste the code below</span>
              </div>
              <Input
                label="Authorization Code"
                type="text"
                value={codeInput}
                onChange={(e) => setCodeInput(e.target.value)}
                placeholder="Paste code#state here..."
                inputSize="sm"
              />

              <div className="flex gap-2">
                <Button
                  variant="primary"
                  size="sm"
                  onClick={handleCompleteLogin}
                  disabled={!codeInput.trim()}
                  leftIcon={<CheckCircle size={12} />}
                >
                  Complete Login
                </Button>
                <Button variant="ghost" size="sm" onClick={handleCancel}>
                  Cancel
                </Button>
              </div>
            </div>
          )}

          {step === 'exchanging' && (
            <div className="flex items-center gap-2 text-xs text-[var(--matrix-text-secondary)]">
              <div className="animate-spin w-3 h-3 border border-[var(--matrix-accent)] border-t-transparent rounded-full" />
              Exchanging tokens...
            </div>
          )}
        </div>
      )}
    </div>
  );
}
