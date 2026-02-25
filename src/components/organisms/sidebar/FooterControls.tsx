// src/components/organisms/sidebar/FooterControls.tsx
/**
 * Shared FooterControls — theme toggle, language selector, and version display.
 * Extracted from Sidebar for reuse across the Jaskier app family.
 */
import { ChevronDown, Globe, Moon, Sun } from 'lucide-react';
import { AnimatePresence, motion } from 'motion/react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useTheme } from '@/contexts/ThemeContext';
import { cn } from '@/shared/utils/cn';

interface FooterControlsProps {
  collapsed: boolean;
  version: string;
  tagline?: string;
}

const LANGUAGES = [
  { code: 'en', name: 'English', flag: '\u{1F1EC}\u{1F1E7}' },
  { code: 'pl', name: 'Polski', flag: '\u{1F1F5}\u{1F1F1}' },
];

export function FooterControls({ collapsed, version, tagline }: FooterControlsProps) {
  const { i18n } = useTranslation();
  const { resolvedTheme, toggleTheme } = useTheme();
  const isLight = resolvedTheme === 'light';

  const [showLangDropdown, setShowLangDropdown] = useState(false);

  const currentLang = LANGUAGES.find((l) => l.code === i18n.language) || LANGUAGES[1];

  const selectLanguage = (langCode: string) => {
    i18n.changeLanguage(langCode);
    setShowLangDropdown(false);
  };

  // Theme-aware utility classes (matching GeminiHydra Sidebar style)
  const textMuted = isLight ? 'text-slate-700' : 'text-white/80';
  const textDim = isLight ? 'text-slate-600' : 'text-white/50';
  const textHover = isLight ? 'hover:text-slate-900' : 'hover:text-white';
  const iconMuted = isLight ? 'text-slate-600' : 'text-white/50';
  const iconHover = isLight ? 'group-hover:text-emerald-700' : 'group-hover:text-white';
  const hoverBg = isLight ? 'hover:bg-black/5' : 'hover:bg-white/5';
  const glassPanel = isLight ? 'glass-panel-light' : 'glass-panel-dark';

  return (
    <>
      {/* Theme & Language Panel */}
      <div className={cn(glassPanel, 'p-2 space-y-1')}>
        {/* Theme Toggle */}
        <button
          type="button"
          data-testid="btn-theme-toggle"
          onClick={toggleTheme}
          className={cn(
            'flex items-center gap-3 w-full p-2 rounded-lg transition-all group',
            collapsed ? 'justify-center' : 'justify-start',
            hoverBg,
          )}
          title={collapsed ? `Theme: ${resolvedTheme === 'dark' ? 'Dark' : 'Light'}` : undefined}
        >
          <div className="relative">
            {resolvedTheme === 'dark' ? (
              <Moon size={18} className="text-slate-400 group-hover:text-white transition-colors" />
            ) : (
              <Sun size={18} className="text-amber-600 group-hover:text-amber-500 transition-colors" />
            )}
          </div>
          {!collapsed && (
            <span className={cn('text-base font-mono tracking-tight truncate', textMuted, textHover)}>
              {resolvedTheme === 'dark' ? 'TRYB CIEMNY' : 'TRYB JASNY'}
            </span>
          )}
        </button>

        {/* Language Selector */}
        <div className="relative">
          <button
            type="button"
            onClick={() => setShowLangDropdown(!showLangDropdown)}
            className={cn(
              'flex items-center gap-3 w-full p-2 rounded-lg transition-all group',
              collapsed ? 'justify-center' : 'justify-between',
              hoverBg,
            )}
            title={collapsed ? `Language: ${currentLang?.name}` : undefined}
          >
            <div className="flex items-center gap-3">
              <div className="relative">
                <Globe size={18} className={cn(iconMuted, iconHover, 'transition-colors')} />
              </div>
              {!collapsed && (
                <span className={cn('text-base font-mono truncate', textMuted, textHover)}>
                  <span className="mr-1.5">{currentLang?.flag}</span>
                  <span className={cn('font-bold', isLight ? 'text-emerald-700' : 'text-white')}>
                    {currentLang?.code.toUpperCase()}
                  </span>
                </span>
              )}
            </div>
            {!collapsed && (
              <ChevronDown
                size={14}
                className={cn(textDim, 'transition-transform duration-200', showLangDropdown ? 'rotate-180' : '')}
              />
            )}
          </button>

          {/* Language Dropdown */}
          <AnimatePresence>
            {showLangDropdown && (
              <motion.div
                initial={{ opacity: 0, y: 4 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: 4 }}
                transition={{ duration: 0.15 }}
                className={cn(
                  'absolute bottom-full left-0 right-0 mb-1 rounded-xl backdrop-blur-xl border overflow-hidden z-50',
                  isLight
                    ? 'bg-white/95 border-emerald-600/20 shadow-[0_8px_32px_rgba(0,0,0,0.15)]'
                    : 'bg-black/90 border-white/20 shadow-[0_8px_32px_rgba(0,0,0,0.6)]',
                )}
              >
                {LANGUAGES.map((lang) => (
                  <button
                    type="button"
                    key={lang.code}
                    onClick={() => selectLanguage(lang.code)}
                    className={cn(
                      'w-full flex items-center gap-3 px-3 py-2.5 text-sm transition-all',
                      i18n.language === lang.code
                        ? isLight
                          ? 'bg-emerald-500/15 text-emerald-800'
                          : 'bg-white/15 text-white'
                        : cn(textMuted, hoverBg, textHover),
                    )}
                  >
                    <span className="text-base">{lang.flag}</span>
                    <span className="font-mono">{lang.name}</span>
                    {i18n.language === lang.code && (
                      <div
                        className={cn(
                          'ml-auto w-1.5 h-1.5 rounded-full',
                          isLight
                            ? 'bg-emerald-600 shadow-[0_0_6px_rgba(5,150,105,0.5)]'
                            : 'bg-white shadow-[0_0_6px_rgba(255,255,255,0.5)]',
                        )}
                      />
                    )}
                  </button>
                ))}
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      </div>

      {/* Version */}
      {!collapsed && (
        <div className={cn('text-center text-xs py-2', isLight ? 'text-slate-600' : 'text-white/50')}>
          <span className={isLight ? 'text-emerald-700' : 'text-white'}>{version.split(' ')[0]}</span>{' '}
          {version.includes(' ') ? version.slice(version.indexOf(' ') + 1) : ''}
          {tagline && <> | {tagline}</>}
        </div>
      )}
    </>
  );
}
