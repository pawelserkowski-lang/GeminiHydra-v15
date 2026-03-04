import { HelpCircle } from 'lucide-react';
import { motion } from 'motion/react';
import { useState } from 'react';
import { Button } from '@/components/atoms/Button';
import { Input } from '@/components/atoms/Input';

interface AskUserDialogProps {
  question: string;
  options?: string[];
  onSubmit: (response: string) => void;
}

export function AskUserDialog({ question, options, onSubmit }: AskUserDialogProps) {
  const [value, setValue] = useState('');

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (value.trim()) {
      onSubmit(value.trim());
    }
  };

  return (
    <div className="absolute inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4">
      <motion.div
        initial={{ opacity: 0, scale: 0.95, y: 20 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        exit={{ opacity: 0, scale: 0.95, y: 20 }}
        className="w-full max-w-md bg-[var(--matrix-bg-secondary)] border border-[var(--matrix-border)] rounded-xl shadow-2xl p-6 flex flex-col gap-6"
      >
        <div className="flex items-start gap-4">
          <div className="p-3 bg-[var(--matrix-accent)]/10 rounded-lg text-[var(--matrix-accent)]">
            <HelpCircle size={24} />
          </div>
          <div className="flex-1">
            <h3 className="text-lg font-semibold text-[var(--matrix-text)] mb-2">Pytanie od agenta</h3>
            <p className="text-sm text-[var(--matrix-text-dim)] whitespace-pre-wrap">{question}</p>
          </div>
        </div>

        {options && options.length > 0 ? (
          <div className="flex flex-col gap-2">
            {options.map((opt) => (
              <Button
                key={opt}
                variant="secondary"
                className="w-full justify-start text-left h-auto py-3 px-4"
                onClick={() => onSubmit(opt)}
              >
                {opt}
              </Button>
            ))}
          </div>
        ) : (
          <form onSubmit={handleSubmit} className="flex flex-col gap-4">
            <Input
              autoFocus
              placeholder="Twoja odpowiedź..."
              value={value}
              onChange={(e) => setValue(e.target.value)}
              className="w-full"
            />
            <div className="flex justify-end gap-2 mt-2">
              <Button type="submit" disabled={!value.trim()}>
                Wyślij
              </Button>
            </div>
          </form>
        )}
      </motion.div>
    </div>
  );
}
