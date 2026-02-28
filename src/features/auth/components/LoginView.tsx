import { motion } from 'motion/react';
import { memo, useEffect } from 'react';
import { useTranslation } from 'react-i18next';

import { Card, RuneRain, ThemedBackground } from '@/components/atoms';
import { useTheme } from '@/contexts/ThemeContext';
import { useAuthStatus } from '@/shared/hooks/useAuthStatus';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';
import { useViewStore } from '@/stores/viewStore';

import { ApiKeyForm } from './ApiKeyForm';
import { GoogleOAuthFlow } from './GoogleOAuthFlow';

export const LoginView = memo(() => {
  const { t } = useTranslation();
  const { resolvedTheme } = useTheme();
  const theme = useViewTheme();
  const isLight = resolvedTheme === 'light';
  const setCurrentView = useViewStore((s) => s.setCurrentView);

  const { status, phase, saveApiKey, login, cancel, authUrl, errorMessage, isMutating } = useAuthStatus();

  // Redirect to home when authenticated
  useEffect(() => {
    if (status?.authenticated) {
      setCurrentView('home');
    }
  }, [status?.authenticated, setCurrentView]);

  return (
    <div
      className={cn(
        'relative flex h-screen w-full items-center justify-center overflow-hidden font-mono',
        isLight ? 'text-black' : 'text-white',
      )}
    >
      <ThemedBackground resolvedTheme={resolvedTheme} />
      <RuneRain opacity={0.08} />

      <motion.div
        className="relative z-10 w-full max-w-md px-4"
        initial={{ opacity: 0, y: 20, scale: 0.97 }}
        animate={{ opacity: 1, y: 0, scale: 1 }}
        transition={{ duration: 0.5, ease: 'easeOut' }}
      >
        {/* Logo */}
        <div className="flex flex-col items-center mb-8">
          <img
            src={isLight ? '/logolight.webp' : '/logodark.webp'}
            alt="GeminiHydra"
            className="h-20 object-contain mb-4"
          />
          <h1 className={cn('text-xl font-bold font-mono tracking-tight', theme.title)}>{t('auth.loginTitle')}</h1>
          <p className={cn('text-xs mt-2 text-center max-w-sm', theme.textMuted)}>{t('auth.loginSubtitle')}</p>
        </div>

        {/* API Key Card */}
        <Card>
          <div className="p-5">
            <ApiKeyForm
              onSave={saveApiKey}
              isSaving={phase === 'saving_key'}
              errorMessage={phase === 'error' ? errorMessage : null}
            />
          </div>
        </Card>

        {/* Divider */}
        {status?.oauth_available && (
          <div className="flex items-center gap-3 my-4">
            <div className={cn('flex-1 h-px', isLight ? 'bg-black/10' : 'bg-white/10')} />
            <span className={cn('text-xs font-mono uppercase tracking-wider', theme.textMuted)}>{t('auth.or')}</span>
            <div className={cn('flex-1 h-px', isLight ? 'bg-black/10' : 'bg-white/10')} />
          </div>
        )}

        {/* Google OAuth Card */}
        {status?.oauth_available && (
          <Card>
            <div className="p-5">
              <GoogleOAuthFlow
                oauthAvailable={!!status?.oauth_available}
                isWaiting={phase === 'oauth_pending'}
                authUrl={authUrl}
                onLogin={login}
                onCancel={cancel}
                isMutating={isMutating}
              />
            </div>
          </Card>
        )}
      </motion.div>
    </div>
  );
});

LoginView.displayName = 'LoginView';

export default LoginView;
