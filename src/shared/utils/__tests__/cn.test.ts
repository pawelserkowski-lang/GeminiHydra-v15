import { describe, it, expect } from 'vitest';
import { cn } from '@/shared/utils/cn';

describe('cn utility', () => {
  // ========================================
  // Basic merging
  // ========================================

  it('should merge multiple class strings', () => {
    expect(cn('foo', 'bar')).toBe('foo bar');
  });

  it('should return empty string for no arguments', () => {
    expect(cn()).toBe('');
  });

  it('should handle a single class string', () => {
    expect(cn('foo')).toBe('foo');
  });

  // ========================================
  // Conditional classes (clsx behavior)
  // ========================================

  it('should filter out falsy values', () => {
    expect(cn('foo', false, 'bar', null, undefined, 0, '')).toBe('foo bar');
  });

  it('should handle conditional object syntax', () => {
    expect(cn({ foo: true, bar: false, baz: true })).toBe('foo baz');
  });

  it('should handle array syntax', () => {
    expect(cn(['foo', 'bar'], 'baz')).toBe('foo bar baz');
  });

  it('should handle mixed inputs', () => {
    expect(cn('base', { active: true, disabled: false }, ['extra'])).toBe(
      'base active extra',
    );
  });

  // ========================================
  // Tailwind conflict resolution
  // ========================================

  it('should resolve padding conflicts (last wins)', () => {
    expect(cn('p-4', 'p-2')).toBe('p-2');
  });

  it('should resolve margin conflicts', () => {
    expect(cn('mt-4', 'mt-8')).toBe('mt-8');
  });

  it('should resolve text color conflicts', () => {
    expect(cn('text-red-500', 'text-blue-500')).toBe('text-blue-500');
  });

  it('should resolve background color conflicts', () => {
    expect(cn('bg-red-500', 'bg-blue-500')).toBe('bg-blue-500');
  });

  it('should resolve font size conflicts', () => {
    expect(cn('text-sm', 'text-lg')).toBe('text-lg');
  });

  it('should resolve display conflicts', () => {
    expect(cn('block', 'flex')).toBe('flex');
  });

  it('should resolve border-radius conflicts', () => {
    expect(cn('rounded-sm', 'rounded-lg')).toBe('rounded-lg');
  });

  // ========================================
  // Non-conflicting classes preserved
  // ========================================

  it('should preserve non-conflicting Tailwind classes', () => {
    const result = cn('p-4', 'mt-2', 'text-red-500', 'bg-blue-500');
    expect(result).toContain('p-4');
    expect(result).toContain('mt-2');
    expect(result).toContain('text-red-500');
    expect(result).toContain('bg-blue-500');
  });

  // ========================================
  // Real-world usage patterns
  // ========================================

  it('should handle component variant pattern', () => {
    const baseClasses = 'rounded-md px-4 py-2 font-medium';
    const variantClasses = 'bg-blue-500 text-white';
    const overrides = 'bg-red-500';

    const result = cn(baseClasses, variantClasses, overrides);
    expect(result).toContain('bg-red-500');
    expect(result).not.toContain('bg-blue-500');
    expect(result).toContain('text-white');
  });

  it('should handle conditional disabled state', () => {
    const isDisabled = true;
    const result = cn(
      'px-4 py-2',
      isDisabled && 'opacity-50 cursor-not-allowed',
    );
    expect(result).toContain('opacity-50');
    expect(result).toContain('cursor-not-allowed');
  });
});
