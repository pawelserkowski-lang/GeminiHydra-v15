/** Jaskier Shared Pattern — Working Folder Settings Section */

import { AlertCircle, Check, FolderOpen, X } from 'lucide-react';
import { memo, useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';

import { Button, Input } from '@/components/atoms';
import { apiPatch } from '@/shared/api/client';
import type { Settings } from '@/shared/api/schemas';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';
import { useSettingsQuery } from '../hooks/useSettings';

export const WorkingFolderSection = memo(() => {
  const { t } = useTranslation();
  const theme = useViewTheme();
  const { data: settings, refetch } = useSettingsQuery();

  const [editing, setEditing] = useState(false);
  const [value, setValue] = useState('');
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');

  useEffect(() => {
    if (settings?.working_directory !== undefined) {
      setValue(settings.working_directory);
    }
  }, [settings?.working_directory]);

  const handleSave = useCallback(async () => {
    setSaving(true);
    setError('');
    try {
      await apiPatch<Settings>('/api/settings', { working_directory: value.trim() });
      await refetch();
      setEditing(false);
      toast.success(t('settings.workingFolder.saved', 'Working folder saved'));
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Failed to save';
      setError(msg);
      toast.error(msg);
    } finally {
      setSaving(false);
    }
  }, [value, refetch, t]);

  const handleClear = useCallback(async () => {
    setSaving(true);
    setError('');
    try {
      await apiPatch<Settings>('/api/settings', { working_directory: '' });
      await refetch();
      setValue('');
      setEditing(false);
      toast.success(t('settings.workingFolder.cleared', 'Working folder cleared'));
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Failed to clear';
      setError(msg);
    } finally {
      setSaving(false);
    }
  }, [refetch, t]);

  const handleCancel = useCallback(() => {
    setValue(settings?.working_directory ?? '');
    setEditing(false);
    setError('');
  }, [settings?.working_directory]);

  const currentFolder = settings?.working_directory;

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-2">
        <FolderOpen size={18} className="text-[var(--matrix-accent)]" />
        <h3 className={cn('text-sm font-semibold font-mono uppercase tracking-wider', theme.text)}>
          {t('settings.workingFolder.title', 'Working Folder')}
        </h3>
      </div>

      <p className={cn('text-xs', theme.textMuted)}>
        {t(
          'settings.workingFolder.description',
          'Set a default working directory so agents can use relative paths instead of absolute ones.',
        )}
      </p>

      {editing ? (
        <div className="space-y-3">
          <Input
            value={value}
            onChange={(e) => {
              setValue(e.target.value);
              setError('');
            }}
            placeholder="C:\Users\you\project"
            onKeyDown={(e) => e.key === 'Enter' && handleSave()}
          />
          {error && (
            <div className="flex items-center gap-2 text-red-400">
              <AlertCircle size={14} />
              <span className="text-xs">{error}</span>
            </div>
          )}
          <div className="flex gap-2">
            <Button variant="primary" size="sm" leftIcon={<Check size={14} />} onClick={handleSave} isLoading={saving}>
              {t('common.save', 'Save')}
            </Button>
            <Button variant="ghost" size="sm" leftIcon={<X size={14} />} onClick={handleCancel} disabled={saving}>
              {t('common.cancel', 'Cancel')}
            </Button>
          </div>
        </div>
      ) : (
        <div className="space-y-3">
          {currentFolder ? (
            <div className={cn('text-sm font-mono px-3 py-2 rounded-lg bg-[var(--matrix-glass)]', theme.text)}>
              {currentFolder}
            </div>
          ) : (
            <p className={cn('text-xs italic', theme.textMuted)}>
              {t('settings.workingFolder.notSet', 'Not set — agents will require absolute paths')}
            </p>
          )}
          <div className="flex gap-2">
            <Button variant="primary" size="sm" leftIcon={<FolderOpen size={14} />} onClick={() => setEditing(true)}>
              {currentFolder
                ? t('settings.workingFolder.change', 'Change')
                : t('settings.workingFolder.set', 'Set Folder')}
            </Button>
            {currentFolder && (
              <Button variant="danger" size="sm" leftIcon={<X size={14} />} onClick={handleClear} isLoading={saving}>
                {t('settings.workingFolder.clear', 'Clear')}
              </Button>
            )}
          </div>
        </div>
      )}
    </div>
  );
});

WorkingFolderSection.displayName = 'WorkingFolderSection';
