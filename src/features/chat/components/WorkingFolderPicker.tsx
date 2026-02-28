/** Compact working folder picker with directory browser for ChatInput area */

import { Check, ChevronRight, FolderOpen, FolderUp, Pencil, X } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { memo, useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { apiPatch, apiPost } from '@/shared/api/client';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';

// ============================================================================
// TYPES
// ============================================================================

interface DirEntry {
  name: string;
  path: string;
  is_dir: boolean;
}

interface ListResponse {
  path: string;
  entries: DirEntry[];
  count: number;
}

// ============================================================================
// DIRECTORY BROWSER (popover)
// ============================================================================

interface DirBrowserProps {
  onSelect: (path: string) => void;
  onClose: () => void;
  initialPath?: string;
}

const DirBrowser = memo<DirBrowserProps>(({ onSelect, onClose, initialPath }) => {
  const theme = useViewTheme();
  const { t } = useTranslation();
  const [currentPath, setCurrentPath] = useState(initialPath || 'C:\\');
  const [dirs, setDirs] = useState<DirEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const scrollRef = useRef<HTMLDivElement>(null);

  const fetchDirs = useCallback(
    async (path: string) => {
      setLoading(true);
      setError('');
      try {
        const res = await apiPost<ListResponse>('/api/files/list', { path, show_hidden: false });
        // Strip Windows extended-length path prefix (\\?\) from entries
        const clean = (p: string) => p.replace(/^\\\\\?\\/, '');
        setDirs(
          res.entries
            .filter((e) => e.is_dir)
            .map((e) => ({ ...e, path: clean(e.path) }))
            .sort((a, b) => a.name.localeCompare(b.name)),
        );
        setCurrentPath(path);
      } catch {
        setError(t('settings.workingFolder.browseError', 'Cannot read this directory'));
        setDirs([]);
      } finally {
        setLoading(false);
      }
    },
    [t],
  );

  // biome-ignore lint/correctness/useExhaustiveDependencies: intentional mount-only fetch
  useEffect(() => {
    fetchDirs(currentPath);
  }, []);

  const goUp = useCallback(() => {
    // Handle both C:\foo\bar and C:\ root
    const sep = currentPath.includes('/') ? '/' : '\\';
    const parts = currentPath.split(sep).filter(Boolean);
    if (parts.length <= 1) return; // already at root
    parts.pop();
    let parent = parts.join(sep);
    // Windows drive root needs trailing backslash: C:\
    if (parent.length === 2 && parent[1] === ':') parent += '\\';
    fetchDirs(parent);
  }, [currentPath, fetchDirs]);

  // Breadcrumb segments
  const sep = currentPath.includes('/') ? '/' : '\\';
  const segments = currentPath.split(sep).filter(Boolean);

  return (
    <motion.div
      initial={{ opacity: 0, y: 4, scale: 0.97 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, y: 4, scale: 0.97 }}
      transition={{ duration: 0.15 }}
      className={cn(
        'absolute bottom-full left-0 mb-2 z-50',
        'w-[420px] rounded-lg shadow-2xl border border-white/10',
        'backdrop-blur-xl',
        theme.dropdown,
      )}
      onClick={(e) => e.stopPropagation()}
    >
      {/* Header: breadcrumb */}
      <div className="flex items-center gap-1 px-3 py-2 border-b border-white/10 overflow-x-auto scrollbar-hide">
        {segments.map((seg, i) => {
          const pathUpTo = segments.slice(0, i + 1).join(sep) + (i === 0 && seg.endsWith(':') ? sep : '');
          return (
            <span key={pathUpTo} className="flex items-center gap-1 shrink-0">
              {i > 0 && <ChevronRight size={10} className={theme.textMuted} />}
              <button
                type="button"
                onClick={() => fetchDirs(pathUpTo)}
                className={cn(
                  'text-xs font-mono px-1 py-0.5 rounded hover:bg-white/10 transition-colors',
                  i === segments.length - 1 ? 'text-[var(--matrix-accent)]' : theme.textMuted,
                )}
              >
                {seg}
              </button>
            </span>
          );
        })}
      </div>

      {/* Directory list */}
      <div ref={scrollRef} className="max-h-[240px] overflow-y-auto scrollbar-hide">
        {/* Go up */}
        <button
          type="button"
          onClick={goUp}
          disabled={segments.length <= 1}
          className={cn(
            'w-full flex items-center gap-2 px-3 py-1.5 text-xs font-mono transition-colors',
            theme.dropdownItem,
            segments.length <= 1 && 'opacity-30 cursor-not-allowed',
          )}
        >
          <FolderUp size={14} className="text-[var(--matrix-accent)]" />
          ..
        </button>

        {loading && (
          <div className={cn('px-3 py-4 text-xs text-center', theme.textMuted)}>
            {t('common.loading', 'Loading...')}
          </div>
        )}

        {error && <div className="px-3 py-3 text-xs text-red-400 text-center">{error}</div>}

        {!loading && !error && dirs.length === 0 && (
          <div className={cn('px-3 py-3 text-xs text-center italic', theme.textMuted)}>
            {t('settings.workingFolder.noDirs', 'No subdirectories')}
          </div>
        )}

        {!loading &&
          dirs.map((dir) => (
            <button
              key={dir.path}
              type="button"
              onClick={() => fetchDirs(dir.path)}
              className={cn(
                'w-full flex items-center gap-2 px-3 py-1.5 text-xs font-mono transition-colors',
                theme.dropdownItem,
              )}
            >
              <FolderOpen size={14} className="shrink-0 text-[var(--matrix-accent)]/70" />
              <span className="truncate">{dir.name}</span>
            </button>
          ))}
      </div>

      {/* Footer: select / cancel */}
      <div className="flex items-center justify-between px-3 py-2 border-t border-white/10">
        <span className={cn('text-[10px] font-mono truncate max-w-[260px]', theme.textMuted)} title={currentPath}>
          {currentPath}
        </span>
        <div className="flex gap-2">
          <button
            type="button"
            onClick={onClose}
            className={cn('text-xs px-2 py-1 rounded transition-colors', theme.textMuted, 'hover:bg-white/10')}
          >
            {t('common.cancel', 'Cancel')}
          </button>
          <button
            type="button"
            onClick={() => onSelect(currentPath)}
            className="text-xs px-3 py-1 rounded bg-[var(--matrix-accent)]/20 text-[var(--matrix-accent)] hover:bg-[var(--matrix-accent)]/30 transition-colors font-semibold"
          >
            {t('settings.workingFolder.select', 'Select')}
          </button>
        </div>
      </div>
    </motion.div>
  );
});

DirBrowser.displayName = 'DirBrowser';

// ============================================================================
// WORKING FOLDER PICKER (per-session)
// ============================================================================

interface WorkingFolderPickerProps {
  sessionId: string;
  workingDirectory: string;
  onDirectoryChange: (wd: string) => void;
}

export const WorkingFolderPicker = memo<WorkingFolderPickerProps>(
  ({ sessionId, workingDirectory, onDirectoryChange }) => {
    const { t } = useTranslation();
    const theme = useViewTheme();

    const [browsing, setBrowsing] = useState(false);
    const [editing, setEditing] = useState(false);
    const [value, setValue] = useState(workingDirectory);
    const [saving, setSaving] = useState(false);
    const inputRef = useRef<HTMLInputElement>(null);
    const containerRef = useRef<HTMLDivElement>(null);

    const currentFolder = workingDirectory;

    useEffect(() => {
      setValue(workingDirectory);
    }, [workingDirectory]);

    useEffect(() => {
      if (editing) {
        requestAnimationFrame(() => inputRef.current?.focus());
      }
    }, [editing]);

    // Close browser on outside click
    useEffect(() => {
      if (!browsing) return;
      const handler = (e: MouseEvent) => {
        if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
          setBrowsing(false);
        }
      };
      document.addEventListener('mousedown', handler);
      return () => document.removeEventListener('mousedown', handler);
    }, [browsing]);

    const saveFolder = useCallback(
      async (path: string) => {
        setSaving(true);
        try {
          await apiPatch(`/api/sessions/${sessionId}/working-directory`, { working_directory: path });
          onDirectoryChange(path);
          setValue(path);
          setEditing(false);
          setBrowsing(false);
          toast.success(
            path
              ? t('settings.workingFolder.saved', 'Working folder saved')
              : t('settings.workingFolder.cleared', 'Working folder cleared'),
          );
        } catch (err) {
          toast.error(err instanceof Error ? err.message : 'Failed to save');
        } finally {
          setSaving(false);
        }
      },
      [sessionId, onDirectoryChange, t],
    );

    const handleSave = useCallback(() => {
      const trimmed = value.trim();
      if (trimmed === currentFolder) {
        setEditing(false);
        return;
      }
      saveFolder(trimmed);
    }, [value, currentFolder, saveFolder]);

    const handleClear = useCallback(() => saveFolder(''), [saveFolder]);

    const handleCancel = useCallback(() => {
      setValue(currentFolder);
      setEditing(false);
    }, [currentFolder]);

    const handleBrowseSelect = useCallback(
      (path: string) => {
        saveFolder(path);
      },
      [saveFolder],
    );

    // Truncate long paths for display
    const displayPath =
      currentFolder.length > 40
        ? `…${currentFolder.slice(currentFolder.lastIndexOf('\\', currentFolder.length - 20))}`
        : currentFolder;

    return (
      <div ref={containerRef} className="relative">
        <AnimatePresence mode="wait">
          {editing ? (
            <motion.div
              key="edit"
              initial={{ opacity: 0, height: 0 }}
              animate={{ opacity: 1, height: 'auto' }}
              exit={{ opacity: 0, height: 0 }}
              className="flex items-center gap-2 px-1 pb-2"
            >
              <FolderOpen size={14} className="shrink-0 text-[var(--matrix-accent)]" />
              <input
                ref={inputRef}
                type="text"
                value={value}
                onChange={(e) => setValue(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') {
                    e.preventDefault();
                    handleSave();
                  }
                  if (e.key === 'Escape') handleCancel();
                }}
                placeholder="C:\Users\you\project"
                disabled={saving}
                className={cn(
                  'flex-1 text-xs font-mono px-2 py-1 rounded-md bg-transparent',
                  'border border-[var(--matrix-accent)]/30 focus:border-[var(--matrix-accent)]/60',
                  'focus:outline-none transition-colors',
                  theme.text,
                )}
              />
              <button
                type="button"
                onClick={handleSave}
                disabled={saving}
                className="p-1 rounded hover:bg-green-500/20 text-green-400 transition-colors"
                title={t('common.save', 'Save')}
              >
                <Check size={14} />
              </button>
              <button
                type="button"
                onClick={handleCancel}
                disabled={saving}
                className="p-1 rounded hover:bg-red-500/20 text-red-400 transition-colors"
                title={t('common.cancel', 'Cancel')}
              >
                <X size={14} />
              </button>
            </motion.div>
          ) : (
            <motion.div
              key="display"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              className="flex items-center gap-2 px-1 pb-2"
            >
              {currentFolder ? (
                <>
                  <button
                    type="button"
                    onClick={() => setBrowsing((b) => !b)}
                    className="shrink-0 p-1 rounded transition-colors text-[var(--matrix-accent)] hover:bg-[var(--matrix-accent)]/10"
                    title={t('settings.workingFolder.browse', 'Browse folders')}
                  >
                    <FolderOpen size={14} />
                  </button>
                  <span className={cn('text-xs font-mono truncate', theme.textMuted)} title={currentFolder}>
                    {displayPath}
                  </span>
                  <button
                    type="button"
                    onClick={() => setEditing(true)}
                    className={cn(
                      'p-1 rounded transition-colors',
                      theme.textMuted,
                      'hover:text-[var(--matrix-accent)]',
                    )}
                    title={t('settings.workingFolder.change', 'Change')}
                  >
                    <Pencil size={12} />
                  </button>
                  <button
                    type="button"
                    onClick={handleClear}
                    disabled={saving}
                    className={cn('p-1 rounded transition-colors', theme.textMuted, 'hover:text-red-400')}
                    title={t('settings.workingFolder.clear', 'Clear')}
                  >
                    <X size={12} />
                  </button>
                </>
              ) : (
                <button
                  type="button"
                  onClick={() => setBrowsing((b) => !b)}
                  className={cn(
                    'flex items-center gap-2 text-xs font-mono italic transition-colors',
                    theme.textMuted,
                    'hover:text-[var(--matrix-accent)]',
                  )}
                >
                  <FolderOpen size={14} />
                  {t('settings.workingFolder.set', 'Set working folder…')}
                </button>
              )}
            </motion.div>
          )}
        </AnimatePresence>

        {/* Directory browser popover */}
        <AnimatePresence>
          {browsing && (
            <DirBrowser
              onSelect={handleBrowseSelect}
              onClose={() => setBrowsing(false)}
              initialPath={currentFolder || 'C:\\Users'}
            />
          )}
        </AnimatePresence>
      </div>
    );
  },
);

WorkingFolderPicker.displayName = 'WorkingFolderPicker';
