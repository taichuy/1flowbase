import {
  act,
  cleanup,
  fireEvent,
  render,
  screen
} from '@testing-library/react';
import { useContext, useEffect, useState } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { FrontstageRuntimeActivityContext } from '../../../../lib/page-canvas/runtime-activity';
import { createFrontstagePageContentFixture } from '../../../../_tests/frontstage-page-content-fixtures';
import type { FrontStagePageProps } from '../../page-props';
import { RetainedFrontstagePages } from '../RetainedFrontstagePages';

import { RETENTION_SAMPLE_MS } from '../retention-memory';
import { useFrontstageRetentionProtection } from '../retention-protection';
import { RetentionPolicy } from '../retention-policy';

const disposed = vi.fn();
const callbacks = new Map<string, FrontStagePageProps['onNavigatePage']>();
function StatefulPage(props: FrontStagePageProps) {
  const [count, setCount] = useState(0);
  useFrontstageRetentionProtection(Boolean(props.isPageTreeMutating));
  const id = `${props.pageId}/${props.tabId}`;
  useEffect(
    () => () => {
      disposed(id);
    },
    [id]
  );
  const activity = useContext(FrontstageRuntimeActivityContext);
  const failed = Boolean(
    props.hasPageTreeLoadError ||
    props.hasPageContentLoadError ||
    props.hasBlockRootsLoadError ||
    props.hasBlockRuntimeLoadError ||
    props.isPageContentPermissionDenied ||
    props.isBlockRuntimePermissionDenied
  );
  if (failed) return <div data-testid="load-error">Load failed</div>;
  if (
    props.pageContent?.page.id !== props.pageId ||
    props.pageContent?.tab.id !== props.tabId
  )
    return <div data-testid="source-pending">Loading</div>;
  callbacks.set(id, props.onNavigatePage);
  return (
    <>
      <input aria-label={`Input ${id}`} defaultValue="" />
      <button
        data-testid={id}
        data-active={props.runtimeActive}
        data-context-active={activity}
        onClick={() => setCount(count + 1)}
      >
        {count}
      </button>
    </>
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
  scope = 'actor1/workspace1',
  budget?: number
) {
  return (
    <RetainedFrontstagePages
      key={scope}
      activeKey={`${page}/${tab}`}
      retentionBudgetBytes={budget}
    >
      {session(page, tab, extra)}
    </RetainedFrontstagePages>
  );
}
afterEach(() => {
  cleanup();
  vi.useRealTimers();
  disposed.mockClear();
  callbacks.clear();
  vi.restoreAllMocks();
});

describe('retained mounted page sessions', () => {
  it('counts successful route activations, not rerenders, failures or retrying the current route', () => {
    const activation = vi.spyOn(RetentionPolicy.prototype, 'activate');
    const ui = render(view('a', 't1', { hasPageContentLoadError: true }));
    expect(activation).not.toHaveBeenCalled();
    ui.rerender(view('a'));
    ui.rerender(view('a'));
    ui.rerender(view('a', 't1', { hasPageContentLoadError: true }));
    ui.rerender(view('a'));
    expect(activation.mock.calls.map(([key]) => key)).toEqual(['a/t1']);
    ui.rerender(view('b'));
    ui.rerender(view('a'));
    expect(activation.mock.calls.map(([key]) => key)).toEqual([
      'a/t1',
      'b/t1',
      'a/t1'
    ]);
    expect(screen.getByTestId('a/t1')).toBeVisible();
  });
  it('permission revocation overrides draft protection', () => {
    const ui = render(view('a', 't1', { isPageTreeMutating: true }));
    ui.rerender(view('b'));
    ui.rerender(view('a', 't1', { isPageContentPermissionDenied: true }));
    expect(screen.queryByTestId('a/t1')).toBeNull();
    expect(disposed).toHaveBeenCalledWith('a/t1');
  });
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
    navigateA?.('other');
    expect(navigate).not.toHaveBeenCalled();
    ui.rerender(
      view('a', 't1', {
        pageContent: createFrontstagePageContentFixture({ page: { id: 'b' } })
      })
    );
    expect(screen.getByTestId('a/t1').textContent).toBe('0');
    expect(screen.getByTestId('a/t1').dataset.active).toBe('false');
    expect(screen.getByTestId('a/t1')).not.toBeVisible();
    expect(screen.getByTestId('source-pending')).toBeVisible();
    navigateA?.('other');
    expect(navigate).not.toHaveBeenCalled();
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
  it('reclaims inactive sessions over budget without disposing the active session', () => {
    vi.useFakeTimers();
    const ui = render(view('a', 't1', {}, undefined, 0));
    ui.rerender(view('b', 't1', {}, undefined, 0));
    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(screen.queryByTestId('a/t1')).toBeNull();
    expect(screen.getByTestId('b/t1')).toBeTruthy();
    expect(disposed).toHaveBeenCalledWith('a/t1');
  });
  it.each([6, 29, 31, 120])(
    'preserves DOM, state and input after %i idle minutes',
    (minutes) => {
      vi.useFakeTimers();
      const ui = render(view('a'));
      const original = screen.getByTestId('a/t1');
      const input = screen.getByLabelText('Input a/t1');
      fireEvent.click(original);
      fireEvent.change(input, { target: { value: 'unsaved native input' } });
      ui.rerender(view('b'));
      act(() => vi.advanceTimersByTime(minutes * 60 * 1000));
      ui.rerender(view('a'));
      expect(screen.getByTestId('a/t1')).toBe(original);
      expect(original.textContent).toBe('1');
      expect(screen.getByLabelText('Input a/t1')).toBe(input);
      expect(input).toHaveValue('unsaved native input');
      expect(original.dataset.contextActive).toBe('true');
      expect(disposed).not.toHaveBeenCalled();
    }
  );
  it.each([
    { hasPageTreeLoadError: true },
    { hasPageContentLoadError: true },
    { hasBlockRootsLoadError: true },
    { hasBlockRuntimeLoadError: true }
  ])(
    'freezes the cached snapshot during a transient failure and recovers %j',
    (error) => {
      const navigate = vi.fn();
      const ui = render(view('a', 't1', { onNavigatePage: navigate }));
      const original = screen.getByTestId('a/t1');
      const input = screen.getByLabelText('Input a/t1');
      const navigateA = callbacks.get('a/t1');
      fireEvent.click(original);
      fireEvent.change(input, { target: { value: 'draft' } });
      ui.rerender(view('a', 't1', { ...error, onNavigatePage: navigate }));
      expect(screen.getByTestId('load-error')).toBeVisible();
      expect(screen.getByTestId('a/t1')).toBe(original);
      expect(original).not.toBeVisible();
      expect(original.dataset.active).toBe('false');
      expect(original.dataset.contextActive).toBe('false');
      expect(disposed).not.toHaveBeenCalled();
      navigateA?.('other');
      expect(navigate).not.toHaveBeenCalled();
      ui.rerender(view('a', 't1', { onNavigatePage: navigate }));
      expect(screen.queryByTestId('load-error')).toBeNull();
      expect(screen.getByTestId('a/t1')).toBe(original);
      expect(original).toBeVisible();
      expect(original.textContent).toBe('1');
      expect(screen.getByLabelText('Input a/t1')).toBe(input);
      expect(input).toHaveValue('draft');
      expect(callbacks.get('a/t1')).toBe(navigateA);
      navigateA?.('other');
      expect(navigate).toHaveBeenCalledWith('other');
    }
  );
  it.each([
    { isPageContentPermissionDenied: true },
    { isBlockRuntimePermissionDenied: true }
  ])('evicts every cached tab of a denied page %j', (error) => {
    const navigate = vi.fn();
    const ui = render(view('a', 't1', { onNavigatePage: navigate }));
    const original = screen.getByTestId('a/t1');
    const navigateA = callbacks.get('a/t1');
    ui.rerender(view('a', 't2'));
    ui.rerender(view('a', 't1', error));
    expect(disposed).toHaveBeenCalledWith('a/t1');
    expect(disposed).toHaveBeenCalledWith('a/t2');
    expect(screen.queryByTestId('a/t1')).toBeNull();
    expect(screen.getByTestId('load-error')).toBeVisible();
    ui.rerender(view('a', 't1', { onNavigatePage: navigate }));
    expect(screen.getByTestId('a/t1')).not.toBe(original);
    expect(screen.getByTestId('a/t1').textContent).toBe('0');
    navigateA?.('other');
    expect(navigate).not.toHaveBeenCalled();
  });
  it('keeps guard functions stable while forwarding the latest callback', () => {
    const initial = vi.fn();
    const latest = vi.fn();
    const ui = render(view('a', 't1', { onNavigatePage: initial }));
    const guard = callbacks.get('a/t1');
    ui.rerender(view('a', 't1', { onNavigatePage: latest }));
    expect(callbacks.get('a/t1')).toBe(guard);
    guard?.('destination');
    expect(initial).not.toHaveBeenCalled();
    expect(latest).toHaveBeenCalledWith('destination');
    ui.unmount();
    guard?.('after-unmount');
    expect(latest).toHaveBeenCalledTimes(1);
  });
  it('blocks captured evicted callbacks after the same session key is recreated', () => {
    vi.useFakeTimers();
    const navigate = vi.fn();
    const ui = render(
      view('a', 't1', { onNavigatePage: navigate }, undefined, 0)
    );
    const expired = callbacks.get('a/t1');
    ui.rerender(view('b', 't1', {}, undefined, 0));
    act(() => vi.advanceTimersByTime(1));
    ui.rerender(view('a', 't1', { onNavigatePage: navigate }));
    expired?.('other');
    expect(navigate).not.toHaveBeenCalled();
    callbacks.get('a/t1')?.('current');
    expect(navigate).toHaveBeenCalledWith('current');
  });
  it('does not refresh the active instance over multiple idle intervals', () => {
    vi.useFakeTimers();
    const ui = render(view('a'));
    const original = screen.getByTestId('a/t1');
    fireEvent.click(original);
    act(() => vi.advanceTimersByTime(3 * 60 * 60 * 1000));
    expect(screen.getByTestId('a/t1')).toBe(original);
    expect(original.textContent).toBe('1');
    expect(disposed).not.toHaveBeenCalled();
    ui.unmount();
  });
  it('protects a known pending write under pressure and releases it after completion', () => {
    vi.useFakeTimers();
    const ui = render(
      view('a', 't1', { isPageTreeMutating: true }, undefined, 0)
    );
    const original = screen.getByTestId('a/t1');
    ui.rerender(view('b', 't1', {}, undefined, 0));
    act(() => vi.advanceTimersByTime(1));
    expect(screen.getByTestId('a/t1')).toBe(original);
    ui.rerender(view('a', 't1', { isPageTreeMutating: false }, undefined, 0));
    ui.rerender(view('b', 't1', {}, undefined, 0));
    act(() => vi.advanceTimersByTime(RETENTION_SAMPLE_MS));
    expect(screen.queryByTestId('a/t1')).toBeNull();
    expect(screen.getByTestId('b/t1')).toBeTruthy();
  });
  it('retains visited sessions without a page-count cap', () => {
    const ui = render(view('page-0'));
    const original = screen.getByTestId('page-0/t1');
    for (let page = 1; page <= 20; page += 1) ui.rerender(view(`page-${page}`));
    expect(screen.getByTestId('page-0/t1')).toBe(original);
    expect(disposed).not.toHaveBeenCalled();
    ui.rerender(view('page-0'));
    expect(screen.getByTestId('page-0/t1')).toBe(original);
  });
  it('does not interpret a failed empty tree response as deletion', () => {
    const ui = render(view('a'));
    const original = screen.getByTestId('a/t1');
    ui.rerender(
      <RetainedFrontstagePages
        key="actor1/workspace1"
        activeKey="a/t1"
        pageTree={[]}
      >
        {session('a', 't1', { hasPageTreeLoadError: true })}
      </RetainedFrontstagePages>
    );
    expect(screen.getByTestId('a/t1')).toBe(original);
    expect(disposed).not.toHaveBeenCalled();
    ui.rerender(view('a'));
    expect(screen.getByTestId('a/t1')).toBe(original);
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
