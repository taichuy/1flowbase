import { act, cleanup, render } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import type { Props } from 'react-infinite-scroll-component';

import {
  NativeBlockSurfaceProvider,
  type NativeBlockSurfaceScope
} from '../../native-block-surface-context';
import NativeInfiniteScroll from '../runtime';

const observers: Array<{
  root: Element | Document | null;
  fire(): void;
  disconnect: ReturnType<typeof vi.fn>;
}> = [];
const hosts: HTMLElement[] = [];
beforeEach(() => {
  observers.length = 0;
  vi.stubGlobal(
    'IntersectionObserver',
    class {
      disconnect = vi.fn();
      observe = vi.fn();
      unobserve = vi.fn();
      constructor(
        callback: IntersectionObserverCallback,
        options: IntersectionObserverInit
      ) {
        observers.push({
          root: options.root ?? null,
          disconnect: this.disconnect,
          fire: () =>
            callback(
              [{ isIntersecting: true } as IntersectionObserverEntry],
              this as unknown as IntersectionObserver
            )
        });
      }
    }
  );
});
afterEach(() => {
  cleanup();
  hosts.splice(0).forEach((host) => host.remove());
  vi.unstubAllGlobals();
});

function mount(props: Partial<Props> = {}) {
  const host = document.createElement('div');
  hosts.push(host);
  document.body.append(host);
  const root = host.attachShadow({ mode: 'open' });
  const container = document.createElement('div');
  root.append(container);
  const scope = {
    targetRoot: root,
    scrollOwner: container
  } as unknown as NativeBlockSurfaceScope;
  const next = vi.fn();
  const view = (overrides: Partial<Props> = {}) => (
    <NativeBlockSurfaceProvider scope={scope}>
      <div id="scrollableDiv">
        <NativeInfiniteScroll
          dataLength={10}
          next={next}
          hasMore
          loader="loading"
          scrollableTarget="scrollableDiv"
          {...props}
          {...overrides}
        >
          rows
        </NativeInfiniteScroll>
      </div>
    </NativeBlockSurfaceProvider>
  );
  const result = render(view(), { container });
  return {
    ...result,
    root,
    container,
    next,
    update: (props: Partial<Props>) => result.rerender(view(props))
  };
}

describe('Block infinite scroll', () => {
  test('resolves identical IDs independently inside each ShadowRoot', () => {
    const first = mount();
    const second = mount();
    expect(observers).toHaveLength(2);
    expect(observers[0].root).toBe(first.root.getElementById('scrollableDiv'));
    expect(observers[1].root).toBe(second.root.getElementById('scrollableDiv'));
    act(() => observers[0].fire());
    expect(first.next).toHaveBeenCalledTimes(1);
    expect(second.next).not.toHaveBeenCalled();
  });
  test('guards duplicate loads, unlocks on appended data, stops and cleans up', () => {
    const view = mount({
      onScroll: vi.fn(),
      pullDownToRefresh: true,
      refreshFunction: vi.fn()
    });
    const observer = observers[0];
    const target = observer.root as HTMLElement;
    const remove = vi.spyOn(target, 'removeEventListener');
    act(() => {
      observer.fire();
      observer.fire();
    });
    expect(view.next).toHaveBeenCalledTimes(1);
    view.update({ dataLength: 20 });
    act(() => observer.fire());
    expect(view.next).toHaveBeenCalledTimes(2);
    view.update({ dataLength: 30, hasMore: false, endMessage: 'finished' });
    expect(observer.disconnect).toHaveBeenCalled();
    expect(view.container.textContent).toContain('finished');
    view.unmount();
    expect(remove.mock.calls.map(([event]) => event)).toEqual(
      expect.arrayContaining(['scroll', 'touchstart', 'touchmove', 'touchend'])
    );
  });
  test('height uses the component scrollbox instead of resolving an irrelevant target', () => {
    const view = mount({ height: 300, scrollableTarget: 'not-used' });
    expect(observers[0].root).toBe(
      view.container.querySelector('.infinite-scroll-component')
    );
  });
  test('defaults to the owning surface scroll container', () => {
    const view = mount({ scrollableTarget: undefined });
    expect(observers[0].root).toBe(view.container);
  });
});
