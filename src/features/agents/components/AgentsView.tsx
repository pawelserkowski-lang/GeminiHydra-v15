// src/features/agents/components/AgentsView.tsx
/**
 * AgentsView - Witcher swarm agent dashboard
 * ============================================
 * Displays agents from the database with CRUD capabilities.
 * Dynamic loading, editing, and creation of new agents.
 */

import {
  ChevronRight,
  Crown,
  Edit,
  Eye,
  Flame,
  Flower2,
  Gem,
  type LucideIcon,
  Mountain,
  Music,
  Plus,
  Shield,
  Sword,
  Trash2,
  Wand2,
  Zap,
} from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { type ReactNode, useMemo, useState } from 'react';
import { Badge, Button, Card } from '@/components/atoms';
import { StatusIndicator, type StatusState } from '@/components/molecules';
import {
  useAgentsQuery,
  useCreateAgentMutation,
  useDeleteAgentMutation,
  useUpdateAgentMutation,
} from '@/features/agents/hooks/useAgents';
import { Agent } from '@/shared/api/schemas';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';
import { AgentEditor } from './AgentEditor';

// ============================================================================
// TYPES
// ============================================================================

type AgentTier = 'commander' | 'coordinator' | 'executor';
type PipelinePhase = 'PRE-A' | 'A' | 'B' | 'C' | 'D';

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
// HELPERS
// ============================================================================

function getAgentIcon(role: string): LucideIcon {
  const r = role.toLowerCase();
  if (r.includes('security')) return Shield;
  if (r.includes('architect')) return Crown;
  if (r.includes('data')) return Gem;
  if (r.includes('document')) return Music;
  if (r.includes('test')) return Flower2;
  if (r.includes('speed') || r.includes('performance')) return Zap;
  if (r.includes('devops')) return Mountain;
  if (r.includes('backend')) return Flame;
  if (r.includes('frontend')) return Wand2;
  if (r.includes('research')) return Eye;
  return Sword; // Default
}

function getAgentColor(name: string): string {
  let hash = 0;
  for (let i = 0; i < name.length; i++) {
    hash = name.charCodeAt(i) + ((hash << 5) - hash);
  }
  const c = (hash & 0x00ffffff).toString(16).toUpperCase();
  return '#' + '00000'.substring(0, 6 - c.length) + c;
}

function getAgentPhase(role: string): PipelinePhase {
  const r = role.toLowerCase();
  if (r.includes('security')) return 'PRE-A';
  if (r.includes('architect') || r.includes('research') || r.includes('strateg')) return 'A';
  if (r.includes('frontend') || r.includes('backend') || r.includes('devops')) return 'B';
  if (r.includes('test') || r.includes('qa')) return 'C';
  return 'D';
}

// ============================================================================
// SUB-COMPONENTS
// ============================================================================

const TIER_CONFIG: Record<string, { label: string; badgeVariant: 'accent' | 'warning' | 'default' }> = {
  commander: { label: 'Commander', badgeVariant: 'warning' },
  coordinator: { label: 'Coordinator', badgeVariant: 'accent' },
  executor: { label: 'Executor', badgeVariant: 'default' },
};

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
              {...(isActive && {
                style: {
                  backgroundColor: `${agentColor}20`,
                  borderColor: `${agentColor}50`,
                  color: agentColor,
                },
              })}
              title={PIPELINE_LABELS[phase]}
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

