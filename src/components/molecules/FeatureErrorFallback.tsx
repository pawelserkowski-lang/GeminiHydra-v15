/** Jaskier Design System */
import { AlertTriangle, RefreshCw } from 'lucide-react';
import { Button } from '@/components/atoms/Button';

interface Props {
  feature: string;
  onRetry?: () => void;
}

export function FeatureErrorFallback({ feature, onRetry }: Props) {
  return (
    <div className="flex flex-col items-center justify-center gap-4 py-16 text-center">
      <AlertTriangle className="h-10 w-10 text-red-400" />
      <h3 className="text-lg font-medium text-[var(--matrix-text-primary)]">
        {feature} encountered an error
      </h3>
      <p className="text-sm text-[var(--matrix-text-secondary)] max-w-md">
        Something went wrong loading this section. Try refreshing.
      </p>
      {onRetry && (
        <Button variant="secondary" onClick={onRetry}>
          <RefreshCw className="h-4 w-4 mr-2" /> Retry
        </Button>
      )}
    </div>
  );
}
