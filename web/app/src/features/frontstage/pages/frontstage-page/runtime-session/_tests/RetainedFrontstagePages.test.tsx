import { act, fireEvent, render, screen } from '@testing-library/react';
import { useEffect, useState } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createFrontstagePageContentFixture } from '../../../../_tests/frontstage-page-content-fixtures';
import type { FrontStagePageProps } from '../../page-props';
import {
  FRONTSTAGE_SESSION_IDLE_MS,
  RetainedFrontstagePages
} from '../RetainedFrontstagePages';

const disposed = vi.fn();
const callbacks = new Map<string, (() => void) | undefined>();
function StatefulPage(props: FrontStagePageProps) {
  const [count, setCount] = useState(0);
  const id = `${props.pageId}/${props.tabId}`;
  useEffect(
    () => () => {
      disposed(id);
    },
    [id]
  );
  callbacks.set(id, () => props.onNavigatePage?.('other'));
  return (
    <button
      data-testid={id}
      data-active={props.runtimeActive}
      onClick={() => setCount(count + 1)}
    >
      {count}
    </button>
  );
}
function session(
  page: string,
  tab = 't1',
  extra: Partial<FrontStagePageProps> = {}
) {
  return (
    <StatefulPage
      workspaceId="w1"
      pageId={page}
      tabId={tab}
      pageContent={createFrontstagePageContentFixture({
        page: { id: page },
        tab: { id: tab }
      })}
      {...extra}
    />
  );
}
function view(
  page: string,
  tab = 't1',
  extra: Partial<FrontStagePageProps> = {},
  scope = 'actor1/workspace1'
) {
  return (
    <RetainedFrontstagePages key={scope} activeKey={`${page}/${tab}`}>
      {session(page, tab, extra)}
    </RetainedFrontstagePages>
  );
}
afterEach(() => {
  vi.useRealTimers();
  disposed.mockClear();
  callbacks.clear();
});

describe('retained mounted page sessions', () => {
  it('restores the same mounted instance and local state across A/B/A and tabs', () => {
    const ui = render(view('a'));
    fireEvent.click(screen.getByTestId('a/t1'));
    const original = screen.getByTestId('a/t1');
    ui.rerender(view('b'));
    expect(original.closest('[hidden]')).not.toBeNull();
    expect(original.dataset.active).toBe('false');
    ui.rerender(view('a', 't2'));
    fireEvent.click(screen.getByTestId('a/t2'));
    ui.rerender(view('a'));
    expect(screen.getByTestId('a/t1')).toBe(original);
    expect(original.textContent).toBe('1');
    expect(disposed).not.toHaveBeenCalled();
    ui.unmount();
    expect(disposed).toHaveBeenCalledTimes(3);
  });
  it('blocks captured inactive navigation and keeps mismatched source props out', () => {
    const navigate = vi.fn();
    const ui = render(view('a', 't1', { onNavigatePage: navigate }));
    const navigateA = callbacks.get('a/t1');
    ui.rerender(view('b'));
    navigateA?.();
    expect(navigate).not.toHaveBeenCalled();
    ui.rerender(
      view('a', 't1', {
        pageContent: createFrontstagePageContentFixture({ page: { id: 'b' } })
      })
    );
    expect(screen.getByTestId('a/t1').textContent).toBe('0');
    expect(screen.getByTestId('a/t1').dataset.active).toBe('false');
  });
  it.each(['actor2/workspace1', 'actor1/workspace2'])(
    'disposes on scope change %s',
    (scope) => {
      const ui = render(view('a'));
      fireEvent.click(screen.getByTestId('a/t1'));
      ui.rerender(view('a', 't1', {}, scope));
      expect(disposed).toHaveBeenCalledWith('a/t1');
      expect(screen.getByTestId('a/t1').textContent).toBe('0');
    }
  );
  it('expires idle sessions without disposing the active session', () => {
    vi.useFakeTimers();
    const ui = render(view('a'));
    ui.rerender(view('b'));
    act(() => {
      vi.advanceTimersByTime(FRONTSTAGE_SESSION_IDLE_MS);
    });
    expect(screen.queryByTestId('a/t1')).toBeNull();
    expect(screen.getByTestId('b/t1')).toBeTruthy();
    expect(disposed).toHaveBeenCalledWith('a/t1');
  });
  it.each([
    { hasPageContentLoadError: true },
    { isPageContentPermissionDenied: true }
  ])('disposes failed or denied sessions %j', (error) => {
    const ui = render(view('a'));
    fireEvent.click(screen.getByTestId('a/t1'));
    ui.rerender(view('a', 't1', error));
    expect(disposed).toHaveBeenCalledWith('a/t1');
    expect(screen.getByTestId('a/t1').textContent).toBe('0');
  });
  it('disposes removed pages when the tree refreshes', () => {
    const ui = render(view('a'));
    ui.rerender(view('b'));
    ui.rerender(
      <RetainedFrontstagePages
        key="actor1/workspace1"
        activeKey="b/t1"
        pageTree={[{ id: 'b', kind: 'page', title: 'B' }]}
      >
        {session('b')}
      </RetainedFrontstagePages>
    );
    expect(screen.queryByTestId('a/t1')).toBeNull();
    expect(disposed).toHaveBeenCalledWith('a/t1');
  });
});
