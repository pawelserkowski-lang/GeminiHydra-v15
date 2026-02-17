// src/features/settings/components/SettingsView.tsx
/**
 * Settings View
 * =============
 * Full settings panel with Model Configuration, UI, and Advanced sections.
 * Uses existing hooks from useSettings.ts and atom components.
 */

import { AnimatePresence, motion } from 'motion/react';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { Badge, Button, Card, Input } from '@/components/atoms';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';
import {
  useResetSettingsMutation,
  useSettingsQuery,
  useUpdateSettingsMutation,
} from '@/features/settings/hooks/useSettings';
import type { Settings } from '@/shared/api/schemas';

// ============================================================================
// HELPERS
// ============================================================================

function getTemperatureLabel(t: (key: string) => string, value: number): { label: string; variant: 'accent' | 'success' | 'warning' | 'error' } {
  if (value <= 0.5) return { label: t('settings.temperature_labels.focused'), variant: 'accent' };
  if (value <= 1.0) return { label: t('settings.temperature_labels.balanced'), variant: 'success' };
  if (value <= 1.5) return { label: t('settings.temperature_labels.creative'), variant: 'warning' };
  return { label: t('settings.temperature_labels.experimental'), variant: 'error' };
}

// ============================================================================
// COMPONENT
// ============================================================================

