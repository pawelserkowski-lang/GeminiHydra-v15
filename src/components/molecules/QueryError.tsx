/** Jaskier Shared Pattern -- QueryError */
import { AlertCircle, RefreshCw } from 'lucide-react';
import { memo } from 'react';
import { useTranslation } from 'react-i18next';

interface QueryErrorProps {
  onRetry: () => void;
  message?: string;
}

export const QueryError = memo(function QueryError({ onRetry, message }: QueryErrorProps) {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col items-center justify-center gap-4 p-8 text-center">
      <AlertCircle className="w-12 h-12 text-red-400/60" />
      <p className="text-sm text-[var(--matrix-text-secondary)]">
        {message || t('common.loadError', 'Failed to load data')}
      </p>
      <button
        type="button"
        onClick={onRetry}
        className="flex items-center gap-2 px-4 py-2 text-sm rounded-lg bg-[var(--matrix-glass-bg)] border border-[var(--matrix-border)] hover:bg-[var(--matrix-glass-hover)] transition-colors"
      >
        <RefreshCw className="w-4 h-4" />
        {t('common.retry', 'Retry')}
      </button>
    </div>
  );
});
