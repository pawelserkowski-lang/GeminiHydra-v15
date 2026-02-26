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
import { AnimatePresence, motion } from 'motion/react';
import { memo, useCallback, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';
import { type ChatTab, useViewStore } from '@/stores/viewStore';

// ============================================================================
// TAB ITEM
// ============================================================================

interface TabItemProps {
  tab: ChatTab;
  index: number;
  isActive: boolean;
  onSwitch: (tabId: string) => void;
  onClose: (tabId: string) => void;
  onTogglePin: (tabId: string) => void;
  onContextMenu?: (x: number, y: number, tabId: string) => void;
  onArrowNav?: (tabId: string, direction: 'left' | 'right') => void;
  onDragStart: (index: number) => void;
  onDragOver: (e: React.DragEvent, index: number) => void;
  onDragEnd: () => void;
  isDragOver: boolean;
  isDragging: boolean;
  messageCount: number;
}

const TabItem = memo<TabItemProps>(
  ({
    tab,
    index,
    isActive,
    onSwitch,
    onClose,
    onTogglePin,
    onContextMenu,
    onArrowNav,
    onDragStart,
    onDragOver,
    onDragEnd,
    isDragOver,
    isDragging,
    messageCount,
  }) => {
    const { t } = useTranslation();
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
        if (onContextMenu) {
          onContextMenu(e.clientX, e.clientY, tab.id);
        } else {
          onTogglePin(tab.id);
        }
      },
      [tab.id, onTogglePin, onContextMenu],
    );

    return (
      // biome-ignore lint/a11y/noStaticElementInteractions: drag-and-drop requires div with event handlers
      <div
        draggable
        onDragStart={(e) => {
          e.dataTransfer.setData('text/plain', String(index));
          e.dataTransfer.effectAllowed = 'move';
          onDragStart(index);
        }}
        onDragOver={(e) => {
          e.preventDefault();
          e.dataTransfer.dropEffect = 'move';
          onDragOver(e, index);
        }}
        onDragEnd={onDragEnd}
      >
        <motion.div
          layout
          layoutId={`tab-${tab.id}`}
          data-tab-id={tab.id}
          role="tab"
          aria-selected={isActive}
          aria-label={tab.isPinned ? `Pinned tab: ${tab.title || 'New Chat'}` : tab.title || 'New Chat'}
          tabIndex={isActive ? 0 : -1}
          onClick={() => onSwitch(tab.id)}
          onMouseDown={handleMouseDown}
          onContextMenu={handleContextMenu}
          onMouseEnter={() => setIsHovering(true)}
          onMouseLeave={() => setIsHovering(false)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' || e.key === ' ') {
              e.preventDefault();
              onSwitch(tab.id);
            }
            if (e.key === 'ArrowLeft') {
              e.preventDefault();
              onArrowNav?.(tab.id, 'left');
            }
            if (e.key === 'ArrowRight') {
              e.preventDefault();
              onArrowNav?.(tab.id, 'right');
            }
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
            // Drag visual indicators (#14)
            isDragging && 'opacity-50',
            isDragOver && 'border-l-2 border-l-[var(--matrix-accent)]',
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
              title={t('chat.closeTab', 'Close tab')}
            >
              <X size={14} />
            </button>
          )}
        </motion.div>
      </div>
    );
  },
);

TabItem.displayName = 'TabItem';

// ============================================================================
// TAB BAR
// ============================================================================

