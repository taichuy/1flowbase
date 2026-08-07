import { render, screen } from '@testing-library/react';
import { describe, expect, test } from 'vitest';

import { LoadingState } from '../LoadingState';

describe('LoadingState', () => {
  test('uses the shared loading label', () => {
    render(<LoadingState />);

    expect(screen.getByRole('status', { name: 'thinking' })).toBeInTheDocument();
    expect(screen.getByText('thinking')).toBeInTheDocument();
  });

  test('uses the large circular loading indicator', () => {
    render(<LoadingState />);

    expect(document.querySelector('.anticon-loading')).toHaveStyle({
      fontSize: '48px'
    });
  });

  test('supports fullscreen and compact layout variants', () => {
    render(<LoadingState fullscreen compact />);

    expect(screen.getByRole('status', { name: 'thinking' })).toHaveClass(
      'loading-state',
      'loading-state--fullscreen',
      'loading-state--compact'
    );
  });
});
