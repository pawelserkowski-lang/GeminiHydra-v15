import { Chrome, ExternalLink, Loader2 } from 'lucide-react';
import { memo } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/components/atoms';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';

interface GoogleOAuthFlowProps {
  oauthAvailable: boolean;
  isWaiting: boolean;
  authUrl: string | null;
  onLogin: () => void;
  onCancel: () => void;
  isMutating: boolean;
}

export const GoogleOAuthFlow = memo(
  ({ oauthAvailable, isWaiting, authUrl, onLogin, onCancel, isMutating }: GoogleOAuthFlowProps) => {
    const { t } = useTranslation();
    const theme = useViewTheme();

    if (!oauthAvailable) {
      return (
        <div className="space-y-3 opacity-50">
          <div className="flex items-center gap-2">
            <Chrome size={16} className="text-[var(--matrix-accent)]" />
            <h3 className={cn('text-sm font-semibold font-mono', theme.text)}>{t('auth.oauthTitle')}</h3>
          </div>
          <p className={cn('text-xs', theme.textMuted)}>{t('auth.oauthNotAvailable')}</p>
        </div>
      );
    }

    if (isWaiting) {
      return (
        <div className="space-y-3">
          <div className="flex items-center gap-2">
            <Loader2 size={16} className="text-[var(--matrix-accent)] animate-spin" />
            <h3 className={cn('text-sm font-semibold font-mono', theme.text)}>{t('auth.oauthWaiting')}</h3>
          </div>
          <p className={cn('text-xs', theme.textMuted)}>{t('auth.oauthWaitingDesc')}</p>
          {authUrl && (
            <a
              href={authUrl}
              target="_blank"
              rel="noopener noreferrer"
              className={cn(
                'inline-flex items-center gap-1.5 text-xs font-mono',
                'text-[var(--matrix-accent)] hover:underline',
              )}
            >
              <ExternalLink size={11} />
              accounts.google.com
            </a>
          )}
          <Button variant="ghost" size="sm" onClick={onCancel}>
            {t('auth.cancel')}
          </Button>
        </div>
      );
    }

    return (
      <div className="space-y-3">
        <div className="flex items-center gap-2">
          <Chrome size={16} className="text-[var(--matrix-accent)]" />
          <h3 className={cn('text-sm font-semibold font-mono', theme.text)}>{t('auth.oauthTitle')}</h3>
        </div>
        <p className={cn('text-xs', theme.textMuted)}>{t('auth.oauthDesc')}</p>
        <Button
          variant="secondary"
          size="sm"
          leftIcon={<Chrome size={14} />}
          onClick={onLogin}
          isLoading={isMutating}
          className="w-full"
        >
          {t('auth.signInWithGoogle')}
        </Button>
      </div>
    );
  },
);

GoogleOAuthFlow.displayName = 'GoogleOAuthFlow';
