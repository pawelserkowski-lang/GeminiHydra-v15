// src/components/organisms/sidebar/LogoButton.tsx
/** Jaskier Design System */
/**
 * Shared LogoButton — theme-aware logo with home navigation.
 * Collapsed: w-12 h-12 icon. Expanded: h-28 full logo.
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
      data-testid="sidebar-logo"
      onClick={onClick}
      className={cn(
        'flex items-center justify-center flex-shrink-0 cursor-pointer',
        'hover:opacity-80 transition-opacity',
        collapsed ? 'w-full' : '',
        className,
      )}
      title="Home"
      aria-label="Navigate to home"
    >
      <img
        src={logoSrc}
        alt="Logo"
        width={512}
        height={425}
        className={cn('object-contain transition-all duration-300', collapsed ? 'w-12 h-12' : 'h-28')}
      />
    </button>
  );
}
