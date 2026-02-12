// src/features/history/components/HistoryView.tsx
/**
 * HistoryView - Session browser with search, sort, and management
 * ================================================================
 * Displays past chat sessions in a scrollable list with glass cards.
 * Supports search, sort-by-date, delete with confirmation, and empty state.
 * Ported from GeminiHydra legacy with v15 design system atoms/molecules.
 */

import { ArrowRight, ArrowUpDown, Clock, MessageSquare, Search, Trash2 } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { type ReactNode, useCallback, useMemo, useState } from 'react';

import { Badge, Button, Card, Input } from '@/components/atoms';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';
import { type Message, type Session, selectSortedSessions, useViewStore } from '@/stores/viewStore';

// ============================================================================
// TYPES
// ============================================================================

type SortOrder = 'newest' | 'oldest';

interface SessionDisplayData {
  session: Session;
  messageCount: number;
  previewText: string;
}

// ============================================================================
// HELPERS
// ============================================================================

function formatDate(timestamp: number): string {
  return new Date(timestamp).toLocaleDateString('en-US', {
    day: 'numeric',
    month: 'short',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}

function getPreviewText(messages: Message[]): string {
  if (messages.length === 0) return 'Empty session';
  const lastMsg = messages[messages.length - 1];
  if (!lastMsg) return 'Empty session';
  return lastMsg.content.slice(0, 120) + (lastMsg.content.length > 120 ? '...' : '');
}

// ============================================================================
// SUB-COMPONENTS
// ============================================================================

/** Delete confirmation dialog overlay */
function DeleteConfirmDialog({
  sessionTitle,
  onConfirm,
  onCancel,
}: {
  sessionTitle: string;
  onConfirm: () => void;
  onCancel: () => void;
}): ReactNode {
  const t = useViewTheme();

  return (
    <motion.div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      onClick={onCancel}
    >
      <motion.div
        className={cn('w-full max-w-sm mx-4 rounded-2xl p-6 space-y-4', t.glassPanel)}
        initial={{ scale: 0.9, opacity: 0 }}
        animate={{ scale: 1, opacity: 1 }}
        exit={{ scale: 0.9, opacity: 0 }}
        onClick={(e) => e.stopPropagation()}
      >
        <h3 className={cn('text-base font-bold font-mono', t.title)}>Delete Session</h3>
        <p className={cn('text-sm', t.textMuted)}>
          Are you sure you want to delete{' '}
          <span className={cn('font-semibold', t.text)}>&quot;{sessionTitle}&quot;</span>? This action cannot be undone.
        </p>
        <div className="flex items-center justify-end gap-3 pt-2">
          <Button variant="secondary" size="sm" onClick={onCancel}>
            Cancel
          </Button>
          <Button variant="danger" size="sm" onClick={onConfirm}>
            Delete
          </Button>
        </div>
      </motion.div>
    </motion.div>
  );
}

/** Single session card row */
function SessionCard({
  data,
  isActive,
  onOpen,
  onDelete,
}: {
  data: SessionDisplayData;
  isActive: boolean;
  onOpen: () => void;
  onDelete: () => void;
}): ReactNode {
  const t = useViewTheme();

  return (
    <motion.div
      initial={{ opacity: 0, x: -12 }}
      animate={{ opacity: 1, x: 0 }}
      exit={{ opacity: 0, x: 12, height: 0, marginBottom: 0 }}
      transition={{ duration: 0.2 }}
      layout
    >
      <Card
        variant="hover"
        padding="none"
        interactive
        className={cn(
          'group relative overflow-hidden',
          isActive && (t.isLight ? 'ring-1 ring-emerald-400/40 bg-emerald-50/30' : 'ring-1 ring-white/20 bg-white/5'),
        )}
        onClick={onOpen}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            onOpen();
          }
        }}
      >
        {/* Active indicator bar */}
        {isActive && (
          <div className={cn('absolute left-0 top-0 bottom-0 w-0.5', t.isLight ? 'bg-emerald-500' : 'bg-white')} />
        )}

        <div className="px-4 py-3">
          <div className="flex items-start justify-between gap-3">
            {/* Left: icon, title, preview */}
            <div className="flex-1 min-w-0">
              <div className="flex items-center gap-2">
                <MessageSquare size={14} className={cn(isActive ? t.iconAccent : t.iconMuted, 'flex-shrink-0')} />
                <h3 className={cn('text-sm font-semibold font-mono truncate', isActive ? t.textAccent : t.text)}>
                  {data.session.title}
                </h3>
              </div>

              {data.previewText !== 'Empty session' && (
                <p className={cn('text-xs mt-1 truncate pl-[22px]', t.textMuted)}>{data.previewText}</p>
              )}
            </div>

            {/* Right: meta + actions */}
            <div className="flex items-center gap-2 flex-shrink-0">
              <div className="flex flex-col items-end gap-1">
                <Badge variant="default" size="sm">
                  {data.messageCount} msg
                </Badge>
                <span className={cn('text-[10px] font-mono whitespace-nowrap', t.textMuted)}>
                  {formatDate(data.session.createdAt)}
                </span>
              </div>

              {/* Delete button */}
              <motion.button
                onClick={(e) => {
                  e.stopPropagation();
                  onDelete();
                }}
                className={cn('p-1.5 rounded-lg opacity-0 group-hover:opacity-100 transition-all', t.btnDanger)}
                whileHover={{ scale: 1.1 }}
                whileTap={{ scale: 0.9 }}
                title="Delete session"
                aria-label={`Delete session: ${data.session.title}`}
              >
                <Trash2 size={12} />
              </motion.button>

              {/* Open arrow */}
              <ArrowRight
                size={14}
                className={cn('opacity-0 group-hover:opacity-100 transition-all flex-shrink-0', t.iconAccent)}
              />
            </div>
          </div>
        </div>
      </Card>
    </motion.div>
  );
}

