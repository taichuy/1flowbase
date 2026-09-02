import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { PageTreeIconPicker } from '../PageTreeIconPicker';

describe('PageTreeIconPicker', () => {
  it('MDP-003 renders only the first virtual viewport', () => {
    render(<PageTreeIconPicker onSelect={vi.fn()} />);

    expect(screen.getAllByRole('button').length).toBeLessThanOrEqual(108);
    expect(document.querySelectorAll('svg use').length).toBeLessThanOrEqual(
      108
    );
  });

  it('MDP-003 searches the compact catalog before rendering previews', () => {
    render(<PageTreeIconPicker onSelect={vi.fn()} />);

    fireEvent.change(screen.getByRole('searchbox'), {
      target: { value: 'SmileTwoTone' }
    });

    expect(screen.getByRole('button', { name: 'SmileTwoTone' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'SmileOutlined' })).toBeNull();
  });
});
