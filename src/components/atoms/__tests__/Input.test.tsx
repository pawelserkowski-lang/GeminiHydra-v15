import { render, screen } from '@testing-library/react';
import { Search } from 'lucide-react';
import { describe, expect, it } from 'vitest';
import { Input } from '@/components/atoms/Input';

describe('Input', () => {
  it('renders an input element', () => {
    render(<Input placeholder="Type here" />);
    expect(screen.getByPlaceholderText('Type here')).toBeInTheDocument();
  });

  it('matches snapshot', () => {
    const { container } = render(
      <Input label="Search" icon={<Search />} error="Invalid input" inputSize="md" placeholder="Search..." />,
    );
    expect(container).toMatchSnapshot();
  });
});
