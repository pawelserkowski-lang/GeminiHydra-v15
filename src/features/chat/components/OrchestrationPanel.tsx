// src/features/chat/components/OrchestrationPanel.tsx
/**
 * OrchestrationPanel — ADK multi-agent pipeline visualization
 * =============================================================
 * Displays orchestration progress: sequential steps, parallel agent cards,
 * delegation chain, and pattern badge. Shown above AgentActivityPanel
 * when orchestration is active.
 */

import {
  ArrowRight,
  CheckCircle2,
  ChevronDown,
  ChevronUp,
  GitBranch,
  Loader2,
  Network,
  RefreshCw,
  Shield,
  Users,
  XCircle,
  Zap,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { memo, useCallback, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';
import type { AgentStatus, DelegationEvent, OrchestrationState } from '../hooks/useOrchestration';

// ============================================================================
// PATTERN DISPLAY
// ============================================================================

const PATTERN_CONFIG: Record<string, { label: string; icon: typeof Network; color: string }> = {
  sequential: { label: 'Sequential', icon: ArrowRight, color: 'text-blue-400' },
  parallel: { label: 'Parallel', icon: Users, color: 'text-purple-400' },
  loop: { label: 'Loop', icon: RefreshCw, color: 'text-amber-400' },
  hierarchical: { label: 'Hierarchical', icon: Network, color: 'text-emerald-400' },
  review: { label: 'Review', icon: GitBranch, color: 'text-cyan-400' },
  security: { label: 'Security Review', icon: Shield, color: 'text-red-400' },
};

// ============================================================================
// STATUS ICON
// ============================================================================

const StatusIcon = memo<{ status: AgentStatus['status'] }>(({ status }) => {
  switch (status) {
    case 'running':
      return <Loader2 size={14} className="animate-spin text-amber-400 shrink-0" />;
    case 'done':
      return <CheckCircle2 size={14} className="text-emerald-400 shrink-0" />;
    case 'error':
      return <XCircle size={14} className="text-red-400 shrink-0" />;
    default:
      return <div className="w-3.5 h-3.5 rounded-full border border-current opacity-30 shrink-0" />;
  }
});

StatusIcon.displayName = 'StatusIcon';

// ============================================================================
// DELEGATION CHAIN
// ============================================================================

const DelegationChain = memo<{ chain: DelegationEvent[] }>(({ chain }) => {
  const theme = useViewTheme();
  if (chain.length === 0) return null;

  return (
    <div className="space-y-1">
      {/* biome-ignore lint/suspicious/noArrayIndexKey: delegation events are append-only and have no stable ID */}
      {chain.map((d, i) => (
        <motion.div
          key={`del-${d.fromAgent}-${d.toAgent}-${i}`}
          initial={{ opacity: 0, x: -8 }}
          animate={{ opacity: 1, x: 0 }}
          className="flex items-center gap-1.5 text-xs"
        >
          <span className={cn('font-bold', theme.accentText)}>{d.fromAgent}</span>
          <ArrowRight size={10} className={theme.textMuted} />
          <span className={cn('font-bold', theme.accentText)}>{d.toAgent}</span>
          <span className={cn('truncate', theme.textMuted)}>{d.reason}</span>
        </motion.div>
      ))}
    </div>
  );
});

DelegationChain.displayName = 'DelegationChain';

// ============================================================================
// SEQUENTIAL PROGRESS
// ============================================================================

const SequentialProgress = memo<{ agents: AgentStatus[]; currentStep: number; totalSteps: number }>(
  ({ agents, currentStep, totalSteps }) => {
    const theme = useViewTheme();
    const progress = totalSteps > 0 ? (currentStep / totalSteps) * 100 : 0;

    return (
      <div className="space-y-2">
        {/* Progress bar */}
        <div className="h-1.5 rounded-full bg-white/5 overflow-hidden">
          <motion.div
            className="h-full rounded-full bg-blue-500"
            initial={{ width: 0 }}
            animate={{ width: `${progress}%` }}
            transition={{ duration: 0.3 }}
          />
        </div>
        {/* Agent steps */}
        <div className="flex items-center gap-1 flex-wrap">
          {agents.map((a, i) => (
            <div key={a.agent} className="flex items-center gap-1">
              {i > 0 && <ArrowRight size={10} className={theme.textMuted} />}
              <div className="flex items-center gap-1">
                <StatusIcon status={a.status} />
                <span
                  className={cn(
                    'text-xs font-mono',
                    a.status === 'running' && 'font-bold',
                    a.status === 'done' ? theme.textMuted : theme.accentText,
                  )}
                >
                  {a.agent}
                </span>
              </div>
            </div>
          ))}
        </div>
      </div>
    );
  },
);

SequentialProgress.displayName = 'SequentialProgress';

// ============================================================================
// PARALLEL AGENTS
// ============================================================================

const ParallelAgents = memo<{ agents: AgentStatus[] }>(({ agents }) => {
  const theme = useViewTheme();

  return (
    <div className="grid grid-cols-2 gap-2">
      {agents.map((a) => (
        <motion.div
          key={a.agent}
          initial={{ opacity: 0, scale: 0.95 }}
          animate={{ opacity: 1, scale: 1 }}
          className={cn(
            'px-3 py-2 rounded-lg border text-xs font-mono',
            a.status === 'running'
              ? 'border-amber-500/30 bg-amber-500/5'
              : a.status === 'done'
                ? 'border-emerald-500/30 bg-emerald-500/5'
                : a.status === 'error'
                  ? 'border-red-500/30 bg-red-500/5'
                  : 'border-white/10 bg-white/5',
          )}
        >
          <div className="flex items-center gap-1.5 mb-1">
            <StatusIcon status={a.status} />
            <span className={cn('font-bold', theme.accentText)}>{a.agent}</span>
          </div>
          {a.outputPreview && <p className={cn('truncate', theme.textMuted)}>{a.outputPreview}</p>}
        </motion.div>
      ))}
    </div>
  );
});

ParallelAgents.displayName = 'ParallelAgents';

// ============================================================================
// MAIN COMPONENT
// ============================================================================

export const OrchestrationPanel = memo<{ orchestration: OrchestrationState }>(({ orchestration }) => {
  const { t } = useTranslation();
  const theme = useViewTheme();
  const [expanded, setExpanded] = useState(true);
  const toggleExpanded = useCallback(() => setExpanded((p) => !p), []);

  const patternCfg = useMemo(() => {
    const cfg = PATTERN_CONFIG[orchestration.pattern ?? ''];
    if (cfg) return cfg;
    return { label: 'Hierarchical', icon: Network, color: 'text-emerald-400' };
  }, [orchestration.pattern]);

  const PatternIcon = patternCfg.icon;
  const doneCount = orchestration.agents.filter((a) => a.status === 'done').length;

  if (!orchestration.isOrchestrating && orchestration.agents.length === 0) {
    return null;
  }

  return (
    <motion.div
      initial={{ opacity: 0, height: 0 }}
      animate={{ opacity: 1, height: 'auto' }}
      exit={{ opacity: 0, height: 0 }}
      transition={{ duration: 0.2 }}
      className={cn(
        'shrink-0 rounded-xl overflow-hidden font-mono text-sm',
        theme.isLight ? 'bg-blue-500/5 border border-blue-500/15' : 'bg-blue-500/5 border border-blue-500/15',
      )}
    >
      {/* Header */}
      <button
        type="button"
        onClick={toggleExpanded}
        className={cn('w-full flex items-center gap-2 px-4 py-2 transition-colors', 'hover:bg-blue-500/10')}
      >
        {orchestration.isOrchestrating ? (
          <Loader2 size={16} className={cn(patternCfg.color, 'animate-spin')} />
        ) : (
          <Zap size={16} className={patternCfg.color} />
        )}

        {/* Pattern badge */}
        <span className={cn('px-2 py-0.5 rounded text-xs font-bold bg-blue-500/20', patternCfg.color)}>
          <PatternIcon size={12} className="inline mr-1" />
          {patternCfg.label}
        </span>

        {/* Progress summary */}
        <span className={cn('text-xs', theme.textMuted)}>
          {doneCount}/{orchestration.agents.length} {t('chat.agentsDone', 'agents')}
        </span>

        {/* Current agent */}
        {orchestration.currentAgent && orchestration.isOrchestrating && (
          <span className={cn('text-xs font-bold ml-auto', theme.accentText)}>{orchestration.currentAgent}</span>
        )}

        {expanded ? (
          <ChevronUp size={14} className={cn(theme.textMuted, !orchestration.currentAgent && 'ml-auto')} />
        ) : (
          <ChevronDown size={14} className={cn(theme.textMuted, !orchestration.currentAgent && 'ml-auto')} />
        )}
      </button>

      {/* Expandable body */}
      <AnimatePresence initial={false}>
        {expanded && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 'auto', opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.15 }}
            className="overflow-hidden"
          >
            <div className="px-4 pb-3 space-y-2">
              {/* Pattern-specific visualization */}
              {(orchestration.pattern === 'sequential' ||
                orchestration.pattern === 'review' ||
                orchestration.pattern === 'security') && (
                <SequentialProgress
                  agents={orchestration.agents}
                  currentStep={orchestration.currentStep}
                  totalSteps={orchestration.totalSteps}
                />
              )}

              {orchestration.pattern === 'parallel' && <ParallelAgents agents={orchestration.agents} />}

              {orchestration.pattern === 'loop' && (
                <div className="space-y-2">
                  <div className="flex items-center gap-2 text-xs">
                    <RefreshCw
                      size={14}
                      className={cn(orchestration.isOrchestrating ? 'animate-spin' : '', 'text-amber-400')}
                    />
                    <span className={theme.textMuted}>
                      Iteration {orchestration.currentStep}/{orchestration.totalSteps}
                    </span>
                  </div>
                  <SequentialProgress
                    agents={orchestration.agents}
                    currentStep={orchestration.currentStep}
                    totalSteps={orchestration.totalSteps}
                  />
                </div>
              )}

              {orchestration.pattern === 'hierarchical' && (
                <div className="space-y-2">
                  {orchestration.agents.map((a) => (
                    <div key={a.agent} className="flex items-center gap-2">
                      <StatusIcon status={a.status} />
                      <span
                        className={cn('text-xs font-mono', a.status === 'running' ? 'font-bold' : '', theme.accentText)}
                      >
                        {a.agent}
                      </span>
                      {a.outputPreview && (
                        <span className={cn('text-xs truncate max-w-[250px]', theme.textMuted)}>{a.outputPreview}</span>
                      )}
                    </div>
                  ))}
                </div>
              )}

              {/* Delegation chain */}
              {orchestration.delegationChain.length > 0 && <DelegationChain chain={orchestration.delegationChain} />}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </motion.div>
  );
});

OrchestrationPanel.displayName = 'OrchestrationPanel';

export default OrchestrationPanel;
