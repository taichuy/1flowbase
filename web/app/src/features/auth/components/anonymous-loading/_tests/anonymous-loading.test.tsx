import { render, screen } from '@testing-library/react';
import { expect, test } from 'vitest';
import { AnonymousLoadingState } from '../AnonymousLoadingState';

test('loading preserves status, compact skeleton rows, spinner and caller classes', () => {
  const view = render(
    <AnonymousLoadingState compact fullscreen className="overlay" />
  );
  expect(screen.getByRole('status', { name: 'thinking' })).toHaveClass(
    'loading-state--compact',
    'loading-state--fullscreen',
    'overlay'
  );
  expect(view.container.querySelectorAll('li')).toHaveLength(3);
  expect(view.container.querySelector('svg')).toHaveAttribute(
    'viewBox',
    '0 0 1024 1024'
  );
  expect(view.container.querySelector('h3')).toHaveStyle({ width: '38%' });
  view.rerender(<AnonymousLoadingState />);
  expect(view.container.querySelectorAll('li')).toHaveLength(5);
  expect(view.container.querySelector('h3')).toHaveStyle({ width: '28%' });
});
test('SSR switches omit only requested visuals, not the accessible status', () => {
  const view = render(
    <AnonymousLoadingState showSpinner={false} showSkeleton={false} />
  );
  expect(screen.getByRole('status', { name: 'thinking' })).toBeInTheDocument();
  expect(view.container.querySelector('svg')).toBeNull();
  expect(view.container.querySelector('li')).toBeNull();
});
