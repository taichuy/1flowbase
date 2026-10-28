import { act, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

import { PageCanvas } from '../../components/PageCanvas';
import type { FrontstageBlockInstance } from '../../lib/page-document';
import { createFrontstagePageContentFixture } from '../frontstage-page-content-fixtures';

const runtimeBlock: FrontstageBlockInstance = {
  id: 'wide-block',
  rendererVersion: 'v1',
  sourceId: 'wide-block',
  codeRef: 'frontstage.block.wide-block',
  sourceCodeRef: 'frontstage.block.wide-block',
  catalog: { providerCode: null, installationId: null },
  contribution: {
    pluginId: null,
    pluginVersion: null,
    code: 'official.wide-block'
  },
  props: {},
  ports: { inputs: [], outputs: [] },
  presentation: { heightMode: 'auto', height: null },
  layout: {
    order: 0,
    gridColumns: 24,
    verticalGridVersion: 2,
    lg: { x: 0, y: 0, w: 24, h: 4 }
  },
  order: 0,
  runtime: {
    kind: 'native_react',
    entry: 'blocks/wide-block.tsx',
    hint: 'native_react'
  }
};

describe('PageCanvas width lifecycle', () => {
  test.each([false, true])(
    'fills a single automatic block and follows viewport resizing (design=%s)',
    async (isDesignMode) => {
      const width = vi
        .spyOn(HTMLElement.prototype, 'offsetWidth', 'get')
        .mockReturnValue(1200);
      const bounds = vi
        .spyOn(HTMLElement.prototype, 'getBoundingClientRect')
        .mockImplementation(function (this: HTMLElement) {
          return {
            x: 0,
            y: 80,
            top: 80,
            bottom: 1000,
            left: 0,
            right: 1200,
            width: 1200,
            height: this.hasAttribute('data-flowbase-frontstage-canvas-footer')
              ? 32
              : 920,
            toJSON: () => ({})
          };
        });
      const originalHeight = window.innerHeight;
      Object.defineProperty(window, 'innerHeight', {
        configurable: true,
        value: 1000
      });
      const content = createFrontstagePageContentFixture();
      const canvas = (
        blocks: FrontstageBlockInstance[],
        layoutMode = 'auto'
      ) => (
        <div data-flowbase-frontstage-scroll-owner="">
          <PageCanvas
            content={{
              ...content,
              document: {
                ...content.document,
                payload: { 'x-layout-mode': layoutMode }
              }
            }}
            runtimeBlocks={blocks}
            isDesignMode={isDesignMode}
          />
          {isDesignMode && (
            <button
              data-flowbase-frontstage-canvas-footer=""
              style={{ margin: '8px 16px 16px' }}
            >
              Add block
            </button>
          )}
        </div>
      );
      const view = render(canvas([runtimeBlock]));
      const itemHeight = () =>
        Number.parseFloat(
          view.container.querySelector<HTMLElement>('.react-grid-item')?.style
            .height ?? '0'
        );
      try {
        await waitFor(() =>
          expect(itemHeight()).toBe(isDesignMode ? 864 : 918)
        );
        act(() => {
          Object.defineProperty(window, 'innerHeight', {
            configurable: true,
            value: 700
          });
          window.dispatchEvent(new Event('resize'));
        });
        await waitFor(() =>
          expect(itemHeight()).toBe(isDesignMode ? 564 : 618)
        );
        view.rerender(
          canvas([runtimeBlock, { ...runtimeBlock, id: 'second' }])
        );
        await waitFor(() => expect(itemHeight()).toBeLessThan(618));
        view.rerender(
          canvas([
            {
              ...runtimeBlock,
              presentation: { heightMode: 'fixed', height: 240 }
            }
          ])
        );
        await waitFor(() => expect(itemHeight()).toBe(252));
        view.rerender(canvas([runtimeBlock], 'free'));
        await waitFor(() => expect(itemHeight()).toBeLessThan(618));
        view.rerender(canvas([runtimeBlock]));
        await waitFor(() =>
          expect(itemHeight()).toBe(isDesignMode ? 564 : 618)
        );
      } finally {
        view.unmount();
        width.mockRestore();
        bounds.mockRestore();
        Object.defineProperty(window, 'innerHeight', {
          configurable: true,
          value: originalHeight
        });
      }
    }
  );

  test('AC-001 measures a host mounted after loading and tracks later resizes', async () => {
    const originalResizeObserver = globalThis.ResizeObserver;
    const observe = vi.fn();
    const resizeCallbacks = new Map<Element, ResizeObserverCallback>();

    class ResizeObserverHarness implements ResizeObserver {
      private readonly callback: ResizeObserverCallback;

      constructor(callback: ResizeObserverCallback) {
        this.callback = callback;
      }

      observe = (target: Element) => {
        observe(target);
        resizeCallbacks.set(target, this.callback);
      };
      unobserve = vi.fn();
      disconnect = vi.fn();
    }

    globalThis.ResizeObserver = ResizeObserverHarness;

    try {
      const view = render(<PageCanvas isLoading />);

      view.rerender(
        <PageCanvas
          content={createFrontstagePageContentFixture()}
          runtimeBlocks={[runtimeBlock]}
          renderBlockIds={[runtimeBlock.id]}
        />
      );

      const measuredHost = screen.getByTestId('page-canvas-render-slots');
      await waitFor(() => expect(observe).toHaveBeenCalledWith(measuredHost));

      const observedResize = resizeCallbacks.get(measuredHost);
      expect(observedResize).toEqual(expect.any(Function));
      await act(async () => {
        observedResize?.(
          [
            {
              target: measuredHost,
              contentRect: { width: 1834 } as DOMRectReadOnly,
              borderBoxSize: [],
              contentBoxSize: [],
              devicePixelContentBoxSize: []
            } as ResizeObserverEntry
          ],
          {} as ResizeObserver
        );
      });

      await waitFor(() => {
        const gridItem =
          measuredHost.querySelector<HTMLElement>('.react-grid-item');
        expect(Number.parseFloat(gridItem?.style.width ?? '0')).toBeGreaterThan(
          1700
        );
      });
    } finally {
      globalThis.ResizeObserver = originalResizeObserver;
    }
  });
});
