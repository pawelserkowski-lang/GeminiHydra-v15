import { Check, Edit2, Loader2, LockOpen, MessageSquare, Trash2, X } from 'lucide-react';
import { type KeyboardEvent, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { cn } from '@/shared/utils/cn';
import type { Session } from '@/stores/viewStore';

interface SessionItemProps {
  session: Session;
  isActive: boolean;
  isFocused?: boolean;
  msgCount: number;
  isLight: boolean;
  onSelect: () => void;
  onDelete: () => void;
  onRename: (newTitle: string) => void;
  onUnlock?: () => void;
}

export function SessionItem({
  session,
  isActive,
  isFocused = false,
  msgCount,
  isLight,
  onSelect,
  onDelete,
  onRename,
  onUnlock,
}: SessionItemProps) {
  const { t } = useTranslation();
  const [isEditing, setIsEditing] = useState(false);
  const [editTitle, setEditTitle] = useState(session.title);
  const [confirmDelete, setConfirmDelete] = useState(false);
  // Optimistic session detection (#16) — pending sessions have temporary IDs
  const isPending = session.id.startsWith('pending-');

  useEffect(() => {
    if (!confirmDelete) return;
    const timer = setTimeout(() => setConfirmDelete(false), 3000);
    return () => clearTimeout(timer);
  }, [confirmDelete]);

  const handleSave = () => {
    if (editTitle.trim() && editTitle !== session.title) {
      onRename(editTitle.trim());
    }
    setIsEditing(false);
  };

  const handleCancel = () => {
    setEditTitle(session.title);
    setIsEditing(false);
  };

  const handleDeleteClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (confirmDelete) {
      onDelete();
      setConfirmDelete(false);
    } else {
      setConfirmDelete(true);
    }
  };

  const handleEditKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') handleSave();
    if (e.key === 'Escape') handleCancel();
  };

  const textMuted = isLight ? 'text-slate-700' : 'text-white/80';
  const textDim = isLight ? 'text-slate-600' : 'text-white/50';
  const hoverBg = isLight ? 'hover:bg-black/5' : 'hover:bg-white/5';
  const iconMuted = isLight ? 'text-slate-600' : 'text-white/50';

  if (isEditing) {
    return (
      <div className="flex items-center gap-1 p-1">
        <input
          type="text"
          value={editTitle}
          onChange={(e) => setEditTitle(e.target.value)}
          onKeyDown={handleEditKeyDown}
          className="flex-1 glass-input text-xs py-1 px-2"
          ref={(el) => el?.focus()}
        />
        <button
          type="button"
          onClick={handleSave}
          className={cn('p-1 rounded', isLight ? 'text-emerald-600 hover:bg-black/5' : 'text-white hover:bg-white/15')}
        >
          <Check size={14} />
        </button>
        <button
          type="button"
          onClick={handleCancel}
          className={cn(
            'p-1 rounded',
            isLight ? 'hover:bg-red-500/15 text-red-600' : 'hover:bg-red-500/20 text-red-400',
          )}
        >
          <X size={14} />
        </button>
      </div>
    );
  }

  return (
    <div
      role="option"
      aria-selected={isActive}
      tabIndex={0}
      onClick={onSelect}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          onSelect();
        }
      }}
      aria-label={`Select session: ${session.title}`}
      className={cn(
        'relative w-full flex items-center gap-2 px-2 py-1.5 rounded-lg transition-all duration-200 group text-left cursor-pointer',
        isActive
          ? isLight
            ? 'bg-emerald-500/15 text-emerald-800'
            : 'bg-white/10 text-white'
          : cn(textMuted, hoverBg, isLight ? 'hover:text-slate-900' : 'hover:text-white'),
        isFocused && 'ring-2 ring-[var(--matrix-accent)]/50',
      )}
      title={session.title}
    >
      {isPending ? (
        <Loader2
          size={14}
          className={cn('flex-shrink-0 animate-spin', isLight ? 'text-emerald-600' : 'text-white/60')}
        />
      ) : (
        <MessageSquare
          size={14}
          className={cn(
            'flex-shrink-0 transition-colors',
            isActive ? (isLight ? 'text-emerald-700' : 'text-white') : iconMuted,
          )}
        />
      )}
      <div className="flex-1 min-w-0">
        <span className={cn('text-sm truncate block leading-tight', isPending && 'opacity-60 italic')}>
          {session.title}
        </span>
        <div className="flex items-center gap-1">
          {msgCount > 0 && (
            <span className={cn('text-[10px] font-mono', textDim)}>
              {msgCount} {msgCount === 1 ? t('sidebar.message', 'msg') : t('sidebar.messages', 'msgs')}
            </span>
          )}
          {session.agentId && (
            <span
              className={cn(
                'text-[9px] font-mono px-1 py-0.5 rounded leading-none',
                isLight ? 'bg-emerald-100 text-emerald-700' : 'bg-emerald-500/20 text-emerald-400',
              )}
              title={`Agent: ${session.agentId}`}
            >
              {session.agentId}
            </span>
          )}
        </div>
      </div>
      <div className="flex items-center gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity">
        {session.agentId && onUnlock && (
          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              onUnlock();
            }}
            className={cn(
              'p-1 rounded',
              isLight ? 'hover:bg-amber-500/15 text-amber-600' : 'hover:bg-amber-500/20 text-amber-400',
            )}
            title={t('sidebar.unlockAgent', 'Unlock agent')}
          >
            <LockOpen size={12} />
          </button>
        )}
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            setIsEditing(true);
          }}
          className={cn('p-1 rounded', isLight ? 'hover:bg-black/5' : 'hover:bg-white/15')}
          title={t('sidebar.rename', 'Rename')}
        >
          <Edit2 size={12} />
        </button>
        <button
          type="button"
          onClick={handleDeleteClick}
          className={cn(
            'p-1 rounded transition-colors',
            confirmDelete
              ? isLight
                ? 'bg-red-500/20 text-red-600'
                : 'bg-red-500/30 text-red-300'
              : isLight
                ? 'hover:bg-red-500/15 text-red-600'
                : 'hover:bg-red-500/20 text-red-400',
          )}
          title={confirmDelete ? t('sidebar.confirmDelete', 'Click again to delete') : t('common.delete', 'Delete')}
        >
          <Trash2 size={12} />
        </button>
      </div>
      {isActive && (
        <div
          className={cn(
            'absolute left-0 top-1/2 -translate-y-1/2 w-0.5 h-4 rounded-r-full',
            isLight
              ? 'bg-emerald-600 shadow-[0_0_8px_rgba(5,150,105,0.5)]'
              : 'bg-white shadow-[0_0_8px_rgba(255,255,255,0.5)]',
          )}
        />
      )}
    </div>
  );
}
