// src/features/agents/components/AgentEditor.tsx
import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Input } from '@/components/atoms';
import type { Agent } from '@/shared/api/schemas';
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
  const { t: tr } = useTranslation();
  const t = useViewTheme();
  const [formData, setFormData] = useState<Agent>(DEFAULT_AGENT);
  const nameInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (isOpen) {
      if (agent) {
        setFormData(agent);
      } else {
        setFormData({ ...DEFAULT_AGENT, id: crypto.randomUUID() });
      }
      // Auto-focus the name input when the modal opens
      requestAnimationFrame(() => {
        nameInputRef.current?.focus();
      });
    }
  }, [isOpen, agent]);

  // Close on Escape key
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    },
    [onClose],
  );

  useEffect(() => {
    if (!isOpen) return;
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, handleKeyDown]);

  const isNameValid = formData.name.trim().length > 0;

  const handleChange = (field: keyof Agent, value: string | string[]) => {
    setFormData((prev) => ({ ...prev, [field]: value }));
  };

  const handleKeywordsChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = e.target.value;
    const keywords = val
      .split(',')
      .map((k) => k.trim())
      .filter((k) => k);
    handleChange('keywords', keywords);
  };

  if (!isOpen) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm"
      role="dialog"
      aria-modal="true"
      aria-labelledby="agent-editor-title"
    >
      <div
        className={cn(
          'w-full max-w-2xl p-6 rounded-xl border shadow-xl',
          t.isLight ? 'bg-white border-slate-200' : 'bg-slate-900 border-white/10',
        )}
      >
        <h2 id="agent-editor-title" className={cn('text-xl font-bold mb-4', t.title)}>
          {agent ? tr('agents.editAgent', 'Edit Agent') : tr('agents.createAgent', 'Create New Agent')}
        </h2>

        <div className="space-y-4 max-h-[70vh] overflow-y-auto pr-2">
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-1">
              <label htmlFor="agent-name" className="text-xs font-mono opacity-70">
                Name
              </label>
              <Input
                ref={nameInputRef}
                id="agent-name"
                value={formData.name}
                onChange={(e) => handleChange('name', e.target.value)}
                placeholder={tr('agents.agentName', 'Agent Name')}
              />
            </div>
            <div className="space-y-1">
              <label htmlFor="agent-role" className="text-xs font-mono opacity-70">
                Role
              </label>
              <Input
                id="agent-role"
                value={formData.role}
                onChange={(e) => handleChange('role', e.target.value)}
                placeholder={tr('agents.role', 'Role (e.g. Backend)')}
              />
            </div>
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-1">
              <label htmlFor="agent-tier" className="text-xs font-mono opacity-70">
                Tier
              </label>
              <select
                id="agent-tier"
                className={cn(
                  'w-full h-10 px-3 rounded-lg border bg-transparent outline-none font-mono text-sm transition-all',
                  t.input,
                )}
                value={formData.tier}
                onChange={(e) => handleChange('tier', e.target.value)}
              >
                <option value="executor">Executor</option>
                <option value="coordinator">{tr('agents.coordinator', 'Coordinator')}</option>
                <option value="commander">Commander</option>
              </select>
            </div>
            <div className="space-y-1">
              <label htmlFor="agent-status" className="text-xs font-mono opacity-70">
                Status
              </label>
              <select
                id="agent-status"
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
            <label htmlFor="agent-description" className="text-xs font-mono opacity-70">
              {tr('agents.description', 'Description')}
            </label>
            <textarea
              id="agent-description"
              className={cn(
                'w-full p-3 rounded-lg border bg-transparent outline-none font-mono text-sm transition-all min-h-[80px]',
                t.input,
              )}
              value={formData.description}
              onChange={(e) => handleChange('description', e.target.value)}
              placeholder={tr('agents.agentDescription', 'Agent description...')}
            />
          </div>

          <div className="space-y-1">
            <label htmlFor="agent-keywords" className="text-xs font-mono opacity-70">
              Keywords (comma separated)
            </label>
            <Input
              id="agent-keywords"
              value={formData.keywords.join(', ')}
              onChange={handleKeywordsChange}
              placeholder={tr('agents.keywords', 'sql, database, query...')}
            />
          </div>

          <div className="space-y-1">
            <label htmlFor="agent-system-prompt" className="text-xs font-mono opacity-70">
              System Prompt (Override)
            </label>
            <textarea
              id="agent-system-prompt"
              className={cn(
                'w-full p-3 rounded-lg border bg-transparent outline-none font-mono text-sm transition-all min-h-[120px]',
                t.input,
              )}
              value={formData.system_prompt || ''}
              onChange={(e) => handleChange('system_prompt', e.target.value)}
              placeholder={tr('agents.systemPrompt', 'Custom system instructions...')}
            />
          </div>
        </div>

        <div className="flex justify-end gap-3 mt-6 pt-4 border-t border-white/10">
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button onClick={() => onSave(formData)} disabled={!isNameValid}>
            {agent ? tr('agents.saveChanges', 'Save Changes') : tr('agents.createAgentBtn', 'Create Agent')}
          </Button>
        </div>
      </div>
    </div>
  );
}
