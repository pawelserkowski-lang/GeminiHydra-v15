/** Jaskier Shared Pattern — Settings View */

import { Settings } from 'lucide-react';
import { motion } from 'motion/react';
import { memo } from 'react';
import { useTranslation } from 'react-i18next';

import { Card } from '@/components/atoms';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';
import { OAuthSection } from './OAuthSection';

export const SettingsView = memo(() => {
  const { t } = useTranslation();
  const theme = useViewTheme();

  return (
    <div className="h-full flex flex-col items-center p-8 overflow-y-auto">
      <motion.div
        className="w-full max-w-2xl space-y-6"
        initial={{ opacity: 0, y: 12 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.4, ease: 'easeOut' }}
      >
        {/* Header */}
        <div className="flex items-center gap-3">
          <Settings size={22} className="text-[var(--matrix-accent)]" />
          <h1 className={cn('text-2xl font-bold font-mono tracking-tight', theme.title)}>
            {t('settings.title', 'Settings')}
          </h1>
        </div>

        {/* Authentication Section */}
        <Card>
          <div className="p-6">
            <OAuthSection />
          </div>
        </Card>
      </motion.div>
    </div>
  );
});

SettingsView.displayName = 'SettingsView';

export default SettingsView;
