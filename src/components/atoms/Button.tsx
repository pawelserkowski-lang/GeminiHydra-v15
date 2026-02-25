import { cva, type VariantProps } from 'class-variance-authority';
import { Loader2 } from 'lucide-react';
import { type HTMLMotionProps, motion } from 'motion/react';
import { type ButtonHTMLAttributes, forwardRef, memo, type ReactNode } from 'react';
import { cn } from '@/shared/utils/cn';

// ============================================
// BUTTON VARIANTS (Jaskier Design System)
// ============================================

const buttonVariants = cva(
  [
    'inline-flex items-center justify-center gap-2 font-medium',
    'transition-all duration-200',
    'disabled:opacity-50 disabled:cursor-not-allowed',
    'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--matrix-accent)]',
    'focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--matrix-bg-primary)]',
    'rounded-lg font-mono',
  ].join(' '),
  {
    variants: {
      variant: {
        primary: [
          'bg-matrix-accent text-matrix-bg-primary',
          'hover:bg-matrix-accent-glow',
          'shadow-[0_0_10px_rgba(255,255,255,0.2)]',
          'hover:shadow-[0_0_20px_rgba(255,255,255,0.3)]',
        ].join(' '),
        secondary: ['glass-button', 'text-matrix-text'].join(' '),
        ghost: [
          'bg-transparent',
          'text-matrix-text-dim hover:text-matrix-accent',
          'hover:bg-[var(--matrix-hover-bg)]',
        ].join(' '),
        danger: [
          'bg-[var(--matrix-error-bg)] text-[var(--matrix-error)]',
          'border border-[var(--matrix-error)]/30',
          'hover:border-[var(--matrix-error)]/50',
          'hover:shadow-[0_0_15px_rgba(248,113,113,0.2)]',
        ].join(' '),
      },
      size: {
        sm: 'text-xs px-3 py-1.5',
        md: 'text-sm px-4 py-2',
        lg: 'text-base px-6 py-3',
      },
    },
    defaultVariants: {
      variant: 'primary',
      size: 'md',
    },
  },
);

// ============================================
// TYPES
// ============================================

type MotionButtonProps = HTMLMotionProps<'button'>;

export interface ButtonProps
  extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, keyof MotionButtonProps>,
    MotionButtonProps,
    VariantProps<typeof buttonVariants> {
  /** Show loading spinner and disable interactions */
  isLoading?: boolean;
  /** Text to show while loading (defaults to children) */
  loadingText?: string;
  /** Icon rendered before children */
  leftIcon?: ReactNode;
  /** Icon rendered after children */
  rightIcon?: ReactNode;
}

// ============================================
// COMPONENT
// ============================================

export const Button = memo(forwardRef<HTMLButtonElement, ButtonProps>(
  (
    { className, variant, size, isLoading = false, loadingText, leftIcon, rightIcon, children, disabled, ...props },
    ref,
  ) => {
    return (
      <motion.button
        ref={ref}
        className={cn(buttonVariants({ variant, size }), className)}
        disabled={disabled || isLoading}
        whileHover={disabled || isLoading ? undefined : { scale: 1.02 }}
        whileTap={disabled || isLoading ? undefined : { scale: 0.97 }}
        transition={{ type: 'spring', stiffness: 400, damping: 25 }}
        {...props}
      >
        {isLoading ? (
          <>
            <Loader2 className="h-4 w-4 animate-spin" />
            {loadingText ?? children}
          </>
        ) : (
          <>
            {leftIcon && <span className="flex-shrink-0">{leftIcon}</span>}
            {children}
            {rightIcon && <span className="flex-shrink-0">{rightIcon}</span>}
          </>
        )}
      </motion.button>
    );
  },
));

Button.displayName = 'Button';
