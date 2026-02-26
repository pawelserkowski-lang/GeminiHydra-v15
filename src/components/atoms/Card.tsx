import { cva, type VariantProps } from 'class-variance-authority';
import { forwardRef, type HTMLAttributes, memo, type ReactNode } from 'react';
import { cn } from '@/shared/utils/cn';

// ============================================
// CARD VARIANTS (Jaskier Design System — CSS Variable Glass)
// ============================================

const cardVariants = cva('rounded-xl transition-all duration-200', {
  variants: {
    variant: {
      default: ['bg-matrix-glass', 'border border-matrix-border'].join(' '),
      glass: 'glass-panel',
      elevated: ['bg-matrix-glass', 'border border-matrix-border', 'shadow-lg'].join(' '),
      hover: ['glass-panel', 'hover:border-matrix-accent-dim', 'hover:shadow-[0_0_20px_rgba(255,255,255,0.08)]'].join(
        ' ',
      ),
    },
    padding: {
      none: '',
      sm: 'p-3',
      md: 'p-4',
      lg: 'p-6',
    },
    interactive: {
      true: [
        'cursor-pointer',
        'hover:-translate-y-0.5',
        'hover:shadow-[0_8px_25px_rgba(0,0,0,0.3)]',
        'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--matrix-accent)]',
        'focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--matrix-bg-primary)]',
      ].join(' '),
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

export const Card = memo(
  forwardRef<HTMLDivElement, CardProps>(
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
  ),
);

Card.displayName = 'Card';
