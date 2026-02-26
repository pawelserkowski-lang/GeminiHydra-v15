// src/features/chat/components/OfflineBanner.tsx
/**
 * GeminiHydra v15 - Offline Banner (#25)
 * ========================================
 * Animated banner shown when the browser goes offline.
 * Uses useOnlineStatus() hook + motion/react for slide animation.
 */

import { WifiOff } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useOnlineStatus } from '@/shared/hooks/useOnlineStatus';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';

export const OfflineBanner = memo(() => {
  const { t } = useTranslation();
  const isOnline = useOnlineStatus();
  const theme = useViewTheme();

  return (
    <AnimatePresence>
      {!isOnline && (
        <motion.div
          initial={{ opacity: 0, height: 0 }}
          animate={{ opacity: 1, height: 'auto' }}
          exit={{ opacity: 0, height: 0 }}
          transition={{ duration: 0.25, ease: 'easeInOut' }}
          className="overflow-hidden shrink-0"
        >
          <div
            className={cn(
              'flex items-center justify-center gap-2 px-4 py-2.5 text-sm font-mono',
              theme.isLight
                ? 'bg-amber-500/15 border-b border-amber-500/20 text-amber-800'
                : 'bg-amber-500/10 border-b border-amber-500/20 text-amber-400',
            )}
          >
            <WifiOff size={14} className="animate-pulse" />
            <span>{t('chat.offlineWarning', "You're offline — messages will be sent when reconnected")}</span>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
});

OfflineBanner.displayName = 'OfflineBanner';

export default OfflineBanner;