function AgentCard({ agent, onEdit, onDelete }: { agent: Agent; onEdit: () => void; onDelete: () => void }) {
  const t = useViewTheme();
  const Icon = getAgentIcon(agent.role);
  const color = getAgentColor(agent.name);
  const phase = getAgentPhase(agent.role);
  const tierKey = agent.tier.toLowerCase();
  const tierCfg = TIER_CONFIG[tierKey] ?? TIER_CONFIG['executor'] ?? { label: 'Executor', badgeVariant: 'default' };

  return (
    <motion.div
      layout
      initial={{ opacity: 0, scale: 0.9 }}
      animate={{ opacity: 1, scale: 1 }}
      exit={{ opacity: 0, scale: 0.9 }}
      transition={{ duration: 0.2 }}
    >
      <Card
        variant="hover"
        padding="none"
        className={cn(
          'group relative overflow-hidden',
          t.isLight && 'bg-white/50 border-white/30 hover:border-emerald-400/40 hover:shadow-lg',
        )}
      >
        <div className="h-0.5 w-full" style={{ backgroundColor: color }} />

        <div className="p-4 space-y-3">
          <div className="flex items-start justify-between">
            <div className="flex items-center gap-3">
              <div
                className="w-10 h-10 rounded-lg flex items-center justify-center flex-shrink-0"
                style={{
                  backgroundColor: `${color}15`,
                  border: `1px solid ${color}30`,
                }}
              >
                <Icon className="w-5 h-5" style={{ color }} />
              </div>
              <div>
                <h3 className="text-sm font-bold font-mono leading-tight" style={{ color }}>
                  {agent.name}
                </h3>
                <span className={cn('text-[11px] font-mono', t.textMuted)}>{agent.role}</span>
              </div>
            </div>

            <div className="flex flex-col items-end gap-1.5">
              <StatusIndicator status={agent.status as StatusState} size="sm" />
              <Badge variant={tierCfg.badgeVariant} size="sm">
                {tierCfg.label}
              </Badge>
            </div>
          </div>

          <p className={cn('text-xs leading-relaxed line-clamp-2', t.textMuted)}>{agent.description}</p>

          <div className="flex items-center justify-between mt-2">
            <PipelineStrip activePhase={phase} agentColor={color} />
            <div className="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
              <button onClick={onEdit} className="p-1 hover:bg-white/10 rounded">
                <Edit size={14} className={t.textMuted} />
              </button>
              <button onClick={onDelete} className="p-1 hover:bg-red-500/20 rounded">
                <Trash2 size={14} className="text-red-400" />
              </button>
            </div>
          </div>
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
  const { data, isLoading } = useAgentsQuery();
  const createMutation = useCreateAgentMutation();
  const updateMutation = useUpdateAgentMutation();
  const deleteMutation = useDeleteAgentMutation();

  const [tierFilter, setTierFilter] = useState<TierFilter>('all');
  const [editorOpen, setEditorOpen] = useState(false);
  const [editingAgent, setEditingAgent] = useState<Agent | null>(null);

  const agents = data?.agents || [];

  const filteredAgents = useMemo(() => {
    if (tierFilter === 'all') return agents;
    return agents.filter((a) => a.tier.toLowerCase() === tierFilter);
  }, [tierFilter, agents]);

  const tierCounts = useMemo(() => {
    const counts: Record<string, number> = { commander: 0, coordinator: 0, executor: 0 };
    for (const agent of agents) {
      const tier = agent.tier.toLowerCase();
      counts[tier] = (counts[tier] || 0) + 1;
    }
    return counts;
  }, [agents]);

  const handleSave = (agent: Agent) => {
    if (editingAgent) {
      updateMutation.mutate({ id: agent.id, agent });
    } else {
      createMutation.mutate(agent);
    }
    setEditorOpen(false);
    setEditingAgent(null);
  };

  const handleDelete = (id: string) => {
    if (confirm('Are you sure you want to retire this agent?')) {
      deleteMutation.mutate(id);
    }
  };

  if (isLoading) return <div className="p-6">Loading agents...</div>;

  return (
    <div className="flex flex-col h-full overflow-hidden">
      {/* Header */}
      <div className={cn('px-6 py-4 border-b', t.border)}>
        <div className="flex items-center justify-between">
          <div>
            <h2 className={cn('text-xl font-bold font-mono', t.title)}>Hydra Agents</h2>
            <p className={cn('text-sm mt-1 font-mono', t.textMuted)}>
              {agents.length} agents active &middot; {tierCounts.commander} Cmd &middot; {tierCounts.coordinator} Coord
              &middot; {tierCounts.executor} Exec
            </p>
          </div>
          <Button onClick={() => { setEditingAgent(null); setEditorOpen(true); }} size="sm">
            <Plus size={16} className="mr-2" />
            New Agent
          </Button>
        </div>

        <div className="flex items-center gap-2 mt-4">
          {(['all', 'commander', 'coordinator', 'executor'] as const).map((filter) => (
            <motion.button
              key={filter}
              onClick={() => setTierFilter(filter as TierFilter)}
              className={cn(
                'px-3 py-1.5 rounded-lg text-xs font-mono border transition-all',
                tierFilter === filter
                  ? 'bg-white/15 border-white/30 text-white font-semibold'
                  : 'bg-white/5 border-white/10 text-white/40 hover:bg-white/10',
              )}
              whileHover={{ scale: 1.03 }}
              whileTap={{ scale: 0.97 }}
            >
              {filter.charAt(0).toUpperCase() + filter.slice(1)}
            </motion.button>
          ))}
        </div>
      </div>

      {/* Grid */}
      <div className={cn('flex-1 overflow-y-auto p-6', t.scrollbar)}>
        <AnimatePresence mode="popLayout">
          <motion.div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4" layout>
            {filteredAgents.map((agent) => (
              <AgentCard
                key={agent.id}
                agent={agent}
                onEdit={() => { setEditingAgent(agent); setEditorOpen(true); }}
                onDelete={() => handleDelete(agent.id)}
              />
            ))}
          </motion.div>
        </AnimatePresence>
      </div>

      <AgentEditor
        isOpen={editorOpen}
        agent={editingAgent}
        onClose={() => setEditorOpen(false)}
        onSave={handleSave}
      />
    </div>
  );
}

export default AgentsView;
