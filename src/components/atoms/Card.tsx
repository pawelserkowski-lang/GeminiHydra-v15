import { cva, type VariantProps } from 'class-variance-authority';
import { forwardRef, type HTMLAttributes, type ReactNode } from 'react';
import { cn } from '@/shared/utils/cn';

// ============================================
// CARD VARIANTS (GeminiHydra — Glass Panel Design)
// ============================================

const cardVariants = cva('rounded-xl transition-all duration-200', {
  variants: {
    variant: {
      default: 'bg-white/5 border border-white/10',
      glass: 'glass-panel',
      elevated: 'bg-white/5 border border-white/10 shadow-lg',
      hover: ['glass-panel', 'hover:border-white/20', 'hover:shadow-[0_0_20px_rgba(255,255,255,0.05)]'].join(' '),
    },
    padding: {
      none: '',
      sm: 'p-3',
      md: 'p-4',
      lg: 'p-6',
    },
    interactive: {
      true: 'cursor-pointer hover:shadow-lg hover:-translate-y-0.5',
      false: '',
    },
  },
  defaultVariants: {
    variant: 'default',
    padding: 'md',
    interactive: false,
  },
});

// ============================================
// CARD COMPONENT
// ============================================

export interface CardProps extends HTMLAttributes<HTMLDivElement>, VariantProps<typeof cardVariants> {
  /** Optional header content rendered above children with a bottom border */
  header?: ReactNode;
  /** Optional footer content rendered below children with a top border */
  footer?: ReactNode;
}

export const Card = forwardRef<HTMLDivElement, CardProps>(
  ({ className, variant, padding, interactive, header, footer, children, ...props }, ref) => {
    return (
      <div ref={ref} className={cn(cardVariants({ variant, padding, interactive }), className)} {...props}>
        {header && (
          <div className="flex items-center justify-between pb-4 border-b border-[var(--matrix-divider)] mb-4">
            {header}
          </div>
        )}
        {children}
        {footer && (
          <div className="flex items-center justify-between pt-4 border-t border-[var(--matrix-divider)] mt-4">
            {footer}
          </div>
        )}
      </div>
    );
  },
);

Card.displayName = 'Card';
