// src/components/organisms/sidebar/LogoButton.tsx
/**
 * Shared LogoButton — theme-aware logo with home navigation.
 * Collapsed: w-16 h-16 icon. Expanded: h-36 full logo.
 */
import { useTheme } from '@/contexts/ThemeContext';
import { cn } from '@/shared/utils/cn';

interface LogoButtonProps {
  collapsed: boolean;
  onClick: () => void;
  className?: string;
}

export function LogoButton({ collapsed, onClick, className }: LogoButtonProps) {
  const { resolvedTheme } = useTheme();
  const logoSrc = resolvedTheme === 'dark' ? '/logodark.webp' : '/logolight.webp';

  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        'flex items-center justify-center py-4 px-1 flex-shrink-0 cursor-pointer',
        collapsed ? 'w-full' : 'flex-1',
        className,
      )}
      title="Home"
    >
      <img
        src={logoSrc}
        alt="GeminiHydra Logo"
        className={cn(
          'object-contain transition-all duration-300',
          collapsed ? 'w-16 h-16' : 'h-36',
        )}
      />
    </button>
  );
}
