import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { Card } from '@/components/atoms/Card';

describe('Card', () => {
  it('renders children', () => {
    render(<Card>Card Content</Card>);
    expect(screen.getByText('Card Content')).toBeInTheDocument();
  });

  it('matches snapshot', () => {
    const { container } = render(
      <Card variant="glass" padding="md" header={<h3>Title</h3>} footer={<button type="button">Save</button>}>
        Body
      </Card>,
    );
    expect(container).toMatchSnapshot();
  });
});
