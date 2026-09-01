import { render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, test, vi } from 'vitest';

import { ApplicationBootBoundary } from '../ApplicationBootBoundary';

function BrokenRuntime(): ReactNode {
  throw new Error("does not provide an export named 'ForwardRef'");
}

describe('ApplicationBootBoundary', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  test('DRS-003 leaves the canonical loading owner outside the error boundary', () => {
    render(
      <ApplicationBootBoundary>
        <div data-testid="canonical-loading-owner" />
      </ApplicationBootBoundary>
    );

    expect(screen.getByTestId('canonical-loading-owner')).toBeVisible();
  });

  test('DRS-003 replaces a failed application graph with a retry surface', () => {
    vi.spyOn(console, 'error').mockImplementation(() => undefined);

    render(
      <ApplicationBootBoundary>
        <BrokenRuntime />
      </ApplicationBootBoundary>
    );

    expect(screen.getByRole('alert')).toHaveTextContent('应用模块加载失败');
    expect(screen.getByRole('button', { name: '重新加载' })).toBeVisible();
  });
});
