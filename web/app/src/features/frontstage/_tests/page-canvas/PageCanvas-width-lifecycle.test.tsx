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
      expect(observedResize).toBeDefined();
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