export const TabBar = memo(() => {
  const { t } = useTranslation();
  const theme = useViewTheme();
  const scrollRef = useRef<HTMLDivElement>(null);

  const tabs = useViewStore((state) => state.tabs);
  const activeTabId = useViewStore((state) => state.activeTabId);
  const chatHistory = useViewStore((state) => state.chatHistory);
  const switchTab = useViewStore((state) => state.switchTab);
  const closeTab = useViewStore((state) => state.closeTab);
  const togglePinTab = useViewStore((state) => state.togglePinTab);
  const reorderTabs = useViewStore((state) => state.reorderTabs);
  const createSession = useViewStore((state) => state.createSession);
  const openTab = useViewStore((state) => state.openTab);

  // Context menu state
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; tabId: string } | null>(null);

  // Drag & drop state (#14)
  const [dragFromIndex, setDragFromIndex] = useState<number | null>(null);
  const [dragOverIndex, setDragOverIndex] = useState<number | null>(null);

  const handleTabDragStart = useCallback((index: number) => {
    setDragFromIndex(index);
  }, []);

  const handleTabDragOver = useCallback((e: React.DragEvent, index: number) => {
    e.preventDefault();
    setDragOverIndex(index);
  }, []);

  const handleTabDragEnd = useCallback(() => {
    if (dragFromIndex !== null && dragOverIndex !== null && dragFromIndex !== dragOverIndex) {
      reorderTabs(dragFromIndex, dragOverIndex);
    }
    setDragFromIndex(null);
    setDragOverIndex(null);
  }, [dragFromIndex, dragOverIndex, reorderTabs]);

  const handleContextMenuOpen = useCallback((x: number, y: number, tabId: string) => {
    setContextMenu({ x, y, tabId });
  }, []);

  const handleCloseOtherTabs = useCallback(
    (tabId: string) => {
      const otherTabs = tabs.filter((t) => t.id !== tabId && !t.isPinned);
      for (const tab of otherTabs) {
        closeTab(tab.id);
      }
      setContextMenu(null);
    },
    [tabs, closeTab],
  );

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

  const handleArrowNav = useCallback(
    (tabId: string, direction: 'left' | 'right') => {
      const currentIndex = tabs.findIndex((t) => t.id === tabId);
      if (currentIndex === -1) return;
      const nextIndex =
        direction === 'left' ? (currentIndex - 1 + tabs.length) % tabs.length : (currentIndex + 1) % tabs.length;
      const nextTab = tabs[nextIndex];
      if (!nextTab) return;
      switchTab(nextTab.id);
      // Focus the newly active tab element
      requestAnimationFrame(() => {
        const el = scrollRef.current?.querySelector<HTMLElement>(`[data-tab-id="${nextTab.id}"]`);
        el?.focus();
      });
    },
    [tabs, switchTab],
  );

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
        <AnimatePresence mode="popLayout">
          {tabs.map((tab, index) => (
            <TabItem
              key={tab.id}
              tab={tab}
              index={index}
              isActive={tab.id === activeTabId}
              onSwitch={switchTab}
              onClose={closeTab}
              onTogglePin={togglePinTab}
              onContextMenu={handleContextMenuOpen}
              onArrowNav={handleArrowNav}
              onDragStart={handleTabDragStart}
              onDragOver={handleTabDragOver}
              onDragEnd={handleTabDragEnd}
              isDragOver={dragOverIndex === index}
              isDragging={dragFromIndex === index}
              messageCount={(chatHistory[tab.sessionId] || []).length}
            />
          ))}
        </AnimatePresence>
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
        title={t('chat.newTab', 'New tab (Ctrl+T)')}
        aria-label={t('chat.newTab', 'New tab')}
      >
        <Plus size={18} strokeWidth={2.5} />
      </button>

      {/* Context Menu Popup */}
      <AnimatePresence>
        {contextMenu &&
          (() => {
            const targetTab = tabs.find((t) => t.id === contextMenu.tabId);
            if (!targetTab) return null;
            return (
              <>
                {/* Backdrop — intentionally a non-interactive overlay */}
                {/* biome-ignore lint/a11y/noStaticElementInteractions: backdrop overlay dismiss */}
                <div
                  role="presentation"
                  className="fixed inset-0 z-50"
                  onClick={() => setContextMenu(null)}
                  onKeyDown={(e) => {
                    if (e.key === 'Escape') setContextMenu(null);
                  }}
                />
                {/* Menu */}
                <motion.div
                  initial={{ opacity: 0, scale: 0.95 }}
                  animate={{ opacity: 1, scale: 1 }}
                  exit={{ opacity: 0, scale: 0.95 }}
                  transition={{ duration: 0.1 }}
                  className={cn(
                    'fixed z-50 min-w-[180px] rounded-xl border backdrop-blur-xl shadow-2xl overflow-hidden py-1',
                    theme.isLight ? 'bg-white/95 border-slate-200/50' : 'bg-black/90 border-white/15',
                  )}
                  style={{ left: contextMenu.x, top: contextMenu.y }}
                >
                  {/* Pin/Unpin */}
                  <button
                    type="button"
                    onClick={() => {
                      togglePinTab(contextMenu.tabId);
                      setContextMenu(null);
                    }}
                    className={cn(
                      'w-full flex items-center gap-2 px-3 py-2 text-sm font-mono transition-colors',
                      theme.isLight
                        ? 'text-slate-700 hover:bg-emerald-500/10 hover:text-emerald-800'
                        : 'text-white/80 hover:bg-white/10 hover:text-white',
                    )}
                  >
                    <Pin size={14} />
                    {targetTab.isPinned ? 'Unpin tab' : 'Pin tab'}
                  </button>

                  {/* Close tab (if not pinned) */}
                  {!targetTab.isPinned && (
                    <button
                      type="button"
                      onClick={() => {
                        closeTab(contextMenu.tabId);
                        setContextMenu(null);
                      }}
                      className={cn(
                        'w-full flex items-center gap-2 px-3 py-2 text-sm font-mono transition-colors',
                        theme.isLight
                          ? 'text-slate-700 hover:bg-red-500/10 hover:text-red-600'
                          : 'text-white/80 hover:bg-red-500/15 hover:text-red-400',
                      )}
                    >
                      <X size={14} />
                      Close tab
                    </button>
                  )}

                  {/* Close other tabs */}
                  {tabs.filter((t) => t.id !== contextMenu.tabId && !t.isPinned).length > 0 && (
                    <>
                      <div
                        className={cn('mx-2 my-1 border-t', theme.isLight ? 'border-slate-200/50' : 'border-white/10')}
                      />
                      <button
                        type="button"
                        onClick={() => handleCloseOtherTabs(contextMenu.tabId)}
                        className={cn(
                          'w-full flex items-center gap-2 px-3 py-2 text-sm font-mono transition-colors',
                          theme.isLight
                            ? 'text-slate-700 hover:bg-slate-500/10 hover:text-slate-900'
                            : 'text-white/80 hover:bg-white/10 hover:text-white',
                        )}
                      >
                        <X size={14} />
                        Close other tabs
                      </button>
                    </>
                  )}
                </motion.div>
              </>
            );
          })()}
      </AnimatePresence>
    </div>
  );
});

TabBar.displayName = 'TabBar';
