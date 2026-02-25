// src/features/memory/components/KnowledgeGraphView.tsx

import { useQuery } from '@tanstack/react-query';
import { Share2 } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { EmptyState } from '@/components/molecules/EmptyState';
import { QueryError } from '@/components/molecules/QueryError';
import { apiGet } from '@/shared/api/client';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';

// ============================================================================
// TYPES
// ============================================================================

interface Node {
  id: string;
  label: string;
  node_type: string;
  x: number;
  y: number;
  vx: number;
  vy: number;
}

interface Edge {
  source: string;
  target: string;
  label: string;
}

interface GraphData {
  nodes: Node[];
  edges: Edge[];
}

// ============================================================================
// DATA FETCHING
// ============================================================================

function useKnowledgeGraph() {
  return useQuery<GraphData>({
    queryKey: ['knowledge-graph'],
    queryFn: async () => {
      // Fetch from backend
      const res = await apiGet<{ nodes: Node[]; edges: Edge[] }>('/api/memory/graph');

      // Initialize with random positions
      const nodes = res.nodes.map((n) => ({
        ...n,
        x: Math.random() * 800 - 400,
        y: Math.random() * 600 - 300,
        vx: 0,
        vy: 0,
      }));

      return { nodes, edges: res.edges };
    },
  });
}

// ============================================================================
// SIMULATION
// ============================================================================

const REPULSION = 5000;
const SPRING_LENGTH = 100;
const SPRING_STRENGTH = 0.05;
const DAMPING = 0.9;
const CENTER_STRENGTH = 0.01;

function runSimulation(nodes: Node[], edges: Edge[]): number {
  // Repulsion
  for (let i = 0; i < nodes.length; i++) {
    const n1 = nodes[i];
    if (!n1) continue;

    // Center pull
    n1.vx -= n1.x * CENTER_STRENGTH;
    n1.vy -= n1.y * CENTER_STRENGTH;

    for (let j = i + 1; j < nodes.length; j++) {
      const n2 = nodes[j];
      if (!n2) continue;

      const dx = n1.x - n2.x;
      const dy = n1.y - n2.y;
      const distSq = dx * dx + dy * dy || 1;
      const dist = Math.sqrt(distSq);

      const force = REPULSION / distSq;
      const fx = (dx / dist) * force;
      const fy = (dy / dist) * force;

      n1.vx += fx;
      n1.vy += fy;
      n2.vx -= fx;
      n2.vy -= fy;
    }
  }

  // Springs
  edges.forEach((edge) => {
    const source = nodes.find((n) => n.id === edge.source);
    const target = nodes.find((n) => n.id === edge.target);
    if (source && target) {
      const dx = target.x - source.x;
      const dy = target.y - source.y;
      const dist = Math.sqrt(dx * dx + dy * dy) || 1;

      const stretch = dist - SPRING_LENGTH;
      const force = stretch * SPRING_STRENGTH;
      const fx = (dx / dist) * force;
      const fy = (dy / dist) * force;

      source.vx += fx;
      source.vy += fy;
      target.vx -= fx;
      target.vy -= fy;
    }
  });

  // Update positions
  nodes.forEach((n) => {
    n.vx *= DAMPING;
    n.vy *= DAMPING;
    n.x += n.vx;
    n.y += n.vy;
  });

  // Compute total kinetic energy for convergence detection
  let totalKineticEnergy = 0;
  for (const node of nodes) {
    totalKineticEnergy += node.vx * node.vx + node.vy * node.vy;
  }
  return totalKineticEnergy;
}

// ============================================================================
// COMPONENT
// ============================================================================

