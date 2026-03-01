import { motion } from 'motion/react';
import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';

export const StreamingIndicator = memo(() => {
  const { t } = useTranslation();
  const theme = useViewTheme();
  return (
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      className="flex items-center gap-2 px-4 py-2"
    >
      <div className="flex gap-1">
        {[0, 1, 2].map((i) => (
          <motion.span
            key={i}
            className={cn('w-1.5 h-1.5 rounded-full', theme.accentBg)}
            animate={{ opacity: [0.3, 1, 0.3] }}
            transition={{
              duration: 1.2,
              repeat: Number.POSITIVE_INFINITY,
              delay: i * 0.2,
            }}
          />
        ))}
      </div>
      <span className={cn('text-xs font-mono', theme.textMuted)}>{t('chat.generating', 'Generating...')}</span>
    </motion.div>
  );
});

StreamingIndicator.displayName = 'StreamingIndicator';
