// src/features/agents/components/AgentEditor.tsx
import { useEffect, useState } from 'react';
import { Button, Input } from '@/components/atoms';
import { Agent } from '@/shared/api/schemas';
import { useViewTheme } from '@/shared/hooks/useViewTheme';
import { cn } from '@/shared/utils/cn';

interface AgentEditorProps {
  agent?: Agent | null; // null = create mode
  isOpen: boolean;
  onClose: () => void;
  onSave: (agent: Agent) => void;
}

const DEFAULT_AGENT: Agent = {
  id: '',
  name: '',
  role: '',
  tier: 'executor',
  status: 'online',
  description: '',
  keywords: [],
  system_prompt: '',
};

export function AgentEditor({ agent, isOpen, onClose, onSave }: AgentEditorProps) {
  const t = useViewTheme();
  const [formData, setFormData] = useState<Agent>(DEFAULT_AGENT);

  useEffect(() => {
    if (isOpen) {
      if (agent) {
        setFormData(agent);
      } else {
        setFormData({ ...DEFAULT_AGENT, id: crypto.randomUUID() });
      }
    }
  }, [isOpen, agent]);

  const handleChange = (field: keyof Agent, value: string | string[]) => {
    setFormData((prev) => ({ ...prev, [field]: value }));
  };

  const handleKeywordsChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = e.target.value;
    const keywords = val.split(',').map((k) => k.trim()).filter((k) => k);
    handleChange('keywords', keywords);
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm">
      <div
        className={cn(
          'w-full max-w-2xl p-6 rounded-xl border shadow-xl',
          t.isLight ? 'bg-white border-slate-200' : 'bg-slate-900 border-white/10',
        )}
      >
        <h2 className={cn('text-xl font-bold mb-4', t.title)}>
          {agent ? 'Edit Agent' : 'Create New Agent'}
        </h2>

        <div className="space-y-4 max-h-[70vh] overflow-y-auto pr-2">
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-1">
              <label className="text-xs font-mono opacity-70">Name</label>
              <Input
                value={formData.name}
                onChange={(e) => handleChange('name', e.target.value)}
                placeholder="Agent Name"
              />
            </div>
            <div className="space-y-1">
              <label className="text-xs font-mono opacity-70">Role</label>
              <Input
                value={formData.role}
                onChange={(e) => handleChange('role', e.target.value)}
                placeholder="Role (e.g. Backend)"
              />
            </div>
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-1">
              <label className="text-xs font-mono opacity-70">Tier</label>
              <select
                className={cn(
                  'w-full h-10 px-3 rounded-lg border bg-transparent outline-none font-mono text-sm transition-all',
                  t.input,
                )}
                value={formData.tier}
                onChange={(e) => handleChange('tier', e.target.value)}
              >
                <option value="executor">Executor</option>
                <option value="coordinator">Coordinator</option>
                <option value="commander">Commander</option>
              </select>
            </div>
            <div className="space-y-1">
              <label className="text-xs font-mono opacity-70">Status</label>
              <select
                className={cn(
                  'w-full h-10 px-3 rounded-lg border bg-transparent outline-none font-mono text-sm transition-all',
                  t.input,
                )}
                value={formData.status}
                onChange={(e) => handleChange('status', e.target.value)}
              >
                <option value="online">Online</option>
                <option value="offline">Offline</option>
                <option value="pending">Pending</option>
                <option value="error">Error</option>
              </select>
            </div>
          </div>

          <div className="space-y-1">
            <label className="text-xs font-mono opacity-70">Description</label>
            <textarea
              className={cn(
                'w-full p-3 rounded-lg border bg-transparent outline-none font-mono text-sm transition-all min-h-[80px]',
                t.input,
              )}
              value={formData.description}
              onChange={(e) => handleChange('description', e.target.value)}
              placeholder="Agent description..."
            />
          </div>

          <div className="space-y-1">
            <label className="text-xs font-mono opacity-70">Keywords (comma separated)</label>
            <Input
              value={formData.keywords.join(', ')}
              onChange={handleKeywordsChange}
              placeholder="sql, database, query..."
            />
          </div>

          <div className="space-y-1">
            <label className="text-xs font-mono opacity-70">System Prompt (Override)</label>
            <textarea
              className={cn(
                'w-full p-3 rounded-lg border bg-transparent outline-none font-mono text-sm transition-all min-h-[120px]',
                t.input,
              )}
              value={formData.system_prompt || ''}
              onChange={(e) => handleChange('system_prompt', e.target.value)}
              placeholder="Custom system instructions..."
            />
          </div>
        </div>

        <div className="flex justify-end gap-3 mt-6 pt-4 border-t border-white/10">
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button onClick={() => onSave(formData)}>
            {agent ? 'Save Changes' : 'Create Agent'}
          </Button>
        </div>
      </div>
    </div>
  );
}
