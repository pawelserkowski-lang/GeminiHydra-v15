// src/components/organisms/ErrorBoundary.tsx
/**
 * GeminiHydra v15 - Error Boundary
 * ==================================
 * React Error Boundary with retry functionality.
 * Styled with the Card atom for consistent glass-panel appearance.
 */

import { AlertTriangle, RotateCcw } from 'lucide-react';
import { Component, type ErrorInfo, type ReactNode } from 'react';
import { Button, Card } from '@/components/atoms';

// ============================================================================
// TYPES
// ============================================================================

interface ErrorBoundaryProps {
  children: ReactNode;
  fallback?: ReactNode;
}

interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
}

// ============================================================================
// COMPONENT
// ============================================================================

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo): void {
    console.error('[ErrorBoundary] Caught error:', error, errorInfo);
  }

  handleRetry = (): void => {
    this.setState({ hasError: false, error: null });
  };

  render(): ReactNode {
    if (this.state.hasError) {
      if (this.props.fallback) {
        return this.props.fallback;
      }

      return (
        <div className="min-h-screen flex items-center justify-center bg-[var(--matrix-bg-primary)] p-6">
          <Card variant="elevated" padding="lg" className="max-w-md w-full">
            <div className="flex flex-col items-center gap-4 text-center">
              <div className="w-14 h-14 rounded-2xl flex items-center justify-center bg-red-500/10 border border-red-500/20">
                <AlertTriangle size={28} className="text-red-400" />
              </div>

              <div>
                <h2 className="text-lg font-bold font-mono text-[var(--matrix-text-primary)]">Something went wrong</h2>
                <p className="text-sm text-[var(--matrix-text-dim)] mt-1">
                  An unexpected error occurred. You can try again or refresh the page.
                </p>
              </div>

              {this.state.error && (
                <pre className="w-full text-xs text-red-400/80 bg-red-500/5 border border-red-500/10 rounded-lg p-3 overflow-auto max-h-32 text-left font-mono">
                  {this.state.error.message}
                </pre>
              )}

              <Button variant="primary" size="md" leftIcon={<RotateCcw size={16} />} onClick={this.handleRetry}>
                Try Again
              </Button>
            </div>
          </Card>
        </div>
      );
    }

    return this.props.children;
  }
}
