/** Jaskier Shared Pattern — Google Auth startup suggestion banner */

import { ArrowRight, Key, X } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { memo } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/components/atoms';
import { useAuthStatus } from '@/shared/hooks/useAuthStatus';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';
import { useViewStore } from '@/stores/viewStore';

export const OAuthBanner = memo(() => {
  const { t } = useTranslation();
  const theme = useViewTheme();
  const { status, isLoading, isDismissed, dismiss } = useAuthStatus();
  const setCurrentView = useViewStore((s) => s.setCurrentView);

  const visible = !isLoading && !status?.authenticated && !isDismissed;

  return (
    <AnimatePresence>
      {visible && (
        <motion.div
          className="w-full max-w-lg mt-6"
          initial={{ opacity: 0, y: -12, scale: 0.97 }}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          exit={{ opacity: 0, y: -12, scale: 0.97 }}
          transition={{ duration: 0.3, ease: 'easeOut' }}
        >
          <div
            className={cn(
              'relative rounded-2xl p-5',
              'border border-[var(--matrix-accent)]/20',
              'bg-[var(--matrix-accent)]/5',
              theme.card,
            )}
          >
            {/* Dismiss X */}
            <button
              type="button"
              onClick={dismiss}
              className={cn(
                'absolute top-3 right-3 p-1 rounded-lg',
                'transition-colors hover:bg-white/10',
                theme.textMuted,
              )}
              aria-label={t('common.close', 'Close')}
            >
              <X size={14} />
            </button>

            <div className="flex items-start gap-4">
              {/* Icon */}
              <div className="flex-shrink-0 p-2.5 rounded-xl bg-[var(--matrix-accent)]/10">
                <Key size={20} className="text-[var(--matrix-accent)]" />
              </div>

              {/* Content */}
              <div className="flex-1 min-w-0">
                <h3 className={cn('text-sm font-semibold font-mono', theme.text)}>{t('auth.bannerTitle')}</h3>
                <p className={cn('text-xs mt-1', theme.textMuted)}>{t('auth.bannerDesc')}</p>

                {/* Actions */}
                <div className="flex gap-2 mt-3 flex-wrap">
                  <Button
                    variant="primary"
                    size="sm"
                    rightIcon={<ArrowRight size={13} />}
                    onClick={() => setCurrentView('settings')}
                  >
                    {t('auth.setupAuth')}
                  </Button>
                  <Button variant="ghost" size="sm" onClick={dismiss}>
                    {t('auth.useApiKeyDismiss')}
                  </Button>
                </div>
              </div>
            </div>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
});

OAuthBanner.displayName = 'OAuthBanner';
