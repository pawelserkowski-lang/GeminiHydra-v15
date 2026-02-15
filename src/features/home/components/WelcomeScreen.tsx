// src/features/home/components/WelcomeScreen.tsx
/**
 * GeminiHydra v15 - WelcomeScreen (Home View)
 * =============================================
 * Centered hero card with logo, feature badges, CTA buttons, and recent sessions.
 * Ported from legacy GeminiHydra WelcomeScreen with glassmorphism + motion.
 */

import { Clock, Cpu, Globe, Layers, MessageSquare, Plus, Settings, Sparkles, Users, Workflow } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { memo, useCallback, useMemo } from 'react';

import { Badge, Button } from '@/components/atoms';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';
import { type Session, useViewStore } from '@/stores/viewStore';

// ============================================================================
// CONSTANTS
// ============================================================================

const FEATURE_BADGES = [
  { label: '12 Agents', icon: Users },
  { label: '5-Phase Pipeline', icon: Workflow },
  { label: 'Multi-Provider', icon: Globe },
  { label: 'Swarm Architecture', icon: Cpu },
] as const;

const MAX_RECENT_SESSIONS = 5;

// ============================================================================
// HELPERS
// ============================================================================

function timeAgo(timestamp: number): string {
  const diff = Date.now() - timestamp;
  const minutes = Math.floor(diff / 60_000);
  if (minutes < 1) return 'just now';
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days === 1) return 'yesterday';
  return `${days}d ago`;
}

// ============================================================================
// ANIMATION VARIANTS
// ============================================================================

const heroVariants = {
  hidden: { opacity: 0, y: 20, scale: 0.96 },
  visible: {
    opacity: 1,
    y: 0,
    scale: 1,
    transition: { duration: 0.6, ease: [0.22, 1, 0.36, 1] as const },
  },
};

const ctaVariants = {
  hidden: { opacity: 0, y: 12 },
  visible: {
    opacity: 1,
    y: 0,
    transition: { duration: 0.4, delay: 0.2 },
  },
};

const recentVariants = {
  hidden: { opacity: 0, y: 8 },
  visible: {
    opacity: 1,
    y: 0,
    transition: { duration: 0.4, delay: 0.3 },
  },
};

const badgeContainerVariants = {
  hidden: {},
  visible: {
    transition: { staggerChildren: 0.06, delayChildren: 0.15 },
  },
};

const badgeItemVariants = {
  hidden: { opacity: 0, scale: 0.85 },
  visible: { opacity: 1, scale: 1 },
};

// ============================================================================
// SESSION ROW SUB-COMPONENT
// ============================================================================

interface SessionRowProps {
  session: Session;
  messageCount: number;
  onOpen: (id: string) => void;
  theme: ReturnType<typeof useViewTheme>;
}

const SessionRow = memo<SessionRowProps>(({ session, messageCount, onOpen, theme }) => (
  <motion.button
    type="button"
    onClick={() => onOpen(session.id)}
    className={cn(
      'w-full flex items-center gap-3 p-3 rounded-xl',
      'transition-all duration-200 group cursor-pointer text-left',
      theme.listItem,
      theme.listItemHover,
    )}
    whileHover={{ x: 4 }}
    whileTap={{ scale: 0.98 }}
  >
    <MessageSquare
      size={16}
      className={cn('flex-shrink-0 transition-colors', 'group-hover:text-[var(--matrix-accent)]', theme.iconMuted)}
    />
    <div className="flex-1 min-w-0">
      <p className={cn('text-sm truncate transition-colors', 'group-hover:text-[var(--matrix-accent)]', theme.text)}>
        {session.title}
      </p>
    </div>
    <div className="flex flex-col items-end flex-shrink-0">
      <span className={cn('text-[10px] font-mono', theme.textMuted)}>{timeAgo(session.createdAt)}</span>
      {messageCount > 0 && <span className={cn('text-[10px] font-mono', theme.textMuted)}>{messageCount} msg</span>}
    </div>
  </motion.button>
));

SessionRow.displayName = 'SessionRow';

// ============================================================================
// WELCOME SCREEN
// ============================================================================

