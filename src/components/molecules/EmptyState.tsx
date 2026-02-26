/** Jaskier Design System */

import type { LucideIcon } from 'lucide-react';
import { cn } from '@/shared/utils/cn';

interface EmptyStateProps {
  icon: LucideIcon;
  title: string;
  description?: string;
  action?: React.ReactNode;
  className?: string;
}

export function EmptyState({ icon: Icon, title, description, action, className }: EmptyStateProps) {
  return (
    <div className={cn('flex flex-col items-center justify-center gap-4 py-16 text-center', className)}>
      <div className="rounded-full bg-matrix-glass p-4">
        <Icon className="h-8 w-8 text-[var(--matrix-text-secondary)]" />
      </div>
      <div className="space-y-1">
        <h3 className="text-lg font-medium text-[var(--matrix-text-primary)]">{title}</h3>
        {description && <p className="text-sm text-[var(--matrix-text-secondary)] max-w-sm">{description}</p>}
      </div>
      {action && <div className="mt-2">{action}</div>}
    </div>
  );
}
