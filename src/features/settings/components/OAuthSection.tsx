/** Jaskier Shared Pattern — Google Auth UI Section (Settings) */

import { AlertTriangle, CheckCircle, Chrome, Key, LogOut, Shield } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { memo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Badge, Button, Input } from '@/components/atoms';
import { useAuthStatus } from '@/shared/hooks/useAuthStatus';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';

const phaseVariants = {
  initial: { opacity: 0, y: 8 },
  animate: { opacity: 1, y: 0, transition: { duration: 0.25 } },
  exit: { opacity: 0, y: -8, transition: { duration: 0.15 } },
};

export const OAuthSection = memo(() => {
  const { t } = useTranslation();
  const theme = useViewTheme();
  const { status, phase, authMethod, login, saveApiKey, logout, cancel, authUrl, errorMessage, isMutating } =
    useAuthStatus();

  const [apiKeyInput, setApiKeyInput] = useState('');

  const methodLabel =
    authMethod === 'oauth'
      ? t('auth.methodOAuth')
      : authMethod === 'api_key'
        ? t('auth.methodApiKey')
        : authMethod === 'env'
          ? t('auth.methodEnv')
          : '';

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-2">
        <Shield size={18} className="text-[var(--matrix-accent)]" />
        <h3 className={cn('text-sm font-semibold font-mono uppercase tracking-wider', theme.text)}>
          {t('auth.title')}
        </h3>
      </div>

      <AnimatePresence mode="wait">
        {/* ── Phase: Authenticated ── */}
        {phase === 'authenticated' && (
          <motion.div key="auth" {...phaseVariants} className="space-y-3">
            <div className="flex items-center gap-3 flex-wrap">
              <Badge variant="accent" size="sm" icon={<CheckCircle size={12} />}>
                {t('auth.connected')}
              </Badge>
              <span className={cn('text-xs font-mono', theme.textMuted)}>
                {t('auth.method', { method: methodLabel })}
              </span>
            </div>

            {status?.user_email && (
              <p className={cn('text-xs font-mono', theme.textMuted)}>
                {t('auth.connectedAs', { email: status.user_email })}
              </p>
            )}

            {status?.expires_at && (
              <p className={cn('text-xs font-mono', theme.textMuted)}>
                {t('auth.expiresAt', { date: new Date(status.expires_at * 1000).toLocaleString() })}
              </p>
            )}

            {authMethod !== 'env' && (
              <Button
                variant="danger"
                size="sm"
                leftIcon={<LogOut size={14} />}
                onClick={logout}
                isLoading={isMutating}
              >
                {t('auth.disconnect')}
              </Button>
            )}

            {/* Allow upgrading from env var to OAuth */}
            {authMethod === 'env' && status?.oauth_available && (
              <div className="space-y-2 pt-2 border-t border-white/10">
                <p className={cn('text-xs', theme.textMuted)}>{t('auth.upgradeToOAuth')}</p>
                <Button
                  variant="secondary"
                  size="sm"
                  leftIcon={<Chrome size={14} />}
                  onClick={login}
                  isLoading={isMutating}
                >
                  {t('auth.signInWithGoogle')}
                </Button>
              </div>
            )}
          </motion.div>
        )}

        {/* ── Phase: OAuth Pending ── */}
        {phase === 'oauth_pending' && (
          <motion.div key="waiting" {...phaseVariants} className="space-y-3">
            <p className={cn('text-sm font-medium', theme.text)}>{t('auth.oauthWaiting')}</p>
            <p className={cn('text-xs', theme.textMuted)}>{t('auth.oauthWaitingDesc')}</p>
            {authUrl && (
              <a
                href={authUrl}
                target="_blank"
                rel="noopener noreferrer"
                className={cn(
                  'inline-flex items-center gap-1.5 text-xs font-mono text-[var(--matrix-accent)] hover:underline',
                )}
              >
                accounts.google.com
              </a>
            )}
            <Button variant="ghost" size="sm" onClick={cancel}>
              {t('auth.cancel')}
            </Button>
          </motion.div>
        )}

        {/* ── Phase: Idle / Error ── */}
        {(phase === 'idle' || phase === 'error' || phase === 'saving_key') && (
          <motion.div key="idle" {...phaseVariants} className="space-y-4">
            {/* API Key input */}
            <div className="space-y-2">
              <div className="flex items-center gap-2">
                <Key size={14} className="text-[var(--matrix-accent)]" />
                <span className={cn('text-xs font-medium', theme.text)}>{t('auth.apiKeyTitle')}</span>
              </div>
              <Input
                type="password"
                value={apiKeyInput}
                onChange={(e) => setApiKeyInput(e.target.value)}
                placeholder={t('auth.apiKeyPlaceholder')}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && apiKeyInput.trim()) saveApiKey(apiKeyInput.trim());
                }}
              />
              <Button
                variant="primary"
                size="sm"
                disabled={!apiKeyInput.trim() || phase === 'saving_key'}
                isLoading={phase === 'saving_key'}
                onClick={() => saveApiKey(apiKeyInput.trim())}
              >
                {t('auth.apiKeyValidate')}
              </Button>
            </div>

            {/* Google OAuth button */}
            {status?.oauth_available && (
              <Button
                variant="secondary"
                size="sm"
                leftIcon={<Chrome size={14} />}
                onClick={login}
                isLoading={isMutating}
              >
                {t('auth.signInWithGoogle')}
              </Button>
            )}

            {phase === 'error' && errorMessage && (
              <div className="flex items-center gap-2 text-red-400">
                <AlertTriangle size={14} />
                <span className="text-xs">{errorMessage}</span>
              </div>
            )}

            {/* Expired token warning */}
            {status?.authenticated && status.expired && (
              <div className="flex items-center gap-2 text-amber-400">
                <AlertTriangle size={14} />
                <span className="text-xs font-mono">{t('auth.expired')}</span>
              </div>
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
});

OAuthSection.displayName = 'OAuthSection';
