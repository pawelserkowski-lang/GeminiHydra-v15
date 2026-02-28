// src/components/organisms/Sidebar.tsx
/**
 * Navigation Sidebar - Tissaia Style
 * ====================================
 * Collapsible glass sidebar with grouped navigation, chat sessions list,
 * theme/language toggles, and version display.
 * Ported pixel-perfect from GeminiHydra legacy Sidebar.tsx.
 *
 * Uses `motion` package (NOT framer-motion).
 */

import {
  Check,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  Edit2,
  ExternalLink,
  Home,
  Loader2,
  type LucideIcon,
  MessageSquare,
  Plus,
  Settings,
  Sparkles,
  Swords,
  Trash2,
  WifiOff,
  X,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import {
  type KeyboardEvent,
  lazy,
  type TouchEvent as ReactTouchEvent,
  Suspense,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { useTranslation } from 'react-i18next';
import { useTheme } from '@/contexts/ThemeContext';

const PartnerChatModal = lazy(() => import('@/features/chat/components/PartnerChatModal'));

import { SessionSearch } from '@/components/molecules/SessionSearch';
import { usePartnerSessions } from '@/features/chat/hooks/usePartnerSessions';
import { useSessionSync } from '@/features/chat/hooks/useSessionSync';
import { cn } from '@/shared/utils/cn';
import { type Session, useViewStore, type View } from '@/stores/viewStore';
import { FooterControls } from './sidebar/FooterControls';
import { LogoButton } from './sidebar/LogoButton';

// ============================================
// TYPES
// ============================================

interface NavItem {
  id: View;
  icon: LucideIcon;
  label: string;
}

interface NavGroup {
  id: string;
  label: string;
  icon: LucideIcon;
  items: NavItem[];
}

// ============================================
// SESSION ITEM SUB-COMPONENT
// ============================================

interface SessionItemProps {
  session: Session;
  isActive: boolean;
  isFocused?: boolean;
  msgCount: number;
  isLight: boolean;
  onSelect: () => void;
  onDelete: () => void;
  onRename: (newTitle: string) => void;
}

function SessionItem({
  session,
  isActive,
  isFocused = false,
  msgCount,
  isLight,
  onSelect,
  onDelete,
  onRename,
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
        {msgCount > 0 && (
          <span className={cn('text-[10px] font-mono', textDim)}>
            {msgCount} {msgCount === 1 ? t('sidebar.message', 'msg') : t('sidebar.messages', 'msgs')}
          </span>
        )}
      </div>
      <div className="flex items-center gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity">
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

// ============================================
// SIDEBAR COMPONENT
// ============================================

export function Sidebar() {
  const { t } = useTranslation();
  const { resolvedTheme } = useTheme();

  // View store
  const currentView = useViewStore((s) => s.currentView);
  const setCurrentView = useViewStore((s) => s.setCurrentView);
  const chatHistory = useViewStore((s) => s.chatHistory);
  const sidebarCollapsed = useViewStore((s) => s.sidebarCollapsed);
  const toggleSidebar = useViewStore((s) => s.toggleSidebar);

  // Session sync (DB + localStorage)
  const {
    sessions,
    currentSessionId,
    selectSession,
    createSessionWithSync,
    deleteSessionWithSync,
    renameSessionWithSync,
  } = useSessionSync();

  // Partner sessions (ClaudeHydra)
  const { data: partnerSessions, isLoading: partnerLoading, isError: partnerError } = usePartnerSessions();
  const [showPartnerSessions, setShowPartnerSessions] = useState(true);
  const [partnerModalSessionId, setPartnerModalSessionId] = useState<string | null>(null);

  // Session search/filter (#19)
  const [sessionSearchQuery, setSessionSearchQuery] = useState('');
  const handleSessionSearch = useCallback((query: string) => {
    setSessionSearchQuery(query);
  }, []);

  // Sessions sorted by creation date (newest first), then filtered by search
  const sortedSessions = useMemo(() => {
    const sorted = [...sessions].sort((a, b) => b.createdAt - a.createdAt);
    if (!sessionSearchQuery) return sorted;
    return sorted.filter((s) => s.title.toLowerCase().includes(sessionSearchQuery));
  }, [sessions, sessionSearchQuery]);
  const sortedPartnerSessions = useMemo(
    () =>
      [...(partnerSessions ?? [])].sort((a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime()),
    [partnerSessions],
  );

  // Mobile drawer state
  const [mobileOpen, setMobileOpen] = useState(false);

  // Swipe gesture refs (#29)
  const touchStartXRef = useRef(0);
  const touchStartYRef = useRef(0);
  const touchCurrentXRef = useRef(0);
  const isSwiping = useRef(false);

  // Auto-collapse sidebar on mobile resize (#29)
  useEffect(() => {
    const handleResize = () => {
      if (window.innerWidth < 768) {
        // On mobile: close mobile drawer when resizing to mobile
        setMobileOpen(false);
        // Auto-collapse the desktop sidebar state too
        if (!sidebarCollapsed) {
          useViewStore.getState().setSidebarCollapsed(true);
        }
      }
    };

    window.addEventListener('resize', handleResize);
    // Also check on mount
    if (window.innerWidth < 768 && !sidebarCollapsed) {
      useViewStore.getState().setSidebarCollapsed(true);
    }
    return () => window.removeEventListener('resize', handleResize);
  }, [sidebarCollapsed]);

  // Swipe right to open sidebar on mobile (#29)
  useEffect(() => {
    const SWIPE_THRESHOLD = 50;
    const EDGE_ZONE = 30; // pixels from left edge to trigger swipe

    const handleTouchStart = (e: TouchEvent) => {
      if (window.innerWidth >= 768) return;
      const touch = e.touches[0];
      if (!touch) return;

      touchStartXRef.current = touch.clientX;
      touchStartYRef.current = touch.clientY;
      touchCurrentXRef.current = touch.clientX;

      // Only start swipe tracking from left edge when sidebar is closed
      if (!mobileOpen && touch.clientX <= EDGE_ZONE) {
        isSwiping.current = true;
      }
    };

    const handleTouchMove = (e: TouchEvent) => {
      if (window.innerWidth >= 768) return;
      const touch = e.touches[0];
      if (!touch) return;
      touchCurrentXRef.current = touch.clientX;
    };

    const handleTouchEnd = () => {
      if (window.innerWidth >= 768) return;

      const deltaX = touchCurrentXRef.current - touchStartXRef.current;
      const deltaY = Math.abs(touchCurrentXRef.current - touchStartYRef.current);

      // Only process horizontal swipes (not vertical scrolling)
      if (Math.abs(deltaX) < deltaY) {
        isSwiping.current = false;
        return;
      }

      // Swipe right from edge to open
      if (!mobileOpen && isSwiping.current && deltaX > SWIPE_THRESHOLD) {
        setMobileOpen(true);
      }

      isSwiping.current = false;
    };

    document.addEventListener('touchstart', handleTouchStart, { passive: true });
    document.addEventListener('touchmove', handleTouchMove, { passive: true });
    document.addEventListener('touchend', handleTouchEnd, { passive: true });
    return () => {
      document.removeEventListener('touchstart', handleTouchStart);
      document.removeEventListener('touchmove', handleTouchMove);
      document.removeEventListener('touchend', handleTouchEnd);
    };
  }, [mobileOpen]);

  // Swipe left on the sidebar drawer itself to close (#29)
  const handleDrawerTouchStart = useCallback((e: ReactTouchEvent) => {
    const touch = e.touches[0];
    if (!touch) return;
    touchStartXRef.current = touch.clientX;
    touchCurrentXRef.current = touch.clientX;
  }, []);

  const handleDrawerTouchMove = useCallback((e: ReactTouchEvent) => {
    const touch = e.touches[0];
    if (!touch) return;
    touchCurrentXRef.current = touch.clientX;
  }, []);

  const handleDrawerTouchEnd = useCallback(() => {
    const deltaX = touchCurrentXRef.current - touchStartXRef.current;
    const SWIPE_THRESHOLD = 50;
    // Swipe left to close
    if (deltaX < -SWIPE_THRESHOLD) {
      setMobileOpen(false);
    }
  }, []);

  // Collapsible sessions toggle
  const [showSessions, setShowSessions] = useState(true);

  const handleNewChat = useCallback(() => {
    void createSessionWithSync();
    setCurrentView('chat');
    setMobileOpen(false);
  }, [createSessionWithSync, setCurrentView]);

  const handleSelectSession = useCallback(
    (id: string) => {
      selectSession(id);
      setCurrentView('chat');
      setMobileOpen(false);
    },
    [selectSession, setCurrentView],
  );

  const handleDeleteSession = useCallback(
    (id: string) => {
      void deleteSessionWithSync(id);
    },
    [deleteSessionWithSync],
  );

  const handleRenameSession = useCallback(
    (id: string, newTitle: string) => {
      void renameSessionWithSync(id, newTitle);
    },
    [renameSessionWithSync],
  );

  // #42 — Keyboard navigation for session list
  const [focusedSessionIndex, setFocusedSessionIndex] = useState(-1);

  const handleSessionListKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setFocusedSessionIndex((i) => (i + 1) % sortedSessions.length);
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        setFocusedSessionIndex((i) => (i - 1 + sortedSessions.length) % sortedSessions.length);
      } else if (e.key === 'Enter' && focusedSessionIndex >= 0 && sortedSessions[focusedSessionIndex]) {
        e.preventDefault();
        handleSelectSession(sortedSessions[focusedSessionIndex].id);
      }
    },
    [sortedSessions, focusedSessionIndex, handleSelectSession],
  );

  const handleNavClick = useCallback(
    (view: View) => {
      setCurrentView(view);
      setMobileOpen(false);
    },
    [setCurrentView],
  );

  // Navigation groups adapted for GeminiHydra v15 (Tissaia style)
  const navGroups: NavGroup[] = [
    {
      id: 'main',
      label: t('sidebar.groups.main', 'MAIN'),
      icon: Sparkles,
      items: [
        { id: 'home', icon: Home, label: t('nav.home', 'Start') },
        { id: 'chat', icon: MessageSquare, label: t('nav.chat', 'Chat') },
        { id: 'settings', icon: Settings, label: t('nav.settings', 'Settings') },
      ],
    },
  ];

  // Track expanded groups
  const [expandedGroups, setExpandedGroups] = useState<Record<string, boolean>>(() => {
    const defaults = { main: true };
    try {
      const saved = localStorage.getItem('geminihydra_expanded_groups');
      return saved ? { ...defaults, ...JSON.parse(saved) } : defaults;
    } catch {
      return defaults;
    }
  });

  useEffect(() => {
    try {
      localStorage.setItem('geminihydra_expanded_groups', JSON.stringify(expandedGroups));
    } catch {
      /* ignore */
    }
  }, [expandedGroups]);

  const toggleGroup = (groupId: string) => {
    setExpandedGroups((prev) => ({ ...prev, [groupId]: !prev[groupId] }));
  };

  const isLight = resolvedTheme === 'light';
  const glassPanel = isLight ? 'glass-panel-light' : 'glass-panel-dark';

  // Light-mode text classes for better readability
  const textMuted = isLight ? 'text-slate-700' : 'text-white/80';
  const textDim = isLight ? 'text-slate-600' : 'text-white/50';
  const textHover = isLight ? 'hover:text-slate-900' : 'hover:text-white';
  const iconMuted = isLight ? 'text-slate-600' : 'text-white/50';
  const iconHover = isLight ? 'group-hover:text-emerald-700' : 'group-hover:text-white';
  const hoverBg = isLight ? 'hover:bg-black/5' : 'hover:bg-white/5';
  const collapseBtn = isLight
    ? 'bg-white/90 border-emerald-600/40 hover:bg-emerald-50 hover:border-emerald-500 hover:shadow-[0_0_12px_rgba(5,150,105,0.3)]'
    : 'bg-black/60 border-white/20 hover:bg-white/20 hover:border-white/50 hover:shadow-[0_0_12px_rgba(255,255,255,0.15)]';
  const collapseIcon = isLight ? 'text-emerald-700' : 'text-white';

  // ========================================
  // SIDEBAR CONTENT (shared between desktop & mobile)
  // ========================================
  const sidebarContent = (
    <div
      className={cn(
        'h-full flex flex-col z-20 relative p-2 gap-2 overflow-y-auto scrollbar-hide hover:scrollbar-thin hover:scrollbar-thumb-current',
        glassPanel,
      )}
    >
      {/* Collapse Toggle Button (desktop only) */}
      <button
        type="button"
        data-testid="btn-sidebar-collapse"
        onClick={toggleSidebar}
        className={cn(
          'absolute -right-4 top-1/2 -translate-y-1/2 z-30 hidden md:flex items-center justify-center w-9 h-9 border rounded-full shadow-lg backdrop-blur-sm transition-all duration-200 hover:scale-110 active:scale-95',
          collapseBtn,
        )}
        title={
          sidebarCollapsed
            ? t('sidebar.expandSidebar', 'Expand sidebar')
            : t('sidebar.collapseSidebar', 'Collapse sidebar')
        }
        aria-label={
          sidebarCollapsed
            ? t('sidebar.expandSidebar', 'Expand sidebar')
            : t('sidebar.collapseSidebar', 'Collapse sidebar')
        }
      >
        {sidebarCollapsed ? (
          <ChevronRight size={18} strokeWidth={2.5} className={collapseIcon} />
        ) : (
          <ChevronLeft size={18} strokeWidth={2.5} className={collapseIcon} />
        )}
      </button>

      {/* Logo — click navigates to home */}
      <LogoButton collapsed={sidebarCollapsed} onClick={() => handleNavClick('home')} />

      {/* Grouped Navigation */}
      <nav className="flex flex-col gap-2 flex-shrink-0">
        {navGroups.map((group) => {
          const isExpanded = expandedGroups[group.id];
          const hasActiveItem = group.items.some((item) => item.id === currentView);
          const GroupIcon = group.icon;

          return (
            <div key={group.id} className={cn(glassPanel, 'overflow-hidden')}>
              {/* Group Header */}
              {!sidebarCollapsed ? (
                <button
                  type="button"
                  onClick={() => toggleGroup(group.id)}
                  aria-expanded={isExpanded}
                  aria-label={`${isExpanded ? 'Collapse' : 'Expand'} ${group.label} group`}
                  className={cn(
                    'w-full flex items-center justify-between px-3 py-2.5 transition-all group',
                    hasActiveItem
                      ? isLight
                        ? 'text-emerald-700 bg-emerald-500/10'
                        : 'text-white bg-white/5'
                      : cn(textMuted, textHover, hoverBg),
                  )}
                >
                  <div className="flex items-center gap-2">
                    <GroupIcon size={14} />
                    <span className="text-sm font-bold tracking-[0.12em] uppercase">{group.label}</span>
                  </div>
                  <ChevronDown
                    size={14}
                    className={cn('transition-transform duration-200', isExpanded ? '' : '-rotate-90')}
                  />
                </button>
              ) : null}

              {/* Group Items */}
              <div
                className={cn(
                  'px-1.5 pb-1.5 space-y-0.5 overflow-hidden transition-all duration-200',
                  !sidebarCollapsed && !isExpanded ? 'max-h-0 opacity-0 pb-0' : 'max-h-96 opacity-100',
                  sidebarCollapsed ? 'py-1.5' : '',
                )}
              >
                {group.items.map((item) => (
                  <button
                    type="button"
                    key={item.id}
                    data-testid={`nav-${item.id}`}
                    onClick={() => handleNavClick(item.id)}
                    className={cn(
                      'relative w-full flex items-center px-3 py-2 rounded-lg transition-all duration-200 group',
                      sidebarCollapsed ? 'justify-center' : 'space-x-3',
                      currentView === item.id
                        ? isLight
                          ? 'bg-emerald-500/15 text-emerald-800'
                          : 'bg-white/10 text-white'
                        : cn(textMuted, hoverBg, textHover),
                    )}
                    title={sidebarCollapsed ? item.label : undefined}
                    aria-label={`Navigate to ${item.label}`}
                  >
                    <item.icon
                      size={16}
                      className={cn(
                        'transition-colors flex-shrink-0',
                        currentView === item.id
                          ? isLight
                            ? 'text-emerald-700'
                            : 'text-white'
                          : cn(iconMuted, iconHover),
                      )}
                    />
                    {!sidebarCollapsed && (
                      <span className="font-medium text-base tracking-wide truncate">{item.label}</span>
                    )}
                    {currentView === item.id && (
                      <div
                        className={cn(
                          'absolute left-0 top-1/2 -translate-y-1/2 w-0.5 h-5 rounded-r-full',
                          isLight
                            ? 'bg-emerald-600 shadow-[0_0_8px_rgba(5,150,105,0.5)]'
                            : 'bg-white shadow-[0_0_8px_rgba(255,255,255,0.4)]',
                        )}
                      />
                    )}
                  </button>
                ))}
              </div>
            </div>
          );
        })}
      </nav>

      {/* Chat Sessions (Tissaia style — glass panel + collapsible) */}
      {!sidebarCollapsed && (
        <div className={cn(glassPanel, 'flex-1 flex flex-col min-h-0 p-2 overflow-hidden')}>
          {/* Section Header */}
          <div className="flex items-center justify-between px-1 py-1.5">
            <button
              type="button"
              onClick={() => setShowSessions(!showSessions)}
              aria-expanded={showSessions}
              aria-label={`${showSessions ? 'Collapse' : 'Expand'} chat sessions`}
              className={cn('flex items-center gap-2 transition-colors', textDim, textHover)}
            >
              <MessageSquare size={14} />
              <span className="text-sm font-bold tracking-[0.12em] uppercase">{t('sidebar.chats', 'CHATS')}</span>
              <ChevronDown
                size={14}
                className={cn('transition-transform duration-200', showSessions ? '' : '-rotate-90')}
              />
            </button>
            <div className="flex items-center gap-1">
              <span className={cn('text-xs', textDim)}>{sessions.length}</span>
              <button
                type="button"
                onClick={handleNewChat}
                className={cn('p-1 rounded-md transition-all', hoverBg)}
                title={t('sidebar.newChat', 'New chat')}
              >
                <Plus
                  size={14}
                  className={cn(
                    iconMuted,
                    'transition-colors',
                    isLight ? 'hover:text-emerald-700' : 'hover:text-white',
                  )}
                />
              </button>
            </div>
          </div>

          {/* Session search (#19) */}
          {showSessions && sessions.length > 3 && <SessionSearch onSearch={handleSessionSearch} isLight={isLight} />}

          {/* Session List (#42 — keyboard nav with role=listbox) */}
          <AnimatePresence>
            {showSessions && (
              <motion.div
                initial={{ height: 0, opacity: 0 }}
                animate={{ height: 'auto', opacity: 1 }}
                exit={{ height: 0, opacity: 0 }}
                transition={{ duration: 0.2, ease: 'easeInOut' }}
                role="listbox"
                aria-label={t('sidebar.chats', 'Chat sessions')}
                onKeyDown={handleSessionListKeyDown}
                className="flex-1 overflow-y-auto scrollbar-hide hover:scrollbar-thin hover:scrollbar-thumb-current space-y-0.5 mt-1"
              >
                {sortedSessions.length === 0 && sessionSearchQuery ? (
                  <p className={cn('text-[10px] text-center py-3', textDim)}>{t('sidebar.noResults', 'No results')}</p>
                ) : (
                  sortedSessions.map((session, idx) => (
                    <SessionItem
                      key={session.id}
                      session={session}
                      isActive={session.id === currentSessionId}
                      isFocused={focusedSessionIndex === idx}
                      msgCount={(chatHistory[session.id] || []).length}
                      isLight={isLight}
                      onSelect={() => handleSelectSession(session.id)}
                      onDelete={() => handleDeleteSession(session.id)}
                      onRename={(newTitle) => handleRenameSession(session.id, newTitle)}
                    />
                  ))
                )}
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      )}

      {/* Partner Sessions — ClaudeHydra (glass panel + collapsible) */}
      {!sidebarCollapsed && (
        <div className={cn(glassPanel, 'flex flex-col min-h-0 p-2 overflow-hidden')}>
          <div className="flex items-center justify-between px-1 py-1.5">
            <button
              type="button"
              onClick={() => setShowPartnerSessions(!showPartnerSessions)}
              aria-expanded={showPartnerSessions}
              aria-label={`${showPartnerSessions ? 'Collapse' : 'Expand'} ClaudeHydra partner sessions`}
              className={cn('flex items-center gap-2 transition-colors', textDim, textHover)}
            >
              <div
                className={cn(
                  'w-5 h-5 rounded flex items-center justify-center text-[9px] font-bold flex-shrink-0',
                  isLight ? 'bg-orange-100 text-orange-700' : 'bg-orange-500/20 text-orange-400',
                )}
              >
                CH
              </div>
              <span className="text-sm font-bold tracking-[0.12em] uppercase">
                {t('sidebar.partner', 'ClaudeHydra')}
              </span>
              <ChevronDown
                size={14}
                className={cn('transition-transform duration-200', showPartnerSessions ? '' : '-rotate-90')}
              />
            </button>
            <div className="flex items-center gap-1">
              {partnerLoading && <Loader2 size={12} className="animate-spin text-orange-400" />}
              {partnerError && <WifiOff size={12} className={isLight ? 'text-slate-400' : 'text-white/30'} />}
              {!partnerLoading && !partnerError && (
                <span className={cn('text-xs', textDim)}>{sortedPartnerSessions.length}</span>
              )}
            </div>
          </div>

          <AnimatePresence>
            {showPartnerSessions && (
              <motion.div
                initial={{ height: 0, opacity: 0 }}
                animate={{ height: 'auto', opacity: 1 }}
                exit={{ height: 0, opacity: 0 }}
                transition={{ duration: 0.2, ease: 'easeInOut' }}
                className="flex-1 overflow-y-auto scrollbar-hide hover:scrollbar-thin hover:scrollbar-thumb-current space-y-0.5 mt-1"
              >
                {partnerError && <p className={cn('text-[10px] text-center py-2', textDim)}>Offline</p>}
                {!partnerError && sortedPartnerSessions.length === 0 && !partnerLoading && (
                  <p className={cn('text-[10px] text-center py-2', textDim)}>
                    {t('sidebar.noSessions', 'No sessions')}
                  </p>
                )}
                {sortedPartnerSessions.map((ps) => (
                  <button
                    type="button"
                    key={ps.id}
                    onClick={() => setPartnerModalSessionId(ps.id)}
                    className={cn(
                      'relative w-full flex items-center gap-2 px-2 py-1.5 rounded-lg transition-all duration-200 group text-left',
                      textMuted,
                      hoverBg,
                      textHover,
                    )}
                    title={ps.title}
                  >
                    <MessageSquare
                      size={14}
                      className={cn(
                        'flex-shrink-0 transition-colors',
                        isLight ? 'text-orange-500' : 'text-orange-400/60',
                      )}
                    />
                    <div className="flex-1 min-w-0">
                      <span className="text-sm truncate block leading-tight">{ps.title}</span>
                      <span className={cn('text-[10px] font-mono', textDim)}>
                        {ps.message_count} {ps.message_count === 1 ? 'msg' : 'msgs'}
                      </span>
                    </div>
                    <ExternalLink
                      size={10}
                      className={cn('opacity-0 group-hover:opacity-60 transition-opacity flex-shrink-0', iconMuted)}
                    />
                  </button>
                ))}
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      )}

      {/* Collapsed: New Chat icon */}
      {sidebarCollapsed && (
        <div className="flex flex-col items-center gap-1 flex-1">
          <button
            type="button"
            onClick={handleNewChat}
            className={cn('p-2 rounded-lg transition-all', hoverBg)}
            title={t('sidebar.newChat', 'New chat')}
          >
            <Plus
              size={18}
              className={cn(iconMuted, isLight ? 'hover:text-emerald-700' : 'hover:text-white', 'transition-colors')}
            />
          </button>
        </div>
      )}

      {/* Footer / Lang & Theme Toggle + Version */}
      <FooterControls
        collapsed={sidebarCollapsed}
        version="GeminiHydra v15.0.0"
        tagline={t('footer.tagline', 'Wolf Swarm')}
      />
    </div>
  );

  return (
    <>
      {/* Desktop Sidebar */}
      <motion.aside
        className={cn('shrink-0 h-full hidden md:flex transition-none')}
        animate={{ width: sidebarCollapsed ? 64 : 240 }}
        transition={{ type: 'spring', stiffness: 300, damping: 30 }}
      >
        {sidebarContent}
      </motion.aside>

      {/* Mobile: Hamburger trigger (rendered outside for AppShell) */}
      <button
        type="button"
        onClick={() => setMobileOpen(true)}
        className={cn('md:hidden fixed top-3 left-3 z-50 p-2 rounded-lg', glassPanel)}
        aria-label={t('sidebar.openSidebar', 'Open sidebar')}
      >
        <Swords size={20} className={isLight ? 'text-emerald-700' : 'text-white'} />
      </button>

      {/* Mobile Drawer Overlay (#29 — swipe gestures + backdrop) */}
      <AnimatePresence>
        {mobileOpen && (
          <>
            {/* Backdrop — click to close */}
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.2 }}
              className="md:hidden fixed inset-0 bg-black/60 backdrop-blur-sm z-40"
              onClick={() => setMobileOpen(false)}
              role="presentation"
            />
            {/* Drawer — swipe left to close (#29) */}
            <motion.aside
              initial={{ x: -280 }}
              animate={{ x: 0 }}
              exit={{ x: -280 }}
              transition={{ type: 'spring', stiffness: 300, damping: 30 }}
              className="md:hidden fixed left-0 top-0 bottom-0 w-60 z-50"
              onTouchStart={handleDrawerTouchStart}
              onTouchMove={handleDrawerTouchMove}
              onTouchEnd={handleDrawerTouchEnd}
            >
              {sidebarContent}
            </motion.aside>
          </>
        )}
      </AnimatePresence>

      {/* Partner session modal (lazy-loaded) */}
      <Suspense fallback={null}>
        <PartnerChatModal sessionId={partnerModalSessionId} onClose={() => setPartnerModalSessionId(null)} />
      </Suspense>
    </>
  );
}

Sidebar.displayName = 'Sidebar';
