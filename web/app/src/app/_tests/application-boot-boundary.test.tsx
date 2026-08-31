import { render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, test, vi } from 'vitest';

import {
  ApplicationBootBoundary,
  ApplicationBootStage
} from '../ApplicationBootBoundary';

function BrokenRuntime(): ReactNode {
  throw new Error("does not provide an export named 'ForwardRef'");
}

describe('ApplicationBootBoundary', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  test('DRS-003 keeps a visible stage while the application graph loads', () => {
    render(<ApplicationBootStage />);

    expect(screen.getByRole('status', { name: '应用正在启动' })).toBeVisible();
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
