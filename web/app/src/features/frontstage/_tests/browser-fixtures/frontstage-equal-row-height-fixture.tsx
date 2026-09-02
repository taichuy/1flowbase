/* eslint-disable react-refresh/only-export-components */
import { Button, ConfigProvider } from 'antd';
import { useMemo, useState } from 'react';
import { createRoot } from 'react-dom/client';

import { PageCanvas } from '../../components/PageCanvas';
import type { FrontstageBlockInstance } from '../../lib/page-document';
import { createFrontstagePageContentFixture } from '../frontstage-page-content-fixtures';

const blocks = [
  createFixtureBlock('short', 0, 0, 0, 12),
  createFixtureBlock('tall', 1, 0, 12, 12),
  createFixtureBlock('following', 2, 144, 0, 24)
];

function FrontstageEqualRowHeightFixture() {
  const [tallHeight, setTallHeight] = useState(420);
  const content = useMemo(
    () =>
      createFrontstagePageContentFixture({
        page: { title: 'Equal row height fixture' },
        document: { payload: { 'x-layout-mode': 'auto' } }
      }),
    []
  );

  return (
    <ConfigProvider>
      <style>{`
        [data-flowbase-frontstage-intrinsic-content="short"] { height: 200px; }
        [data-flowbase-frontstage-intrinsic-content="tall"] { height: ${tallHeight}px; }
        [data-flowbase-frontstage-intrinsic-content="following"] { height: 240px; }
      `}</style>
      <main style={{ padding: 24 }}>
        <Button
          data-testid="shrink-tall-content"
          onClick={() => setTallHeight(170)}
        >
          shrink tall content
        </Button>
        <div
          data-testid="frontstage-equal-row-height-stats"
          data-ready-signal="settled"
          data-tall-height={tallHeight}
        />
        <PageCanvas
          content={content}
          runtimeBlocks={blocks}
          renderBlockIds={blocks.map((block) => block.id)}
          isDesignMode
          designActions={{ onEditCode: () => {}, onDelete: () => {} }}
          showTitle={false}
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
    presentation: { heightMode: 'auto', height: null },
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

const root = document.getElementById('root');
if (!root) throw new Error('Equal row height fixture root missing.');

createRoot(root).render(<FrontstageEqualRowHeightFixture />);
