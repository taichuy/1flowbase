/* eslint-disable react-refresh/only-export-components */
import { ConfigProvider } from 'antd';
import { useMemo, useState } from 'react';
import { createRoot } from 'react-dom/client';

import { PageCanvas } from '../../components/PageCanvas';
import type { FrontstageBlockInstance } from '../../lib/page-document';
import type { FrontstagePersistedGridLayout } from '../../lib/responsive-grid-layout';
import { createFrontstagePageContentFixture } from '../frontstage-page-content-fixtures';

const FIXTURE_STORAGE_KEY = 'frontstage-drag-auto-scroll-fixture';
const initialBlocks = Array.from({ length: 10 }, (_, index) =>
  createFixtureBlock(`block-${index + 1}`, index)
);

function FrontstageDragAutoScrollFixture() {
  const [initialState] = useState(readFixtureState);
  const [blocks, setBlocks] = useState(initialState.blocks);
  const [saveCount, setSaveCount] = useState(initialState.saveCount);
  const content = useMemo(
    () =>
      createFrontstagePageContentFixture({
        page: { title: 'Drag auto-scroll fixture' },
        document: { payload: { 'x-layout-mode': 'auto' } }
      }),
    []
  );

  const positions = Object.fromEntries(
    blocks.map((block) => [block.id, readDesktopY(block)])
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
          data-testid="frontstage-drag-auto-scroll-stats"
          data-ready-signal="settled"
          data-save-count={saveCount}
          data-positions={JSON.stringify(positions)}
        />
        <div
          data-flowbase-frontstage-scroll-owner
          data-testid="frontstage-scroll-owner"
          style={{ height: 520, overflowY: 'auto', overscrollBehavior: 'contain' }}
        >
          <PageCanvas
            content={content}
            runtimeBlocks={blocks}
            renderBlockIds={blocks.map((block) => block.id)}
            isDesignMode
            designActions={{ onEditCode: () => {}, onDelete: () => {} }}
            showTitle={false}
            onResponsiveLayoutSave={saveLayout}
          />
        </div>
      </main>
    </ConfigProvider>
  );
}

function createFixtureBlock(
  id: string,
  order: number
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
      x: 0,
      y: order * 12,
      w: 24
    },
    order,
    runtime: {
      kind: 'native_react',
      entry: `blocks/${id}.js`,
      hint: 'native_react'
    }
  };
}

function readDesktopY(block: FrontstageBlockInstance): number {
  const lg = block.layout.lg;
  if (typeof lg === 'object' && lg !== null && !Array.isArray(lg)) {
    const y = (lg as Record<string, unknown>).y;
    if (typeof y === 'number') return y;
  }
  return typeof block.layout.y === 'number' ? block.layout.y : -1;
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
if (!root) throw new Error('Frontstage drag auto-scroll fixture root missing.');

createRoot(root).render(<FrontstageDragAutoScrollFixture />);
