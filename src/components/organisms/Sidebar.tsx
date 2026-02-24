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
  Activity,
  BrainCircuit,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  Clock,
  Globe,
  Home,
  type LucideIcon,
  MessageSquare,
  Moon,
  Plus,
  Settings,
  Sparkles,
  Sun,
  Swords,
  Users,
  X,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useTheme } from '@/contexts/ThemeContext';
import { cn } from '@/shared/utils/cn';
import { useSessionSync } from '@/features/chat/hooks/useSessionSync';
import { useViewStore, type View } from '@/stores/viewStore';

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
// SIDEBAR COMPONENT
// ============================================

export function Sidebar() {
  const { t, i18n } = useTranslation();
  const { resolvedTheme, toggleTheme } = useTheme();

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
  } = useSessionSync();

  // Sessions sorted by creation date (newest first)
  const sortedSessions = useMemo(() => [...sessions].sort((a, b) => b.createdAt - a.createdAt), [sessions]);

  const isCollapsed = sidebarCollapsed;

  // Mobile drawer state
  const [mobileOpen, setMobileOpen] = useState(false);

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
    (e: React.MouseEvent, id: string) => {
      e.stopPropagation();
      if (sessions.length > 1) {
        void deleteSessionWithSync(id);
      }
    },
    [deleteSessionWithSync, sessions.length],
  );

  const handleNavClick = useCallback(
    (view: View) => {
      setCurrentView(view);
      setMobileOpen(false);
    },
    [setCurrentView],
  );

  // Language dropdown state
  const [showLangDropdown, setShowLangDropdown] = useState(false);

  const languages = [
    { code: 'en', name: 'English', flag: '\u{1F1EC}\u{1F1E7}' },
    { code: 'pl', name: 'Polski', flag: '\u{1F1F5}\u{1F1F1}' },
  ];

  const selectLanguage = (langCode: string) => {
    i18n.changeLanguage(langCode);
    setShowLangDropdown(false);
  };

  const currentLang = languages.find((l) => l.code === i18n.language) || languages[1];

  // Navigation groups adapted for GeminiHydra v15 (Tissaia style)
  const navGroups: NavGroup[] = [
    {
      id: 'main',
      label: t('sidebar.groups.main', 'MAIN'),
      icon: Sparkles,
      items: [
        { id: 'home', icon: Home, label: t('nav.home', 'Start') },
        { id: 'chat', icon: MessageSquare, label: t('nav.chat', 'Chat') },
      ],
    },
    {
      id: 'tools',
      label: t('sidebar.groups.tools', 'TOOLS'),
      icon: Users,
      items: [
        { id: 'agents', icon: Users, label: t('nav.agents', 'Agents') },
        { id: 'brain', icon: BrainCircuit, label: t('nav.brain', 'Brain') },
      ],
    },
    {
      id: 'system',
      label: t('sidebar.groups.system', 'SYSTEM'),
      icon: Settings,
      items: [
        { id: 'history', icon: Clock, label: t('nav.history', 'History') },
        { id: 'settings', icon: Settings, label: t('nav.settings', 'Settings') },
        { id: 'status', icon: Activity, label: t('nav.status', 'Status') },
      ],
    },
  ];

  // Track expanded groups
  const [expandedGroups, setExpandedGroups] = useState<Record<string, boolean>>(() => {
    const defaults = { main: true, tools: true, system: true };
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
        title={isCollapsed ? 'Expand sidebar' : 'Collapse sidebar'}
      >
        {isCollapsed ? (
          <ChevronRight size={18} strokeWidth={2.5} className={collapseIcon} />
        ) : (
          <ChevronLeft size={18} strokeWidth={2.5} className={collapseIcon} />
        )}
      </button>

      {/* Logo — click navigates to home */}
      <button
        type="button"
        onClick={() => handleNavClick('home')}
        className={cn(
          'flex items-center justify-center py-4 px-1 flex-shrink-0 cursor-pointer',
          isCollapsed ? 'w-full' : 'flex-1',
        )}
        title="Home"
      >
        <img
          src={isLight ? '/logolight.webp' : '/logodark.webp'}
          alt="GeminiHydra Logo"
          className={cn(
            'object-contain transition-all duration-300',
            isCollapsed ? 'w-16 h-16' : 'h-36',
          )}
        />
      </button>

      {/* Grouped Navigation */}
      <nav className="flex flex-col gap-2 flex-shrink-0">
        {navGroups.map((group) => {
          const isExpanded = expandedGroups[group.id];
          const hasActiveItem = group.items.some((item) => item.id === currentView);
          const GroupIcon = group.icon;

          return (
            <div key={group.id} className={cn(glassPanel, 'overflow-hidden')}>
              {/* Group Header */}
              {!isCollapsed ? (
                <button
                  type="button"
                  onClick={() => toggleGroup(group.id)}
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
                  !isCollapsed && !isExpanded ? 'max-h-0 opacity-0 pb-0' : 'max-h-96 opacity-100',
                  isCollapsed ? 'py-1.5' : '',
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
                      isCollapsed ? 'justify-center' : 'space-x-3',
                      currentView === item.id
                        ? isLight
                          ? 'bg-emerald-500/15 text-emerald-800'
                          : 'bg-white/10 text-white'
                        : cn(textMuted, hoverBg, textHover),
                    )}
                    title={isCollapsed ? item.label : undefined}
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
                    {!isCollapsed && <span className="font-medium text-base tracking-wide truncate">{item.label}</span>}
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
      {!isCollapsed && (
        <div className={cn(glassPanel, 'flex-1 flex flex-col min-h-0 p-2 overflow-hidden')}>
          {/* Section Header */}
          <div className="flex items-center justify-between px-1 py-1.5">
            <button
              type="button"
              onClick={() => setShowSessions(!showSessions)}
              className={cn(
                'flex items-center gap-2 transition-colors',
                textDim, textHover,
              )}
            >
              <MessageSquare size={14} />
              <span className="text-sm font-bold tracking-[0.12em] uppercase">
                {t('sidebar.chats', 'CZATY')}
              </span>
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
                title={t('sidebar.newChat', 'Nowy czat')}
              >
                <Plus
                  size={14}
                  className={cn(iconMuted, 'transition-colors', isLight ? 'hover:text-emerald-700' : 'hover:text-white')}
                />
              </button>
            </div>
          </div>

          {/* Session List */}
          <AnimatePresence>
            {showSessions && (
              <motion.div
                initial={{ height: 0, opacity: 0 }}
                animate={{ height: 'auto', opacity: 1 }}
                exit={{ height: 0, opacity: 0 }}
                transition={{ duration: 0.2, ease: 'easeInOut' }}
                className="flex-1 overflow-y-auto scrollbar-hide hover:scrollbar-thin hover:scrollbar-thumb-current space-y-0.5 mt-1"
              >
                {sortedSessions.map((session) => {
                  const isActive = session.id === currentSessionId;
                  const msgCount = (chatHistory[session.id] || []).length;

                  return (
                    <button
                      type="button"
                      key={session.id}
                      onClick={() => handleSelectSession(session.id)}
                      className={cn(
                        'relative w-full flex items-center gap-2 px-2 py-1.5 rounded-lg transition-all duration-200 group text-left',
                        isActive
                          ? isLight
                            ? 'bg-emerald-500/15 text-emerald-800'
                            : 'bg-white/10 text-white'
                          : cn(textMuted, hoverBg, textHover),
                      )}
                      title={session.title}
                    >
                      <MessageSquare
                        size={14}
                        className={cn(
                          'flex-shrink-0 transition-colors',
                          isActive ? (isLight ? 'text-emerald-700' : 'text-white') : iconMuted,
                        )}
                      />
                      <div className="flex-1 min-w-0">
                        <span className="text-sm truncate block leading-tight">{session.title}</span>
                        {msgCount > 0 && (
                          <span className={cn('text-[10px] font-mono', textDim)}>
                            {msgCount} {msgCount === 1 ? 'msg' : 'msgs'}
                          </span>
                        )}
                      </div>
                      {/* Delete button */}
                      {sessions.length > 1 && (
                        <button
                          type="button"
                          onClick={(e) => handleDeleteSession(e, session.id)}
                          className={cn(
                            'p-0.5 rounded opacity-0 group-hover:opacity-60 hover:!opacity-100 transition-all',
                            isLight ? 'hover:text-red-600' : 'hover:text-red-400',
                          )}
                          title={t('sidebar.deleteChat', 'Delete chat')}
                        >
                          <X size={12} />
                        </button>
                      )}
                      {/* Active indicator */}
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
                    </button>
                  );
                })}
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      )}

      {/* Collapsed: New Chat icon */}
      {isCollapsed && (
        <div className="flex flex-col items-center gap-1 flex-1">
          <button
            type="button"
            onClick={handleNewChat}
            className={cn('p-2 rounded-lg transition-all', hoverBg)}
            title={t('sidebar.newChat', 'Nowy czat')}
          >
            <Plus size={18} className={cn(iconMuted, isLight ? 'hover:text-emerald-700' : 'hover:text-white', 'transition-colors')} />
          </button>
        </div>
      )}

      {/* Footer / Lang & Theme Toggle */}
      <div className={cn(glassPanel, 'p-2 space-y-1')}>
        {/* Theme Toggle */}
        <button
          type="button"
          data-testid="btn-theme-toggle"
          onClick={toggleTheme}
          className={cn(
            'flex items-center gap-3 w-full p-2 rounded-lg transition-all group',
            isCollapsed ? 'justify-center' : 'justify-start',
            hoverBg,
          )}
          title={isCollapsed ? `Theme: ${resolvedTheme === 'dark' ? 'Dark' : 'Light'}` : undefined}
        >
          <div className="relative">
            {resolvedTheme === 'dark' ? (
              <Moon size={18} className="text-slate-400 group-hover:text-white transition-colors" />
            ) : (
              <Sun size={18} className="text-amber-600 group-hover:text-amber-500 transition-colors" />
            )}
          </div>
          {!isCollapsed && (
            <span className={cn('text-base font-mono tracking-tight truncate', textMuted, textHover)}>
              {resolvedTheme === 'dark'
                ? i18n.language === 'pl'
                  ? 'TRYB CIEMNY'
                  : 'DARK MODE'
                : i18n.language === 'pl'
                  ? 'TRYB JASNY'
                  : 'LIGHT MODE'}
            </span>
          )}
        </button>

        {/* Language Selector */}
        <div className="relative">
          <button
            type="button"
            onClick={() => setShowLangDropdown(!showLangDropdown)}
            className={cn(
              'flex items-center gap-3 w-full p-2 rounded-lg transition-all group',
              isCollapsed ? 'justify-center' : 'justify-between',
              hoverBg,
            )}
            title={isCollapsed ? `Language: ${currentLang?.name}` : undefined}
          >
            <div className="flex items-center gap-3">
              <div className="relative">
                <Globe size={18} className={cn(iconMuted, iconHover, 'transition-colors')} />
              </div>
              {!isCollapsed && (
                <span className={cn('text-base font-mono truncate', textMuted, textHover)}>
                  <span className="mr-1.5">{currentLang?.flag}</span>
                  <span className={cn('font-bold', isLight ? 'text-emerald-700' : 'text-white')}>
                    {currentLang?.code.toUpperCase()}
                  </span>
                </span>
              )}
            </div>
            {!isCollapsed && (
              <ChevronDown
                size={14}
                className={cn(textDim, 'transition-transform duration-200', showLangDropdown ? 'rotate-180' : '')}
              />
            )}
          </button>

          {/* Language Dropdown */}
          <AnimatePresence>
            {showLangDropdown && (
              <motion.div
                initial={{ opacity: 0, y: 4 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: 4 }}
                transition={{ duration: 0.15 }}
                className={cn(
                  'absolute bottom-full left-0 right-0 mb-1 rounded-xl backdrop-blur-xl border overflow-hidden z-50',
                  isLight
                    ? 'bg-white/95 border-emerald-600/20 shadow-[0_8px_32px_rgba(0,0,0,0.15)]'
                    : 'bg-black/90 border-white/20 shadow-[0_8px_32px_rgba(0,0,0,0.6)]',
                )}
              >
                {languages.map((lang) => (
                  <button
                    type="button"
                    key={lang.code}
                    onClick={() => selectLanguage(lang.code)}
                    className={cn(
                      'w-full flex items-center gap-3 px-3 py-2.5 text-sm transition-all',
                      i18n.language === lang.code
                        ? isLight
                          ? 'bg-emerald-500/15 text-emerald-800'
                          : 'bg-white/15 text-white'
                        : cn(textMuted, hoverBg, textHover),
                    )}
                  >
                    <span className="text-base">{lang.flag}</span>
                    <span className="font-mono">{lang.name}</span>
                    {i18n.language === lang.code && (
                      <div
                        className={cn(
                          'ml-auto w-1.5 h-1.5 rounded-full',
                          isLight
                            ? 'bg-emerald-600 shadow-[0_0_6px_rgba(5,150,105,0.5)]'
                            : 'bg-white shadow-[0_0_6px_rgba(255,255,255,0.5)]',
                        )}
                      />
                    )}
                  </button>
                ))}
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      </div>

      {/* Version */}
      {!isCollapsed && (
        <div className={cn('text-center text-xs py-2', isLight ? 'text-slate-600' : 'text-white/50')}>
          <span className={isLight ? 'text-emerald-700' : 'text-white'}>GeminiHydra</span> v15.0.0 | Wolf Swarm
        </div>
      )}
    </div>
  );

  return (
    <>
      {/* Desktop Sidebar */}
      <motion.aside
        className={cn('shrink-0 h-full hidden md:flex transition-none')}
        animate={{ width: isCollapsed ? 64 : 240 }}
        transition={{ type: 'spring', stiffness: 300, damping: 30 }}
      >
        {sidebarContent}
      </motion.aside>

      {/* Mobile: Hamburger trigger (rendered outside for AppShell) */}
      <button
        type="button"
        onClick={() => setMobileOpen(true)}
        className={cn('md:hidden fixed top-3 left-3 z-50 p-2 rounded-lg', glassPanel)}
        aria-label="Open sidebar"
      >
        <Swords size={20} className={isLight ? 'text-emerald-700' : 'text-white'} />
      </button>

      {/* Mobile Drawer Overlay */}
      <AnimatePresence>
        {mobileOpen && (
          <>
            {/* Backdrop */}
            <motion.div
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.2 }}
              className="md:hidden fixed inset-0 bg-black/50 backdrop-blur-sm z-40"
              onClick={() => setMobileOpen(false)}
            />
            {/* Drawer */}
            <motion.aside
              initial={{ x: -280 }}
              animate={{ x: 0 }}
              exit={{ x: -280 }}
              transition={{ type: 'spring', stiffness: 300, damping: 30 }}
              className="md:hidden fixed left-0 top-0 bottom-0 w-60 z-50"
            >
              {sidebarContent}
            </motion.aside>
          </>
        )}
      </AnimatePresence>
    </>
  );
}

Sidebar.displayName = 'Sidebar';
