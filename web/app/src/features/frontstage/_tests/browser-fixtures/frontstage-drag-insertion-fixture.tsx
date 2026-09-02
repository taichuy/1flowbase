/* eslint-disable react-refresh/only-export-components */
import { ConfigProvider } from 'antd';
import { useMemo, useState } from 'react';
import { createRoot } from 'react-dom/client';

import { PageCanvas } from '../../components/PageCanvas';
import type { FrontstageBlockInstance } from '../../lib/page-document';
import type { FrontstagePersistedGridLayout } from '../../lib/responsive-grid-layout';
import { createFrontstagePageContentFixture } from '../frontstage-page-content-fixtures';

const initialBlocks = [
  createFixtureBlock('first', 0, 0),
  createFixtureBlock('second', 1, 12)
];
const FIXTURE_STORAGE_KEY = 'frontstage-drag-insertion-fixture';

function FrontstageDragInsertionFixture() {
  const [initialState] = useState(readFixtureState);
  const [blocks, setBlocks] = useState(initialState.blocks);
  const [saveCount, setSaveCount] = useState(initialState.saveCount);
  const content = useMemo(
    () =>
      createFrontstagePageContentFixture({
        page: { title: 'Drag insertion fixture' },
        document: { payload: { 'x-layout-mode': 'auto' } }
      }),
    []
  );

  const positions = Object.fromEntries(
    blocks.map((block) => [block.id, readDesktopX(block)])
  );

  const saveLayout = (layouts: FrontstagePersistedGridLayout) => {
    const nextBlocks = blocks.map((block) => ({
      ...block,
      layout: {
        ...block.layout,
        ...(layouts[block.id] ?? {})
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
          data-testid="frontstage-drag-stats"
          data-ready-signal="settled"
          data-save-count={saveCount}
          data-first-x={positions.first}
          data-second-x={positions.second}
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
  x: number
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
      y: 0,
      w: 12
    },
    order,
    runtime: {
      kind: 'native_react',
      entry: `blocks/${id}.js`,
      hint: 'native_react'
    }
  };
}

function readDesktopX(block: FrontstageBlockInstance): number {
  const lg = block.layout.lg;
  if (typeof lg === 'object' && lg !== null && !Array.isArray(lg)) {
    const x = (lg as Record<string, unknown>).x;
    if (typeof x === 'number') return x;
  }
  return typeof block.layout.x === 'number' ? block.layout.x : -1;
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
if (!root) throw new Error('Frontstage drag insertion fixture root missing.');

createRoot(root).render(<FrontstageDragInsertionFixture />);
