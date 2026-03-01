import { Copy, Trash2, X } from 'lucide-react';
import { memo, useRef } from 'react';
import { useOutsideClick } from '@/shared/hooks/useOutsideClick';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';

interface ChatContextMenuProps {
  x: number;
  y: number;
  isUser: boolean;
  onClose: () => void;
  onCopy: () => void;
  onDelete?: () => void;
}

export const ChatContextMenu = memo<ChatContextMenuProps>(({ x, y, isUser: _isUser, onClose, onCopy, onDelete }) => {
  const menuRef = useRef<HTMLDivElement>(null);
  const theme = useViewTheme();

  useOutsideClick(menuRef, onClose);

  return (
    <div
      ref={menuRef}
      role="menu"
      className={cn('fixed z-50 min-w-[150px] py-1 text-sm', 'rounded-lg shadow-lg', theme.dropdown)}
      style={{ top: y, left: x }}
      onClick={(e) => e.stopPropagation()}
      onKeyDown={(e) => {
        if (e.key === 'Escape') onClose();
      }}
    >
      <button
        type="button"
        onClick={onCopy}
        className={cn('w-full text-left px-4 py-2 flex items-center gap-2 transition-colors', theme.dropdownItem)}
      >
        <Copy size={14} />
        <span>Copy</span>
      </button>

      {onDelete && (
        <>
          <div className={theme.divider} />
          <button
            type="button"
            onClick={onDelete}
            className="w-full text-left px-4 py-2 hover:bg-red-900/30 text-red-400 flex items-center gap-2 transition-colors rounded-lg"
          >
            <Trash2 size={14} />
            <span>Delete</span>
          </button>
        </>
      )}

      <div className={theme.divider} />
      <button
        type="button"
        onClick={onClose}
        className={cn(
          'w-full text-left px-4 py-2 flex items-center gap-2 transition-colors',
          theme.dropdownItem,
          'opacity-60',
        )}
      >
        <X size={14} />
        <span>Cancel</span>
      </button>
    </div>
  );
});

ChatContextMenu.displayName = 'ChatContextMenu';
