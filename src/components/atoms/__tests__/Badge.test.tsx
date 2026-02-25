import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { Badge } from '@/components/atoms/Badge';

describe('Badge', () => {
  // -------------------------------------------------------------------------
  // Basic rendering
  // -------------------------------------------------------------------------

  it('renders children text', () => {
    render(<Badge>Active</Badge>);
    expect(screen.getByText('Active')).toBeInTheDocument();
  });

  it('renders as a span element', () => {
    const { container } = render(<Badge>Tag</Badge>);
    const span = container.querySelector('span');
    expect(span).toBeInTheDocument();
    expect(span?.textContent).toContain('Tag');
  });

  // -------------------------------------------------------------------------
  // Variants
  // -------------------------------------------------------------------------

  it('applies default variant classes', () => {
    const { container } = render(<Badge>Default</Badge>);
    const span = container.firstChild as HTMLElement;
    expect(span.className).toContain('text-matrix-text-dim');
  });

  it('applies accent variant classes', () => {
    const { container } = render(<Badge variant="accent">Accent</Badge>);
    const span = container.firstChild as HTMLElement;
    expect(span.className).toContain('text-matrix-accent');
  });

  it('applies success variant classes', () => {
    const { container } = render(<Badge variant="success">Success</Badge>);
    const span = container.firstChild as HTMLElement;
    expect(span.className).toContain('text-[var(--matrix-success)]');
  });

  it('applies warning variant classes', () => {
    const { container } = render(<Badge variant="warning">Warning</Badge>);
    const span = container.firstChild as HTMLElement;
    expect(span.className).toContain('text-[var(--matrix-warning)]');
  });

  it('applies error variant classes', () => {
    const { container } = render(<Badge variant="error">Error</Badge>);
    const span = container.firstChild as HTMLElement;
    expect(span.className).toContain('text-[var(--matrix-error)]');
  });

  // -------------------------------------------------------------------------
  // Sizes
  // -------------------------------------------------------------------------

  it('applies sm size classes by default', () => {
    const { container } = render(<Badge>Small</Badge>);
    const span = container.firstChild as HTMLElement;
    expect(span.className).toContain('text-xs');
    expect(span.className).toContain('px-2');
  });

  it('applies md size classes', () => {
    const { container } = render(<Badge size="md">Medium</Badge>);
    const span = container.firstChild as HTMLElement;
    expect(span.className).toContain('text-sm');
  });

  // -------------------------------------------------------------------------
  // Dot indicator
  // -------------------------------------------------------------------------

  it('renders a dot indicator when dot prop is true', () => {
    const { container } = render(<Badge dot>With Dot</Badge>);
    const dotEl = container.querySelector('[aria-hidden="true"]');
    expect(dotEl).toBeInTheDocument();
    expect(dotEl?.className).toContain('rounded-full');
  });

  it('does not render a dot indicator by default', () => {
    const { container } = render(<Badge>No Dot</Badge>);
    const dotEl = container.querySelector('[aria-hidden="true"]');
    expect(dotEl).not.toBeInTheDocument();
  });

  // -------------------------------------------------------------------------
  // Icon
  // -------------------------------------------------------------------------

  it('renders icon when provided and dot is false', () => {
    render(<Badge icon={<span data-testid="badge-icon">*</span>}>With Icon</Badge>);
    expect(screen.getByTestId('badge-icon')).toBeInTheDocument();
  });

  it('does not render icon when dot is true (dot takes precedence)', () => {
    render(<Badge dot icon={<span data-testid="badge-icon">*</span>}>Dot + Icon</Badge>);
    expect(screen.queryByTestId('badge-icon')).not.toBeInTheDocument();
  });

  // -------------------------------------------------------------------------
  // Custom className
  // -------------------------------------------------------------------------

  it('applies additional className', () => {
    const { container } = render(<Badge className="custom-class">Custom</Badge>);
    const span = container.firstChild as HTMLElement;
    expect(span.className).toContain('custom-class');
  });
});
