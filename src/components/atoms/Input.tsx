// src/components/atoms/Input.tsx
/**
 * Input Atom
 * ==========
 * Glass-styled text input with icon slot, right element slot, error state, label, and size
 * variants. Uses `.glass-input` from globals.css and theme accent for focus.
 * Supports forwardRef for external ref access.
 */

import { forwardRef, type InputHTMLAttributes, type ReactNode, useId } from 'react';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type InputSize = 'sm' | 'md' | 'lg';

interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  /** Optional label rendered above the input. */
  label?: string;
  /** Icon element rendered on the left side of the input. */
  icon?: ReactNode;
  /** Element rendered on the right side of the input (e.g. toggle button). */
  rightElement?: ReactNode;
  /** Error message. When truthy the input shows a red border + message. */
  error?: string;
  /** Size variant controlling padding and font size. */
  inputSize?: InputSize;
  /** Extra CSS classes on the outermost wrapper. */
  className?: string;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const sizeClasses: Record<InputSize, string> = {
  sm: 'px-2.5 py-1.5 text-xs',
  md: 'px-3 py-2 text-sm',
  lg: 'px-4 py-3 text-base',
};

const iconSizeClasses: Record<InputSize, string> = {
  sm: 'left-2 [&>svg]:w-3.5 [&>svg]:h-3.5',
  md: 'left-3 [&>svg]:w-4 [&>svg]:h-4',
  lg: 'left-3.5 [&>svg]:w-5 [&>svg]:h-5',
};

const iconPaddingClasses: Record<InputSize, string> = {
  sm: 'pl-7',
  md: 'pl-9',
  lg: 'pl-11',
};

const rightElementPaddingClasses: Record<InputSize, string> = {
  sm: 'pr-8',
  md: 'pr-10',
  lg: 'pr-12',
};

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export const Input = forwardRef<HTMLInputElement, InputProps>(
  ({ label, icon, rightElement, error, inputSize = 'md', className = '', disabled, id: externalId, ...rest }, ref) => {
    const autoId = useId();
    const inputId = externalId ?? autoId;
    const errorId = error ? `${inputId}-error` : undefined;

    const hasError = Boolean(error);

    return (
      <div className={`flex flex-col gap-1.5 ${className}`}>
        {/* Label */}
        {label && (
          <label htmlFor={inputId} className="text-xs font-medium text-[var(--matrix-text-secondary)]">
            {label}
          </label>
        )}

        {/* Input wrapper */}
        <div className="relative">
          {/* Left icon */}
          {icon && (
            <span
              className={`absolute top-1/2 -translate-y-1/2 text-[var(--matrix-text-secondary)] pointer-events-none ${iconSizeClasses[inputSize]}`}
              aria-hidden="true"
            >
              {icon}
            </span>
          )}

          {/* Input */}
          <input
            ref={ref}
            id={inputId}
            disabled={disabled}
            aria-invalid={hasError || undefined}
            aria-describedby={errorId}
            className={[
              'glass-input w-full rounded-lg font-mono',
              'text-[var(--matrix-text-primary)] placeholder:text-[var(--matrix-text-secondary)]/60',
              'outline-none transition-all duration-200',
              'focus:border-[var(--matrix-accent)] focus:ring-2 focus:ring-[var(--matrix-accent)]/30',
              'disabled:opacity-50 disabled:cursor-not-allowed',
              hasError ? 'border-red-500 focus:border-red-500 focus:ring-red-500/30' : '',
              sizeClasses[inputSize],
              icon ? iconPaddingClasses[inputSize] : '',
              rightElement ? rightElementPaddingClasses[inputSize] : '',
            ].join(' ')}
            {...rest}
          />

          {/* Right Element */}
          {rightElement && (
            <div className="absolute right-2 top-1/2 -translate-y-1/2 flex items-center">
              {rightElement}
            </div>
          )}
        </div>

        {/* Error message */}
        {error && (
          <p id={errorId} role="alert" className="text-xs text-red-500 font-mono">
            {error}
          </p>
        )}
      </div>
    );
  },
);

Input.displayName = 'Input';
