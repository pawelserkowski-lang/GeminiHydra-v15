// src/features/chat/components/AgentActivityPanel.tsx
/**
 * AgentActivityPanel — Live agent activity feed
 * ================================================
 * Shows real-time plan steps, tool calls (in-progress/completed),
 * and execution metadata during streaming. Collapses when idle.
 */

import {
  CheckCircle2,
  ChevronDown,
  ChevronUp,
  Cog,
  Loader2,
  Target,
  Wrench,
  XCircle,
  Zap,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { memo, useCallback, useState } from 'react';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';

// ============================================================================
// TYPES
// ============================================================================

export interface ToolActivity {
  name: string;
  args?: unknown;
  iteration: number;
  status: 'running' | 'success' | 'error';
  summary?: string;
  startedAt: number;
  completedAt?: number;
}

export interface AgentActivity {
  agent: string | null;
  model: string | null;
  confidence: number | null;
  planSteps: string[];
  tools: ToolActivity[];
  isActive: boolean;
}

export const EMPTY_ACTIVITY: AgentActivity = {
  agent: null,
  model: null,
  confidence: null,
  planSteps: [],
  tools: [],
  isActive: false,
};

// ============================================================================
// COMPONENT
// ============================================================================

export const AgentActivityPanel = memo<{ activity: AgentActivity }>(({ activity }) => {
  const theme = useViewTheme();
  const [expanded, setExpanded] = useState(true);

  const toggleExpanded = useCallback(() => setExpanded((p) => !p), []);

  const runningTools = activity.tools.filter((t) => t.status === 'running');
  const completedTools = activity.tools.filter((t) => t.status !== 'running');

  // Don't render when there's nothing to show
  if (!activity.isActive && activity.tools.length === 0 && activity.planSteps.length === 0) {
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
        theme.isLight
          ? 'bg-emerald-500/5 border border-emerald-500/15'
          : 'bg-[var(--matrix-accent)]/5 border border-[var(--matrix-accent)]/15',
      )}
    >
      {/* Header bar — always visible */}
      <button
        type="button"
        onClick={toggleExpanded}
        className={cn(
          'w-full flex items-center gap-2 px-4 py-2 transition-colors',
          theme.isLight ? 'hover:bg-emerald-500/10' : 'hover:bg-[var(--matrix-accent)]/10',
        )}
      >
        {activity.isActive ? (
          <Loader2 size={16} className={cn(theme.accentText, 'animate-spin')} />
        ) : (
          <Zap size={16} className={theme.accentText} />
        )}

        {/* Agent + model */}
        {activity.agent && (
          <span className={cn('font-bold', theme.accentText)}>{activity.agent}</span>
        )}
        {activity.model && (
          <span className={cn('opacity-50', theme.textMuted)}>· {activity.model}</span>
        )}

        {/* Confidence badge */}
        {activity.confidence !== null && (
          <span
            className={cn(
              'px-2 py-0.5 rounded text-xs font-bold',
              activity.confidence > 0.7
                ? 'bg-emerald-500/20 text-emerald-400'
                : activity.confidence > 0.4
                  ? 'bg-amber-500/20 text-amber-400'
                  : 'bg-red-500/20 text-red-400',
            )}
          >
            {Math.round(activity.confidence * 100)}%
          </span>
        )}

        {/* Running tool count */}
        {runningTools.length > 0 && (
          <span className="ml-auto flex items-center gap-1.5 text-xs text-amber-400 font-bold">
            <Cog size={14} className="animate-spin" />
            {runningTools.length} running
          </span>
        )}

        {/* Completed count */}
        {completedTools.length > 0 && (
          <span className={cn('flex items-center gap-1.5 text-xs', runningTools.length === 0 && 'ml-auto', theme.textMuted)}>
            <CheckCircle2 size={14} />
            {completedTools.length} done
          </span>
        )}

        {expanded ? <ChevronUp size={14} className={theme.textMuted} /> : <ChevronDown size={14} className={theme.textMuted} />}
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
            <div className="px-4 pb-3 space-y-1.5">
              {/* Plan steps */}
              {activity.planSteps.length > 0 && (
                <div className="space-y-1">
                  {activity.planSteps.map((step, i) => (
                    <div key={`step-${i}`} className="flex items-start gap-2">
                      <Target size={14} className={cn('mt-0.5 shrink-0', theme.accentText, 'opacity-50')} />
                      <span className={theme.textMuted}>{step}</span>
                    </div>
                  ))}
                </div>
              )}

              {/* Tool calls */}
              {activity.tools.length > 0 && (
                <div className="space-y-1 pt-1">
                  {activity.tools.map((tool, i) => (
                    <ToolRow key={`tool-${tool.iteration}-${tool.name}-${i}`} tool={tool} theme={theme} />
                  ))}
                </div>
              )}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </motion.div>
  );
});

AgentActivityPanel.displayName = 'AgentActivityPanel';

// ============================================================================
// TOOL ROW
// ============================================================================

const ToolRow = memo<{ tool: ToolActivity; theme: ReturnType<typeof useViewTheme> }>(({ tool, theme }) => {
  const elapsed = tool.completedAt
    ? `${((tool.completedAt - tool.startedAt) / 1000).toFixed(1)}s`
    : null;

  return (
    <motion.div
      initial={{ opacity: 0, x: -8 }}
      animate={{ opacity: 1, x: 0 }}
      className="flex items-center gap-2"
    >
      {tool.status === 'running' && <Loader2 size={14} className="animate-spin text-amber-400 shrink-0" />}
      {tool.status === 'success' && <CheckCircle2 size={14} className="text-emerald-400 shrink-0" />}
      {tool.status === 'error' && <XCircle size={14} className="text-red-400 shrink-0" />}

      <Wrench size={14} className={cn(theme.textMuted, 'shrink-0')} />
      <span className={cn('font-bold', theme.accentText)}>{tool.name}</span>

      {elapsed && (
        <span className={cn('text-xs', theme.textMuted)}>{elapsed}</span>
      )}

      {tool.summary && tool.status !== 'running' && (
        <span className={cn('truncate max-w-[300px]', theme.textMuted)} title={tool.summary}>
          {tool.summary.slice(0, 80)}{tool.summary.length > 80 ? '…' : ''}
        </span>
      )}
    </motion.div>
  );
});

ToolRow.displayName = 'ToolRow';

export default AgentActivityPanel;
