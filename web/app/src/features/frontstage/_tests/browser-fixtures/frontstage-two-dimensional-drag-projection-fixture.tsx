/* eslint-disable react-refresh/only-export-components */
import { ConfigProvider } from 'antd';
import { useMemo, useState } from 'react';
import { createRoot } from 'react-dom/client';

import { PageCanvas } from '../../components/PageCanvas';
import type { FrontstageBlockInstance } from '../../lib/page-document';
import type { FrontstagePersistedGridLayout } from '../../lib/responsive-grid-layout';
import { createFrontstagePageContentFixture } from '../frontstage-page-content-fixtures';

const FIXTURE_STORAGE_KEY = 'frontstage-two-dimensional-drag-projection';
const initialBlocks = [
  createFixtureBlock('first', 0, 0, 0, 12),
  createFixtureBlock('second', 1, 0, 12, 12),
  createFixtureBlock('middle', 2, 64, 0, 24),
  createFixtureBlock('active', 3, 128, 0, 24)
];

function FrontstageTwoDimensionalDragProjectionFixture() {
  const [initialState] = useState(readFixtureState);
  const [blocks, setBlocks] = useState(initialState.blocks);
  const [saveCount, setSaveCount] = useState(initialState.saveCount);
  const content = useMemo(
    () =>
      createFrontstagePageContentFixture({
        page: { title: 'Two-dimensional drag projection fixture' },
        document: { payload: { 'x-layout-mode': 'auto' } }
      }),
    []
  );

  const layouts = Object.fromEntries(
    blocks.map((block) => [block.id, readDesktopLayout(block)])
  );

  const saveLayout = (nextLayouts: FrontstagePersistedGridLayout) => {
    const nextBlocks = blocks.map((block) => ({
      ...block,
      layout: {
        ...block.layout,
        ...(nextLayouts[block.id] ?? {})
      }
    }));
    const nextSaveCount = saveCount + 1;
    localStorage.setItem(
      FIXTURE_STORAGE_KEY,
      JSON.stringify({ blocks: nextBlocks, saveCount: nextSaveCount })
    );
    setBlocks(nextBlocks);
    setSaveCount(nextSaveCount);
  };

  return (
    <ConfigProvider>
      <main style={{ padding: 24 }}>
        <div
          data-testid="frontstage-two-dimensional-drag-stats"
          data-ready-signal="settled"
          data-save-count={saveCount}
          data-layouts={JSON.stringify(layouts)}
        />
        <PageCanvas
          content={content}
          runtimeBlocks={blocks}
          renderBlockIds={blocks.map((block) => block.id)}
          isDesignMode
          designActions={{ onEditCode: () => {}, onDelete: () => {} }}
          showTitle={false}
          onResponsiveLayoutSave={saveLayout}
        />
      </main>
    </ConfigProvider>
  );
}

function createFixtureBlock(
  id: string,
  order: number,
  y: number,
  x: number,
  w: number
): FrontstageBlockInstance {
  return {
    id,
    title: id,
    rendererVersion: 'v1',
    sourceId: id,
    codeRef: `${id}-code`,
    sourceCodeRef: `${id}-code`,
    catalog: { providerCode: null, installationId: null },
    contribution: {
      pluginId: null,
      pluginVersion: null,
      code: `fixture.${id}`
    },
    props: {},
    ports: { inputs: [], outputs: [] },
    presentation: { heightMode: 'fixed', height: 180 },
    layout: {
      order,
      gridColumns: 24,
      verticalGridVersion: 2,
      x,
      y,
      w
    },
    order,
    runtime: {
      kind: 'native_react',
      entry: `blocks/${id}.js`,
      hint: 'native_react'
    }
  };
}

function readDesktopLayout(block: FrontstageBlockInstance) {
  const stored = block.layout.lg;
  const layout =
    typeof stored === 'object' && stored !== null && !Array.isArray(stored)
      ? (stored as Record<string, unknown>)
      : block.layout;
  return {
    x: typeof layout.x === 'number' ? layout.x : -1,
    y: typeof layout.y === 'number' ? layout.y : -1,
    w: typeof layout.w === 'number' ? layout.w : -1
  };
}

function readFixtureState(): {
  blocks: FrontstageBlockInstance[];
  saveCount: number;
} {
  const stored = localStorage.getItem(FIXTURE_STORAGE_KEY);
  if (!stored) return { blocks: initialBlocks, saveCount: 0 };
  return JSON.parse(stored) as {
    blocks: FrontstageBlockInstance[];
    saveCount: number;
  };
}

const root = document.getElementById('root');
if (!root) throw new Error('Two-dimensional drag projection root missing.');

createRoot(root).render(<FrontstageTwoDimensionalDragProjectionFixture />);