export const WelcomeScreen = memo(() => {
  const theme = useViewTheme();

  const rawSessions = useViewStore((s) => s.sessions);
  const chatHistory = useViewStore((s) => s.chatHistory);
  const selectSession = useViewStore((s) => s.selectSession);
  const createSession = useViewStore((s) => s.createSession);
  const setCurrentView = useViewStore((s) => s.setCurrentView);
  const openTab = useViewStore((s) => s.openTab);

  const recentSessions = useMemo(
    () => [...rawSessions].sort((a, b) => b.createdAt - a.createdAt).slice(0, MAX_RECENT_SESSIONS),
    [rawSessions],
  );

  const handleNewChat = useCallback(() => {
    createSession();
    const sessionId = useViewStore.getState().currentSessionId;
    if (sessionId) openTab(sessionId);
    setCurrentView('chat');
  }, [createSession, openTab, setCurrentView]);

  const handleOpenSession = useCallback(
    (sessionId: string) => {
      selectSession(sessionId);
      openTab(sessionId);
      setCurrentView('chat');
    },
    [selectSession, openTab, setCurrentView],
  );

  const handleViewAgents = useCallback(() => {
    setCurrentView('agents');
  }, [setCurrentView]);

  const handleOpenSettings = useCallback(() => {
    setCurrentView('settings');
  }, [setCurrentView]);

  return (
    <div className="h-full flex flex-col items-center justify-center p-8 overflow-y-auto">
      {/* ====== Hero Card ====== */}
      <motion.div
        data-testid="welcome-hero"
        className={cn('flex flex-col items-center gap-6 p-8 rounded-3xl max-w-lg w-full', theme.card)}
        variants={heroVariants}
        initial="hidden"
        animate="visible"
      >
        {/* Logo with glow */}
        <div className="relative">
          <div
            className="absolute inset-0 rounded-2xl blur-xl opacity-40"
            style={{ background: 'var(--matrix-accent)' }}
          />
          <img
            src={theme.isLight ? '/logolight.webp' : '/logodark.webp'}
            alt="GeminiHydra Logo"
            className="relative w-56 h-56 object-contain drop-shadow-lg"
          />
        </div>

        {/* Title */}
        <div className="text-center">
          <h1 className={cn('text-3xl font-bold font-mono tracking-tight', theme.title)}>GeminiHydra</h1>
          <p className={cn('text-sm mt-1.5 max-w-xs', theme.textMuted)}>
            AI Swarm Control Center -- start a new chat or continue a previous conversation.
          </p>
        </div>

        {/* Feature Badges */}
        <motion.div
          className="flex flex-wrap justify-center gap-2"
          variants={badgeContainerVariants}
          initial="hidden"
          animate="visible"
        >
          {FEATURE_BADGES.map(({ label, icon: Icon }) => (
            <motion.div key={label} variants={badgeItemVariants}>
              <Badge variant="accent" size="sm" icon={<Icon size={12} />}>
                {label}
              </Badge>
            </motion.div>
          ))}
        </motion.div>

        {/* CTA Buttons */}
        <motion.div
          className="grid grid-cols-1 sm:grid-cols-3 gap-3 w-full mt-2"
          variants={ctaVariants}
          initial="hidden"
          animate="visible"
        >
          <Button
            variant="primary"
            size="md"
            leftIcon={<Plus size={16} />}
            onClick={handleNewChat}
            className="w-full"
            data-testid="btn-new-chat"
          >
            New Chat
          </Button>
          <Button
            variant="secondary"
            size="md"
            leftIcon={<Layers size={16} />}
            onClick={handleViewAgents}
            className="w-full"
          >
            Agents
          </Button>
          <Button
            variant="ghost"
            size="md"
            leftIcon={<Settings size={16} />}
            onClick={handleOpenSettings}
            className="w-full"
          >
            Settings
          </Button>
        </motion.div>
      </motion.div>

      {/* ====== Recent Sessions ====== */}
      <AnimatePresence>
        {recentSessions.length > 0 && (
          <motion.div
            className="w-full max-w-lg mt-8"
            variants={recentVariants}
            initial="hidden"
            animate="visible"
            exit="hidden"
          >
            <div className="flex items-center gap-2 mb-3">
              <Clock size={14} className={theme.iconMuted} />
              <span className={cn('text-xs uppercase tracking-wider font-mono', theme.textMuted)}>Recent Chats</span>
            </div>

            <div className="space-y-2">
              {recentSessions.map((session) => (
                <SessionRow
                  key={session.id}
                  session={session}
                  messageCount={chatHistory[session.id]?.length ?? 0}
                  onOpen={handleOpenSession}
                  theme={theme}
                />
              ))}
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* ====== Empty State ====== */}
      <AnimatePresence>
        {recentSessions.length === 0 && (
          <motion.div
            className="flex flex-col items-center gap-3 mt-8 text-center"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ delay: 0.35 }}
          >
            <Sparkles size={32} className={cn(theme.iconMuted, 'opacity-40')} />
            <p className={cn('text-sm', theme.textMuted)}>No chats yet. Start a new conversation!</p>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
});

WelcomeScreen.displayName = 'WelcomeScreen';

export default WelcomeScreen;
