import { ExternalLink, Eye, EyeOff, Key, Loader2 } from 'lucide-react';
import { memo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button, Input } from '@/components/atoms';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';

interface ApiKeyFormProps {
  onSave: (key: string) => void;
  isSaving: boolean;
  errorMessage: string | null;
}

export const ApiKeyForm = memo(({ onSave, isSaving, errorMessage }: ApiKeyFormProps) => {
  const { t } = useTranslation();
  const theme = useViewTheme();
  const [key, setKey] = useState('');
  const [showKey, setShowKey] = useState(false);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (key.trim()) onSave(key.trim());
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-4">
      <div className="flex items-center gap-2 mb-2">
        <Key size={16} className="text-[var(--matrix-accent)]" />
        <h3 className={cn('text-sm font-semibold font-mono', theme.text)}>{t('auth.apiKeyTitle')}</h3>
      </div>
      <p className={cn('text-xs', theme.textMuted)}>{t('auth.apiKeyDesc')}</p>

      <div className="relative">
        <Input
          type={showKey ? 'text' : 'password'}
          value={key}
          onChange={(e) => setKey(e.target.value)}
          placeholder={t('auth.apiKeyPlaceholder')}
          autoComplete="off"
          rightElement={
            <button
              type="button"
              onClick={() => setShowKey(!showKey)}
              className={cn('p-1 rounded hover:bg-white/10 transition-colors', theme.textMuted)}
            >
              {showKey ? <EyeOff size={14} /> : <Eye size={14} />}
            </button>
          }
        />
      </div>

      {errorMessage && <p className="text-xs text-red-400 font-mono">{errorMessage}</p>}

      <Button
        type="submit"
        variant="primary"
        size="sm"
        disabled={!key.trim() || isSaving}
        isLoading={isSaving}
        leftIcon={isSaving ? <Loader2 size={14} className="animate-spin" /> : <Key size={14} />}
        className="w-full"
      >
        {isSaving ? t('auth.apiKeyValidating') : t('auth.apiKeyValidate')}
      </Button>

      <a
        href="https://aistudio.google.com/apikey"
        target="_blank"
        rel="noopener noreferrer"
        className={cn(
          'inline-flex items-center gap-1.5 text-xs font-mono',
          'text-[var(--matrix-accent)] hover:underline',
        )}
      >
        <ExternalLink size={11} />
        {t('auth.apiKeyGetLink')}
      </a>
    </form>
  );
});

ApiKeyForm.displayName = 'ApiKeyForm';
