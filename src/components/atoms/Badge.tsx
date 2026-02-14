import { cva, type VariantProps } from 'class-variance-authority';
import { forwardRef, type HTMLAttributes, type ReactNode } from 'react';
import { cn } from '@/shared/utils/cn';

// ============================================
// BADGE VARIANTS (GeminiHydra — White/Neutral Accent)
// ============================================

const badgeVariants = cva('inline-flex items-center gap-1 font-medium rounded-full transition-colors font-mono', {
  variants: {
    variant: {
      default: ['bg-[var(--matrix-badge-bg)]', 'text-matrix-text-dim', 'border border-matrix-accent/10'].join(' '),
      accent: ['bg-matrix-accent/15 text-matrix-accent', 'border border-matrix-accent/30'].join(' '),
      success: ['bg-[var(--matrix-success-bg)] text-[var(--matrix-success)]'].join(' '),
      warning: ['bg-[var(--matrix-warning-bg)] text-[var(--matrix-warning)]'].join(' '),
      error: ['bg-[var(--matrix-error-bg)] text-[var(--matrix-error)]'].join(' '),
    },
    size: {
      sm: 'text-xs px-2 py-0.5',
      md: 'text-sm px-2.5 py-1',
    },
  },
  defaultVariants: {
    variant: 'default',
    size: 'sm',
  },
});

// ============================================
// DOT COLORS — used for the dot indicator
// ============================================

const dotColorMap: Record<string, string> = {
  default: 'bg-matrix-text-dim',
  accent: 'bg-matrix-accent',
  success: 'bg-[var(--matrix-success)]',
  warning: 'bg-[var(--matrix-warning)]',
  error: 'bg-[var(--matrix-error)]',
};

// ============================================
// BADGE COMPONENT
// ============================================

export interface BadgeProps extends HTMLAttributes<HTMLSpanElement>, VariantProps<typeof badgeVariants> {
  /** Show a small colored dot before the text */
  dot?: boolean;
  /** Optional icon before the text (ignored if dot is true) */
  icon?: ReactNode;
  children: ReactNode;
}

export const Badge = forwardRef<HTMLSpanElement, BadgeProps>(
  ({ className, variant, size, dot = false, icon, children, ...props }, ref) => {
    const resolvedVariant = variant ?? 'default';
    const dotColor = dotColorMap[resolvedVariant] ?? dotColorMap.default;

    return (
      <span ref={ref} className={cn(badgeVariants({ variant, size }), className)} {...props}>
        {dot && <span className={cn('h-1.5 w-1.5 rounded-full flex-shrink-0', dotColor)} aria-hidden="true" />}
        {!dot && icon && <span className="flex-shrink-0">{icon}</span>}
        {children}
      </span>
    );
  },
);

Badge.displayName = 'Badge';