export function KnowledgeGraphView() {
  const { t: tr } = useTranslation();
  const t = useViewTheme();
  const { data, isLoading, isError, refetch } = useKnowledgeGraph();
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [nodes, setNodes] = useState<Node[]>([]);
  const requestRef = useRef<number>(0);
  const convergedFramesRef = useRef(0);

  // Sync data to local state
  useEffect(() => {
    if (data) {
      setNodes(JSON.parse(JSON.stringify(data.nodes))); // Deep copy to avoid mutating cache
    }
  }, [data]);

  // Animation Loop
  useEffect(() => {
    if (!nodes.length || !data?.edges) return;

    const animate = () => {
      const canvas = canvasRef.current;
      if (!canvas) return;

      const ctx = canvas.getContext('2d');
      if (!ctx) return;

      const { width, height } = canvas.getBoundingClientRect();
      // Handle high DPI
      const dpr = window.devicePixelRatio || 1;
      if (canvas.width !== width * dpr || canvas.height !== height * dpr) {
        canvas.width = width * dpr;
        canvas.height = height * dpr;
        ctx.scale(dpr, dpr);
      }

      const totalKineticEnergy = runSimulation(nodes, data.edges);

      // If converged, reduce frame rate by skipping frames
      if (totalKineticEnergy < 0.01) {
        convergedFramesRef.current++;
        if (convergedFramesRef.current % 10 !== 0) {
          requestRef.current = requestAnimationFrame(animate);
          return; // skip draw, just request next frame
        }
      } else {
        convergedFramesRef.current = 0;
      }

      // Render
      ctx.clearRect(0, 0, width, height);
      ctx.save();
      ctx.translate(width / 2, height / 2); // Center 0,0

      // Draw Edges
      ctx.strokeStyle = t.isLight ? 'rgba(0,0,0,0.1)' : 'rgba(255,255,255,0.1)';
      ctx.lineWidth = 1;
      data.edges.forEach((edge) => {
        const s = nodes.find((n) => n.id === edge.source);
        const tg = nodes.find((n) => n.id === edge.target);
        if (s && tg) {
          ctx.beginPath();
          ctx.moveTo(s.x, s.y);
          ctx.lineTo(tg.x, tg.y);
          ctx.stroke();
        }
      });

      // Draw Nodes
      nodes.forEach((node) => {
        ctx.beginPath();
        ctx.arc(node.x, node.y, 6, 0, Math.PI * 2);

        // Color by type
        if (node.node_type === 'agent') ctx.fillStyle = '#FFD700';
        else if (node.node_type === 'concept') ctx.fillStyle = '#00CED1';
        else ctx.fillStyle = '#9370DB';

        ctx.fill();

        // Label
        ctx.fillStyle = t.isLight ? '#333' : '#ccc';
        ctx.font = '10px monospace';
        ctx.fillText(node.label, node.x + 8, node.y + 3);
      });

      ctx.restore();
      requestRef.current = requestAnimationFrame(animate);
    };

    requestRef.current = requestAnimationFrame(animate);
    return () => cancelAnimationFrame(requestRef.current);
  }, [nodes, data, t]);

  return (
    <div className="flex flex-col h-full overflow-hidden relative">
      <div className={cn('px-6 py-4 border-b flex justify-between items-center', t.border)}>
        <div>
          <h2 className={cn('text-xl font-bold font-mono', t.title)}>{tr('memory.neuralNetwork', 'Neural Network')}</h2>
          <p className={cn('text-sm mt-1 font-mono', t.textMuted)}>
            Knowledge Graph Visualization &middot; {nodes.length} nodes
          </p>
        </div>
        <button
          type="button"
          onClick={() => refetch()}
          className={cn(
            'px-3 py-1.5 rounded border text-xs font-mono transition-colors',
            t.isLight ? 'bg-white hover:bg-slate-50' : 'bg-white/10 hover:bg-white/20',
          )}
        >
          Refresh
        </button>
      </div>

      <div className={cn('flex-1 relative', t.isLight ? 'bg-slate-50' : 'bg-black/90')}>
        {isLoading && (
          <div className="absolute inset-0 flex items-center justify-center text-white/50 font-mono z-10">
            Loading neural data...
          </div>
        )}

        {isError && !isLoading && (
          <div className="absolute inset-0 flex items-center justify-center z-10">
            <QueryError onRetry={() => refetch()} />
          </div>
        )}

        {nodes.length === 0 && !isLoading && !isError && (
          <div className="absolute inset-0 flex items-center justify-center z-10">
            <EmptyState
              icon={Share2}
              title={tr('memory.noNodes', 'No knowledge nodes found')}
              description={tr('memory.noNodesDesc', 'Start a conversation to build the knowledge graph.')}
            />
          </div>
        )}

        <canvas ref={canvasRef} className="absolute inset-0 w-full h-full block" />
      </div>
    </div>
  );
}

export default KnowledgeGraphView;