export default function SettingsView() {
  const { t } = useTranslation();
  const theme = useViewTheme();

  const { data: settings, isLoading, error } = useSettingsQuery();
  const updateMutation = useUpdateSettingsMutation();
  const resetMutation = useResetSettingsMutation();

  const [formData, setFormData] = useState<Partial<Settings>>({});
  const [showResetDialog, setShowResetDialog] = useState(false);

  // Sync form data when settings load
  useEffect(() => {
    if (settings) {
      setFormData({
        temperature: settings.temperature,
        max_tokens: settings.max_tokens,
        default_model: settings.default_model,
        language: settings.language,
        theme: settings.theme,
        welcome_message: settings.welcome_message ?? '',
      });
    }
  }, [settings]);

  const isDirty = useMemo(() => {
    if (!settings) return false;
    return (
      formData.temperature !== settings.temperature ||
      formData.max_tokens !== settings.max_tokens ||
      formData.default_model !== settings.default_model ||
      formData.language !== settings.language ||
      formData.theme !== settings.theme ||
      formData.welcome_message !== (settings.welcome_message ?? '')
    );
  }, [formData, settings]);

  const handleSave = useCallback(() => {
    updateMutation.mutate(formData, {
      onSuccess: () => toast.success(t('settings.save_success')),
      onError: () => toast.error(t('common.error')),
    });
  }, [formData, updateMutation, t]);

  const handleReset = useCallback(() => {
    resetMutation.mutate(undefined, {
      onSuccess: () => {
        setShowResetDialog(false);
        toast.success(t('settings.reset_success'));
      },
      onError: () => toast.error(t('common.error')),
    });
  }, [resetMutation, t]);

  const updateField = useCallback(<K extends keyof Settings>(key: K, value: Settings[K]) => {
    setFormData((prev) => ({ ...prev, [key]: value }));
  }, []);

  const tempLabel = getTemperatureLabel(t, formData.temperature ?? 0.7);

  // Loading state
  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-full">
        <p className={cn('text-lg font-mono animate-pulse', theme.isLight ? 'text-slate-600' : 'text-white/50')}>
          {t('common.loading')}
        </p>
      </div>
    );
  }

  // Error state
  if (error) {
    return (
      <div className="flex items-center justify-center h-full p-6">
        <Card variant="glass" padding="lg" className="max-w-md text-center">
          <p className="text-[var(--matrix-error)] font-mono text-lg mb-4">{t('common.error')}</p>
          <p className={cn('text-sm', theme.isLight ? 'text-slate-600' : 'text-white/50')}>
            {error.message}
          </p>
        </Card>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col overflow-hidden">
      {/* Scrollable content */}
      <div className="flex-1 overflow-y-auto scrollbar-hide hover:scrollbar-thin hover:scrollbar-thumb-white/20 p-6 pb-24 space-y-6">
        {/* Header */}
        <div>
          <h1 className={cn('text-2xl font-bold font-mono', theme.isLight ? 'text-slate-900' : 'text-white')}>
            {t('settings.title')}
          </h1>
          <p className={cn('text-sm mt-1', theme.isLight ? 'text-slate-600' : 'text-white/50')}>
            {t('settings.subtitle')}
          </p>
        </div>

        {/* Model Configuration */}
        <Card variant="glass" padding="lg">
          <h2 className={cn('text-lg font-bold font-mono mb-4', theme.isLight ? 'text-slate-800' : 'text-white/90')}>
            {t('settings.sections.model')}
          </h2>
          <div className="space-y-5">
            {/* Temperature */}
            <div>
              <div className="flex items-center justify-between mb-2">
                <label className={cn('text-sm font-medium', theme.isLight ? 'text-slate-700' : 'text-white/80')}>
                  {t('settings.fields.temperature')}
                </label>
                <Badge variant={tempLabel.variant} size="sm">{tempLabel.label} ({formData.temperature?.toFixed(1)})</Badge>
              </div>
              <input
                type="range"
                min="0"
                max="2"
                step="0.1"
                value={formData.temperature ?? 0.7}
                onChange={(e) => updateField('temperature', Number.parseFloat(e.target.value))}
                className="w-full accent-[var(--matrix-accent)] cursor-pointer"
              />
              <p className={cn('text-xs mt-1', theme.isLight ? 'text-slate-500' : 'text-white/40')}>
                {t('settings.fields.temperature_desc')}
              </p>
            </div>

            {/* Max Tokens */}
            <Input
              label={t('settings.fields.max_tokens')}
              type="number"
              value={formData.max_tokens ?? 4096}
              onChange={(e) => updateField('max_tokens', Number.parseInt(e.target.value, 10) || 0)}
              min={1}
              max={128000}
            />
            <p className={cn('text-xs -mt-3', theme.isLight ? 'text-slate-500' : 'text-white/40')}>
              {t('settings.fields.max_tokens_desc')}
            </p>

            {/* Default Model */}
            <div>
              <label className={cn('text-xs font-medium block mb-1.5', theme.isLight ? 'text-slate-600' : 'text-[var(--matrix-text-secondary)]')}>
                {t('settings.fields.default_model')}
              </label>
              <input
                type="text"
                value={formData.default_model ?? ''}
                onChange={(e) => updateField('default_model', e.target.value)}
                className="glass-input w-full rounded-lg font-mono text-sm px-3 py-2 text-[var(--matrix-text-primary)] placeholder:text-[var(--matrix-text-secondary)]/60 outline-none transition-all duration-200 focus:border-[var(--matrix-accent)] focus:ring-2 focus:ring-[var(--matrix-accent)]/30"
                placeholder="e.g. gemini-2.0-flash"
              />
              <p className={cn('text-xs mt-1', theme.isLight ? 'text-slate-500' : 'text-white/40')}>
                {t('settings.fields.default_model_desc')}
              </p>
            </div>
          </div>
        </Card>

        {/* User Interface */}
        <Card variant="glass" padding="lg">
          <h2 className={cn('text-lg font-bold font-mono mb-4', theme.isLight ? 'text-slate-800' : 'text-white/90')}>
            {t('settings.sections.ui')}
          </h2>
          <div className="space-y-5">
            {/* Language */}
            <div>
              <label className={cn('text-xs font-medium block mb-1.5', theme.isLight ? 'text-slate-600' : 'text-[var(--matrix-text-secondary)]')}>
                {t('settings.fields.language')}
              </label>
              <select
                value={formData.language ?? 'en'}
                onChange={(e) => updateField('language', e.target.value)}
                className="glass-input w-full rounded-lg font-mono text-sm px-3 py-2 text-[var(--matrix-text-primary)] outline-none transition-all duration-200 focus:border-[var(--matrix-accent)] focus:ring-2 focus:ring-[var(--matrix-accent)]/30 cursor-pointer"
              >
                <option value="en">English</option>
                <option value="pl">Polski</option>
              </select>
              <p className={cn('text-xs mt-1', theme.isLight ? 'text-slate-500' : 'text-white/40')}>
                {t('settings.fields.language_desc')}
              </p>
            </div>

            {/* Theme */}
            <div>
              <label className={cn('text-xs font-medium block mb-1.5', theme.isLight ? 'text-slate-600' : 'text-[var(--matrix-text-secondary)]')}>
                {t('settings.fields.theme')}
              </label>
              <select
                value={formData.theme ?? 'dark'}
                onChange={(e) => updateField('theme', e.target.value)}
                className="glass-input w-full rounded-lg font-mono text-sm px-3 py-2 text-[var(--matrix-text-primary)] outline-none transition-all duration-200 focus:border-[var(--matrix-accent)] focus:ring-2 focus:ring-[var(--matrix-accent)]/30 cursor-pointer"
              >
                <option value="dark">Dark</option>
                <option value="light">Light</option>
                <option value="system">System</option>
              </select>
              <p className={cn('text-xs mt-1', theme.isLight ? 'text-slate-500' : 'text-white/40')}>
                {t('settings.fields.theme_desc')}
              </p>
            </div>
          </div>
        </Card>

        {/* Advanced */}
        <Card variant="glass" padding="lg">
          <h2 className={cn('text-lg font-bold font-mono mb-4', theme.isLight ? 'text-slate-800' : 'text-white/90')}>
            {t('settings.sections.advanced')}
          </h2>
          <div>
            <label className={cn('text-xs font-medium block mb-1.5', theme.isLight ? 'text-slate-600' : 'text-[var(--matrix-text-secondary)]')}>
              {t('settings.fields.welcome_message')}
            </label>
            <textarea
              value={formData.welcome_message ?? ''}
              onChange={(e) => updateField('welcome_message', e.target.value)}
              rows={4}
              className="glass-input w-full rounded-lg font-mono text-sm px-3 py-2 text-[var(--matrix-text-primary)] placeholder:text-[var(--matrix-text-secondary)]/60 outline-none transition-all duration-200 focus:border-[var(--matrix-accent)] focus:ring-2 focus:ring-[var(--matrix-accent)]/30 resize-none"
              placeholder={t('settings.fields.welcome_message_desc')}
            />
            <p className={cn('text-xs mt-1', theme.isLight ? 'text-slate-500' : 'text-white/40')}>
              {t('settings.fields.welcome_message_desc')}
            </p>
          </div>
        </Card>
      </div>

      {/* Sticky Footer */}
      <div className={cn(
        'sticky bottom-0 flex items-center justify-end gap-3 px-6 py-4 border-t backdrop-blur-xl',
        theme.isLight ? 'border-slate-200/50 bg-white/60' : 'border-white/10 bg-black/60',
      )}>
        <Button
          variant="secondary"
          onClick={() => setShowResetDialog(true)}
          isLoading={resetMutation.isPending}
        >
          {t('settings.reset')}
        </Button>
        <Button
          variant="primary"
          onClick={handleSave}
          disabled={!isDirty}
          isLoading={updateMutation.isPending}
        >
          {t('settings.save')}
        </Button>
      </div>

      {/* Reset Confirmation Dialog */}
      <AnimatePresence>
        {showResetDialog && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="fixed inset-0 z-50 flex items-center justify-center"
          >
            {/* Backdrop */}
            <div
              className="absolute inset-0 bg-black/60 backdrop-blur-sm"
              onClick={() => setShowResetDialog(false)}
            />
            {/* Dialog */}
            <motion.div
              initial={{ opacity: 0, scale: 0.95 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.95 }}
              transition={{ duration: 0.15 }}
              className={cn(
                'relative z-10 max-w-md w-full mx-4 p-6 rounded-2xl border backdrop-blur-xl shadow-2xl',
                theme.isLight
                  ? 'bg-white/95 border-slate-200/50'
                  : 'bg-black/90 border-white/10',
              )}
            >
              <h3 className={cn('text-lg font-bold font-mono mb-3', theme.isLight ? 'text-slate-900' : 'text-white')}>
                {t('settings.reset')}
              </h3>
              <p className={cn('text-sm mb-6', theme.isLight ? 'text-slate-600' : 'text-white/60')}>
                {t('settings.reset_confirm')}
              </p>
              <div className="flex justify-end gap-3">
                <Button variant="secondary" onClick={() => setShowResetDialog(false)}>
                  {t('common.cancel')}
                </Button>
                <Button variant="danger" onClick={handleReset} isLoading={resetMutation.isPending}>
                  {t('common.confirm')}
                </Button>
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