/** Empty state illustration */
function EmptyState({ hasQuery }: { hasQuery: boolean }): ReactNode {
  const t = useViewTheme();

  return (
    <motion.div
      className="flex flex-col items-center justify-center h-60 gap-4"
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
    >
      <div
        className={cn(
          'w-16 h-16 rounded-2xl flex items-center justify-center',
          t.isLight ? 'bg-slate-100 text-slate-300' : 'bg-white/5 text-white/20',
        )}
      >
        <Clock size={32} />
      </div>
      <div className="text-center space-y-1">
        <p className={cn('text-sm font-mono font-semibold', t.textMuted)}>
          {hasQuery ? 'No matching sessions' : 'No sessions yet'}
        </p>
        <p className={cn('text-xs', t.textMuted)}>
          {hasQuery ? 'Try adjusting your search query.' : 'Start a new chat to create your first session.'}
        </p>
      </div>
    </motion.div>
  );
}

// ============================================================================
// MAIN COMPONENT
// ============================================================================

export function HistoryView(): ReactNode {
  const t = useViewTheme();
  const [searchQuery, setSearchQuery] = useState('');
  const [sortOrder, setSortOrder] = useState<SortOrder>('newest');
  const [deleteTarget, setDeleteTarget] = useState<Session | null>(null);

  // Store selectors
  const sortedSessions = useViewStore(selectSortedSessions);
  const chatHistory = useViewStore((s) => s.chatHistory);
  const currentSessionId = useViewStore((s) => s.currentSessionId);
  const selectSession = useViewStore((s) => s.selectSession);
  const deleteSession = useViewStore((s) => s.deleteSession);
  const setCurrentView = useViewStore((s) => s.setCurrentView);

  // Build display data with search filtering and sorting
  const displaySessions = useMemo((): SessionDisplayData[] => {
    const query = searchQuery.trim().toLowerCase();

    const mapped: SessionDisplayData[] = sortedSessions.map((session) => {
      const messages = chatHistory[session.id] ?? [];
      return {
        session,
        messageCount: messages.length,
        previewText: getPreviewText(messages),
      };
    });

    const filtered = query
      ? mapped.filter(
          (d) => d.session.title.toLowerCase().includes(query) || d.previewText.toLowerCase().includes(query),
        )
      : mapped;

    if (sortOrder === 'oldest') {
      return [...filtered].reverse();
    }
    return filtered;
  }, [sortedSessions, chatHistory, searchQuery, sortOrder]);

  const handleOpenSession = useCallback(
    (sessionId: string) => {
      selectSession(sessionId);
      setCurrentView('chat');
    },
    [selectSession, setCurrentView],
  );

  const handleConfirmDelete = useCallback(() => {
    if (deleteTarget) {
      deleteSession(deleteTarget.id);
      setDeleteTarget(null);
    }
  }, [deleteTarget, deleteSession]);

  const toggleSort = useCallback(() => {
    setSortOrder((prev) => (prev === 'newest' ? 'oldest' : 'newest'));
  }, []);

  return (
    <div className="flex flex-col h-full overflow-hidden">
      {/* Header */}
      <div className={cn('px-6 py-4 border-b', t.border)}>
        <div className="flex items-center justify-between">
          <div>
            <h2 className={cn('text-xl font-bold font-mono', t.title)}>Session History</h2>
            <p className={cn('text-sm mt-1 font-mono', t.textMuted)}>
              {sortedSessions.length} {sortedSessions.length === 1 ? 'session' : 'sessions'}
            </p>
          </div>

          {/* Sort toggle */}
          <Button variant="ghost" size="sm" onClick={toggleSort} leftIcon={<ArrowUpDown size={14} />}>
            {sortOrder === 'newest' ? 'Newest first' : 'Oldest first'}
          </Button>
        </div>

        {/* Search bar */}
        <div className="mt-4">
          <Input
            icon={<Search size={14} />}
            placeholder="Search sessions..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            inputSize="sm"
          />
        </div>
      </div>

      {/* Session list */}
      <div className={cn('flex-1 overflow-y-auto p-6 space-y-2', t.scrollbar)}>
        {displaySessions.length === 0 ? (
          <EmptyState hasQuery={searchQuery.trim().length > 0} />
        ) : (
          <AnimatePresence mode="popLayout">
            {displaySessions.map((data) => (
              <SessionCard
                key={data.session.id}
                data={data}
                isActive={data.session.id === currentSessionId}
                onOpen={() => handleOpenSession(data.session.id)}
                onDelete={() => setDeleteTarget(data.session)}
              />
            ))}
          </AnimatePresence>
        )}
      </div>

      {/* Delete confirmation dialog */}
      <AnimatePresence>
        {deleteTarget && (
          <DeleteConfirmDialog
            sessionTitle={deleteTarget.title}
            onConfirm={handleConfirmDelete}
            onCancel={() => setDeleteTarget(null)}
          />
        )}
      </AnimatePresence>
    </div>
  );
}

export default HistoryView;
