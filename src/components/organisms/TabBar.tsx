// src/components/organisms/TabBar.tsx
/**
 * TabBar - Browser-style chat tabs
 * =================================
 * Supports: switching, closing, pinning, middle-click close, new tab button,
 * message count badges, scroll on overflow, glass-panel background.
 * Ported pixel-perfect from GeminiHydra legacy TabBar.tsx.
 *
 * Uses `motion` package (NOT framer-motion).
 */

import { Pin, Plus, X } from 'lucide-react';
import { motion } from 'motion/react';
import { memo, useCallback, useRef, useState } from 'react';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';
import { type ChatTab, useViewStore } from '@/stores/viewStore';

// ============================================================================
// TAB ITEM
// ============================================================================

interface TabItemProps {
  tab: ChatTab;
  isActive: boolean;
  onSwitch: (tabId: string) => void;
  onClose: (tabId: string) => void;
  onTogglePin: (tabId: string) => void;
  messageCount: number;
}

const TabItem = memo<TabItemProps>(({ tab, isActive, onSwitch, onClose, onTogglePin, messageCount }) => {
  const theme = useViewTheme();
  const [isHovering, setIsHovering] = useState(false);

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      // Middle click to close
      if (e.button === 1) {
        e.preventDefault();
        if (!tab.isPinned) onClose(tab.id);
      }
    },
    [tab.id, tab.isPinned, onClose],
  );

  const handleClose = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      onClose(tab.id);
    },
    [tab.id, onClose],
  );

  const handleContextMenu = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      onTogglePin(tab.id);
    },
    [tab.id, onTogglePin],
  );

  return (
    <motion.div
      layout
      layoutId={`tab-${tab.id}`}
      role="tab"
      aria-selected={isActive}
      tabIndex={0}
      onClick={() => onSwitch(tab.id)}
      onMouseDown={handleMouseDown}
      onContextMenu={handleContextMenu}
      onMouseEnter={() => setIsHovering(true)}
      onMouseLeave={() => setIsHovering(false)}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') onSwitch(tab.id);
      }}
      initial={{ opacity: 0, scale: 0.9 }}
      animate={{ opacity: 1, scale: 1 }}
      exit={{ opacity: 0, scale: 0.9 }}
      transition={{ type: 'spring', stiffness: 400, damping: 25 }}
      className={cn(
        'group relative flex items-center gap-2 px-4 py-2.5 cursor-pointer select-none text-sm font-semibold rounded-t-xl transition-all duration-200',
        tab.isPinned ? 'min-w-[48px] max-w-[48px] justify-center' : 'min-w-[140px] max-w-[220px]',
        isActive
          ? theme.isLight
            ? 'bg-white/80 text-black border-b-[3px] border-emerald-500 shadow-md backdrop-blur-sm'
            : 'bg-white/15 text-white border-b-[3px] border-white shadow-lg shadow-white/5 backdrop-blur-sm'
          : theme.isLight
            ? 'bg-white/30 text-gray-700 hover:bg-white/55 hover:text-black border-b-[3px] border-transparent'
            : 'bg-white/[0.06] text-white/50 hover:bg-white/15 hover:text-white border-b-[3px] border-transparent',
      )}
    >
      {/* Pin indicator */}
      {tab.isPinned && (
        <Pin size={13} className={cn('shrink-0', theme.isLight ? 'text-emerald-600' : 'text-white/70')} />
      )}

      {/* Title (hidden for pinned tabs) */}
      {!tab.isPinned && <span className="flex-1 truncate">{tab.title || 'New Chat'}</span>}

      {/* Message count badge */}
      {messageCount > 0 && !tab.isPinned && (
        <span
          className={cn(
            'text-[10px] font-bold px-1.5 py-0.5 rounded-full shrink-0 min-w-[20px] text-center',
            isActive
              ? theme.isLight
                ? 'bg-emerald-500/25 text-emerald-800'
                : 'bg-white/20 text-white'
              : theme.isLight
                ? 'bg-slate-500/15 text-gray-600'
                : 'bg-white/10 text-white/50',
          )}
        >
          {messageCount}
        </span>
      )}

      {/* Close button (hidden for pinned tabs) */}
      {!tab.isPinned && (isHovering || isActive) && (
        <button
          type="button"
          onClick={handleClose}
          className={cn(
            'shrink-0 p-1 rounded-md transition-colors',
            theme.isLight
              ? 'text-gray-400 hover:bg-red-500/25 hover:text-red-600'
              : 'text-white/40 hover:bg-red-500/30 hover:text-red-400',
          )}
          title="Close tab"
        >
          <X size={14} />
        </button>
      )}
    </motion.div>
  );
});

TabItem.displayName = 'TabItem';

// ============================================================================
// TAB BAR
// ============================================================================

export const TabBar = memo(() => {
  const theme = useViewTheme();
  const scrollRef = useRef<HTMLDivElement>(null);

  const tabs = useViewStore((state) => state.tabs);
  const activeTabId = useViewStore((state) => state.activeTabId);
  const chatHistory = useViewStore((state) => state.chatHistory);
  const switchTab = useViewStore((state) => state.switchTab);
  const closeTab = useViewStore((state) => state.closeTab);
  const togglePinTab = useViewStore((state) => state.togglePinTab);
  const createSession = useViewStore((state) => state.createSession);
  const openTab = useViewStore((state) => state.openTab);

  const handleNewTab = useCallback(() => {
    createSession();
    const { currentSessionId } = useViewStore.getState();
    if (currentSessionId) {
      openTab(currentSessionId);
    }
  }, [createSession, openTab]);

  const handleWheel = useCallback((e: React.WheelEvent) => {
    if (scrollRef.current) {
      scrollRef.current.scrollLeft += e.deltaY;
    }
  }, []);

  if (tabs.length === 0) return null;

  return (
    <div
      className={cn(
        'flex items-end gap-1 px-3 pt-2 shrink-0 overflow-hidden border-b-2',
        theme.isLight
          ? 'border-slate-300/50 bg-slate-100/50 backdrop-blur-sm'
          : 'border-white/10 bg-black/40 backdrop-blur-sm',
      )}
      role="tablist"
    >
      {/* Tab scroll container */}
      <div
        ref={scrollRef}
        onWheel={handleWheel}
        className="flex items-end gap-1 overflow-x-auto scrollbar-hide flex-1 min-w-0"
      >
        {tabs.map((tab) => (
          <TabItem
            key={tab.id}
            tab={tab}
            isActive={tab.id === activeTabId}
            onSwitch={switchTab}
            onClose={closeTab}
            onTogglePin={togglePinTab}
            messageCount={(chatHistory[tab.sessionId] || []).length}
          />
        ))}
      </div>

      {/* New tab button */}
      <button
        type="button"
        onClick={handleNewTab}
        className={cn(
          'shrink-0 p-2 mb-1 rounded-xl transition-all',
          theme.isLight
            ? 'text-gray-500 hover:bg-emerald-500/15 hover:text-emerald-700 active:bg-emerald-500/25'
            : 'text-white/50 hover:bg-white/15 hover:text-white active:bg-white/25',
        )}
        title="New tab (Ctrl+T)"
      >
        <Plus size={18} strokeWidth={2.5} />
      </button>
    </div>
  );
});

TabBar.displayName = 'TabBar';
