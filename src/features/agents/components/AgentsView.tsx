// src/features/agents/components/AgentsView.tsx
/**
 * AgentsView - Witcher swarm agent dashboard
 * ============================================
 * Displays 12 Witcher agents in a responsive grid with tier filtering,
 * status indicators, and a 5-phase pipeline visualization.
 * Ported from GeminiHydra legacy with v15 design system atoms/molecules.
 */

import {
  ChevronRight,
  Crown,
  Eye,
  Flame,
  Flower2,
  Gem,
  type LucideIcon,
  Mountain,
  Music,
  Shield,
  Sword,
  Wand2,
  Zap,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { type ReactNode, useMemo, useState } from 'react';
import { Badge, Card } from '@/components/atoms';
import { StatusIndicator, type StatusState } from '@/components/molecules';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';

// ============================================================================
// TYPES
// ============================================================================

type AgentTier = 'commander' | 'coordinator' | 'executor';

type AgentId =
  | 'geralt'
  | 'yennefer'
  | 'triss'
  | 'jaskier'
  | 'vesemir'
  | 'ciri'
  | 'dijkstra'
  | 'lambert'
  | 'eskel'
  | 'regis'
  | 'zoltan'
  | 'philippa';

type PipelinePhase = 'PRE-A' | 'A' | 'B' | 'C' | 'D';

interface AgentDefinition {
  id: AgentId;
  displayName: string;
  role: string;
  description: string;
  icon: LucideIcon;
  color: string;
  tier: AgentTier;
  status: StatusState;
  pipelinePhase: PipelinePhase;
}

type TierFilter = 'all' | AgentTier;

// ============================================================================
// PIPELINE PHASES
// ============================================================================

const PIPELINE_PHASES: readonly PipelinePhase[] = ['PRE-A', 'A', 'B', 'C', 'D'] as const;

const PIPELINE_LABELS: Record<PipelinePhase, string> = {
  'PRE-A': 'Pre-Analysis',
  A: 'Analysis',
  B: 'Build',
  C: 'Check',
  D: 'Deploy',
};

// ============================================================================
// AGENT DEFINITIONS (12 Witcher agents)
// ============================================================================

const AGENTS: readonly AgentDefinition[] = [
  {
    id: 'geralt',
    displayName: 'Geralt',
    role: 'Commander',
    description: 'Supreme commander of the Hydra swarm. Orchestrates all agent coordination and final decision-making.',
    icon: Sword,
    color: '#FFD700',
    tier: 'commander',
    status: 'online',
    pipelinePhase: 'PRE-A',
  },
  {
    id: 'yennefer',
    displayName: 'Yennefer',
    role: 'Strategy',
    description: 'Strategic architect. Designs complex multi-step plans and oversees quality of analysis.',
    icon: Wand2,
    color: '#8B008B',
    tier: 'coordinator',
    status: 'online',
    pipelinePhase: 'A',
  },
  {
    id: 'triss',
    displayName: 'Triss',
    role: 'Medical',
    description: 'System diagnostics and recovery specialist. QA, testing, and health monitoring.',
    icon: Flower2,
    color: '#FF6347',
    tier: 'coordinator',
    status: 'online',
    pipelinePhase: 'C',
  },
  {
    id: 'dijkstra',
    displayName: 'Dijkstra',
    role: 'Intelligence',
    description: 'Master intelligence analyst. Deep research, data gathering, and knowledge synthesis.',
    icon: Eye,
    color: '#4169E1',
    tier: 'coordinator',
    status: 'pending',
    pipelinePhase: 'A',
  },
  {
    id: 'jaskier',
    displayName: 'Jaskier',
    role: 'Creative',
    description: 'Creative writer and documenter. Handles all content generation, documentation, and communication.',
    icon: Music,
    color: '#DAA520',
    tier: 'executor',
    status: 'online',
    pipelinePhase: 'D',
  },
  {
    id: 'vesemir',
    displayName: 'Vesemir',
    role: 'Wisdom',
    description: 'Elder mentor and code reviewer. Enforces best practices and architectural standards.',
    icon: Shield,
    color: '#8B4513',
    tier: 'executor',
    status: 'online',
    pipelinePhase: 'B',
  },
  {
    id: 'ciri',
    displayName: 'Ciri',
    role: 'Speed',
    description: 'Rapid task executor. Handles quick, lightweight operations with minimal latency.',
    icon: Zap,
    color: '#00CED1',
    tier: 'executor',
    status: 'online',
    pipelinePhase: 'B',
  },
  {
    id: 'lambert',
    displayName: 'Lambert',
    role: 'Combat',
    description: 'Debugger and profiler. Tracks down bugs, performance bottlenecks, and system anomalies.',
    icon: Flame,
    color: '#FF4500',
    tier: 'executor',
    status: 'offline',
    pipelinePhase: 'C',
  },
  {
    id: 'eskel',
    displayName: 'Eskel',
    role: 'Defense',
    description: 'DevOps and infrastructure guardian. Ensures system stability and deployment reliability.',
    icon: Mountain,
    color: '#556B2F',
    tier: 'executor',
    status: 'online',
    pipelinePhase: 'D',
  },
  {
    id: 'regis',
    displayName: 'Regis',
    role: 'Analysis',
    description: 'Deep analysis and research scholar. Provides contextual insights and knowledge graph queries.',
    icon: Crown,
    color: '#2F4F4F',
    tier: 'executor',
    status: 'pending',
    pipelinePhase: 'A',
  },
  {
    id: 'zoltan',
    displayName: 'Zoltan',
    role: 'Logistics',
    description: 'Data and resource manager. Handles database operations, file management, and data pipelines.',
    icon: Gem,
    color: '#4682B4',
    tier: 'executor',
    status: 'online',
    pipelinePhase: 'B',
  },
  {
    id: 'philippa',
    displayName: 'Philippa',
    role: 'Magic',
    description: 'Integration and API sorceress. Manages external service connections and advanced transformations.',
    icon: Wand2,
    color: '#9370DB',
    tier: 'executor',
    status: 'online',
    pipelinePhase: 'D',
  },
] as const;

// ============================================================================
// TIER CONFIG
// ============================================================================

const TIER_CONFIG: Record<AgentTier, { label: string; badgeVariant: 'accent' | 'warning' | 'default' }> = {
  commander: { label: 'Commander', badgeVariant: 'warning' },
  coordinator: { label: 'Coordinator', badgeVariant: 'accent' },
  executor: { label: 'Executor', badgeVariant: 'default' },
};

const FILTER_OPTIONS: readonly { value: TierFilter; label: string }[] = [
  { value: 'all', label: 'All Agents' },
  { value: 'commander', label: 'Commander' },
  { value: 'coordinator', label: 'Coordinators' },
  { value: 'executor', label: 'Executors' },
] as const;

// ============================================================================
// SUB-COMPONENTS
// ============================================================================

/** 5-phase pipeline visualization strip */
function PipelineStrip({ activePhase, agentColor }: { activePhase: PipelinePhase; agentColor: string }) {
  const t = useViewTheme();

  return (
    <div className="flex items-center gap-0.5">
      {PIPELINE_PHASES.map((phase, idx) => {
        const isActive = phase === activePhase;
        const isPast = PIPELINE_PHASES.indexOf(activePhase) > PIPELINE_PHASES.indexOf(phase);

        return (
          <div key={phase} className="flex items-center">
            <motion.div
              className={cn(
                'flex items-center justify-center rounded-sm text-[8px] font-mono leading-none px-1.5 py-0.5 border transition-all',
                isActive
                  ? 'border-current font-bold'
                  : isPast
                    ? t.isLight
                      ? 'bg-slate-100 border-slate-200/50 text-slate-400'
                      : 'bg-white/5 border-white/10 text-white/30'
                    : t.isLight
                      ? 'bg-slate-50 border-slate-100 text-slate-300'
                      : 'bg-white/3 border-white/5 text-white/20',
              )}
              style={
                isActive
                  ? {
                      backgroundColor: `${agentColor}20`,
                      borderColor: `${agentColor}50`,
                      color: agentColor,
                    }
                  : undefined
              }
              title={PIPELINE_LABELS[phase]}
              animate={isActive ? { scale: [1, 1.05, 1] } : undefined}
              transition={isActive ? { duration: 2, repeat: Number.POSITIVE_INFINITY, ease: 'easeInOut' } : undefined}
            >
              {phase}
            </motion.div>
            {idx < PIPELINE_PHASES.length - 1 && (
              <ChevronRight
                size={8}
                className={cn(
                  isPast || isActive
                    ? t.isLight
                      ? 'text-slate-400'
                      : 'text-white/30'
                    : t.isLight
                      ? 'text-slate-200'
                      : 'text-white/10',
                )}
              />
            )}
          </div>
        );
      })}
    </div>
  );
}

/** Single agent card */
function AgentCard({ agent }: { agent: AgentDefinition }) {
  const t = useViewTheme();
  const Icon = agent.icon;
  const tierCfg = TIER_CONFIG[agent.tier];

  const statusMap: Record<StatusState, string> = {
    online: 'Active',
    offline: 'Offline',
    pending: 'Busy',
    error: 'Error',
  };

  return (
    <motion.div
      initial={{ opacity: 0, y: 12 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: -8 }}
      transition={{ duration: 0.25 }}
      layout
    >
      <Card
        variant="hover"
        padding="none"
        className={cn(
          'group relative overflow-hidden',
          t.isLight && 'bg-white/50 border-white/30 hover:border-emerald-400/40 hover:shadow-lg',
        )}
      >
        {/* Accent top bar */}
        <div className="h-0.5 w-full" style={{ backgroundColor: agent.color }} />

        <div className="p-4 space-y-3">
          {/* Header: icon + name + status */}
          <div className="flex items-start justify-between">
            <div className="flex items-center gap-3">
              <div
                className="w-10 h-10 rounded-lg flex items-center justify-center flex-shrink-0"
                style={{
                  backgroundColor: `${agent.color}15`,
                  border: `1px solid ${agent.color}30`,
                }}
              >
                <Icon className="w-5 h-5" style={{ color: agent.color }} />
              </div>
              <div>
                <h3 className="text-sm font-bold font-mono leading-tight" style={{ color: agent.color }}>
                  {agent.displayName}
                </h3>
                <span className={cn('text-[11px] font-mono', t.textMuted)}>{agent.role}</span>
              </div>
            </div>

            <div className="flex flex-col items-end gap-1.5">
              <StatusIndicator status={agent.status} label={statusMap[agent.status]} size="sm" />
              <Badge variant={tierCfg.badgeVariant} size="sm">
                {tierCfg.label}
              </Badge>
            </div>
          </div>

          {/* Description */}
          <p className={cn('text-xs leading-relaxed', t.textMuted)}>{agent.description}</p>

          {/* Pipeline strip */}
          <PipelineStrip activePhase={agent.pipelinePhase} agentColor={agent.color} />
        </div>
      </Card>
    </motion.div>
  );
}

// ============================================================================
// MAIN COMPONENT
// ============================================================================

export function AgentsView(): ReactNode {
  const t = useViewTheme();
  const [tierFilter, setTierFilter] = useState<TierFilter>('all');

  const filteredAgents = useMemo(() => {
    if (tierFilter === 'all') return AGENTS;
    return AGENTS.filter((a) => a.tier === tierFilter);
  }, [tierFilter]);

  const tierCounts = useMemo(() => {
    const counts: Record<AgentTier, number> = {
      commander: 0,
      coordinator: 0,
      executor: 0,
    };
    for (const agent of AGENTS) {
      counts[agent.tier]++;
    }
    return counts;
  }, []);

  const onlineCount = AGENTS.filter((a) => a.status === 'online').length;

  return (
    <div className="flex flex-col h-full overflow-hidden">
      {/* Header */}
      <div className={cn('px-6 py-4 border-b', t.border)}>
        <div className="flex items-center justify-between">
          <div>
            <h2 className={cn('text-xl font-bold font-mono', t.title)}>Hydra Agents</h2>
            <p className={cn('text-sm mt-1 font-mono', t.textMuted)}>
              {AGENTS.length} agents &middot; {onlineCount} active &middot; {tierCounts.commander} Commander &middot;{' '}
              {tierCounts.coordinator} Coordinators &middot; {tierCounts.executor} Executors
            </p>
          </div>

          {/* Pipeline legend */}
          <div
            className={cn(
              'hidden md:flex items-center gap-1.5 px-3 py-1.5 rounded-lg border text-[10px] font-mono',
              t.isLight ? 'bg-white/40 border-slate-200/50 text-slate-500' : 'bg-white/5 border-white/10 text-white/40',
            )}
          >
            {PIPELINE_PHASES.map((phase, idx) => (
              <span key={phase} className="flex items-center gap-1">
                <span>{phase}</span>
                {idx < PIPELINE_PHASES.length - 1 && <ChevronRight size={8} className="opacity-40" />}
              </span>
            ))}
          </div>
        </div>

        {/* Tier filter buttons */}
        <div className="flex items-center gap-2 mt-4">
          {FILTER_OPTIONS.map((opt) => {
            const isActive = tierFilter === opt.value;
            return (
              <motion.button
                key={opt.value}
                onClick={() => setTierFilter(opt.value)}
                className={cn(
                  'px-3 py-1.5 rounded-lg text-xs font-mono border transition-all',
                  isActive
                    ? t.isLight
                      ? 'bg-emerald-500/15 border-emerald-500/30 text-emerald-700 font-semibold'
                      : 'bg-white/15 border-white/30 text-white font-semibold'
                    : t.isLight
                      ? 'bg-white/30 border-slate-200/40 text-slate-500 hover:bg-white/50'
                      : 'bg-white/5 border-white/10 text-white/40 hover:bg-white/10',
                )}
                whileHover={{ scale: 1.03 }}
                whileTap={{ scale: 0.97 }}
              >
                {opt.label}
                {opt.value !== 'all' && <span className="ml-1 opacity-60">({tierCounts[opt.value as AgentTier]})</span>}
              </motion.button>
            );
          })}
        </div>
      </div>

      {/* Agent Grid */}
      <div className={cn('flex-1 overflow-y-auto p-6', t.scrollbar)}>
        <AnimatePresence mode="popLayout">
          <motion.div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4" layout>
            {filteredAgents.map((agent) => (
              <AgentCard key={agent.id} agent={agent} />
            ))}
          </motion.div>
        </AnimatePresence>

        {filteredAgents.length === 0 && (
          <div className="flex flex-col items-center justify-center h-40 gap-3">
            <p className={cn('text-sm font-mono', t.empty)}>No agents match the selected filter.</p>
          </div>
        )}
      </div>
    </div>
  );
}

export default AgentsView;
