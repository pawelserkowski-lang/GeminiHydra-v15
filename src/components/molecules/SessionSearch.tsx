// src/components/molecules/SessionSearch.tsx
/**
 * SessionSearch — Debounced search input for filtering chat sessions in sidebar (#19)
 * Filters sessions by title match with 300ms debounce.
 */

import { Search, X } from 'lucide-react';
import { memo, useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { cn } from '@/shared/utils/cn';

interface SessionSearchProps {
  /** Callback fired with the debounced search query. */
  onSearch: (query: string) => void;
  /** Whether the app uses light theme. */
  isLight: boolean;
}

export const SessionSearch = memo<SessionSearchProps>(({ onSearch, isLight }) => {
  const { t } = useTranslation();
  const [value, setValue] = useState('');
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Debounced search (300ms)
  useEffect(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      onSearch(value.trim().toLowerCase());
    }, 300);
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, [value, onSearch]);

  const handleClear = useCallback(() => {
    setValue('');
    onSearch('');
  }, [onSearch]);

  return (
    <div className="relative px-1 pb-1">
      <Search
        size={12}
        className={cn(
          'absolute left-3 top-1/2 -translate-y-1/2 pointer-events-none',
          isLight ? 'text-slate-400' : 'text-white/30',
        )}
      />
      <input
        type="text"
        value={value}
        onChange={(e) => setValue(e.target.value)}
        placeholder={t('sidebar.searchSessions', 'Search sessions...')}
        className={cn(
          'w-full pl-7 pr-7 py-1.5 text-xs rounded-lg font-mono transition-colors',
          'outline-none border',
          isLight
            ? 'bg-white/50 border-slate-200/50 text-slate-700 placeholder:text-slate-400 focus:border-emerald-500/50'
            : 'bg-white/5 border-white/10 text-white/80 placeholder:text-white/30 focus:border-white/30',
        )}
      />
      {value && (
        <button
          type="button"
          onClick={handleClear}
          className={cn(
            'absolute right-3 top-1/2 -translate-y-1/2 p-0.5 rounded transition-colors',
            isLight ? 'text-slate-400 hover:text-slate-600' : 'text-white/30 hover:text-white/60',
          )}
          title={t('common.clear', 'Clear')}
        >
          <X size={10} />
        </button>
      )}
    </div>
  );
});

SessionSearch.displayName = 'SessionSearch';

export default SessionSearch;
