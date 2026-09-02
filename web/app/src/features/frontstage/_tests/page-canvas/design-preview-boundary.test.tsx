import { render, screen } from '@testing-library/react';
import { describe, expect, test } from 'vitest';

import type { FrontstagePageContent } from '../../api/page-content';
import { PageCanvas } from '../../components/PageCanvas';
import type { FrontstageBlockInstance } from '../../lib/page-document';
import { createFrontstagePageContentFixture } from '../frontstage-page-content-fixtures';

function createRuntimeBlock(): FrontstageBlockInstance {
  return {
    id: 'fixed-overlay',
    rendererVersion: 'v1',
    sourceId: 'fixed-overlay',
    codeRef: 'fixed-overlay-code',
    sourceCodeRef: 'fixed-overlay-code',
    catalog: { providerCode: null, installationId: null },
    contribution: {
      pluginId: null,
      pluginVersion: null,
      code: 'official.fixed-overlay'
    },
    props: {},
    ports: { inputs: [], outputs: [] },
    presentation: { heightMode: 'auto', height: null },
    layout: { order: 0, region: 'main' },
    order: 0,
    runtime: {
      kind: 'native_react',
      entry: 'blocks/fixed-overlay.tsx',
      hint: 'native_react'
    }
  };
}

describe('design preview boundary', () => {
  test('contains fixed-positioned block content below design controls', () => {
    const content =
      createFrontstagePageContentFixture() as FrontstagePageContent;

    render(
      <PageCanvas
        content={content}
        isDesignMode
        runtimeBlocks={[createRuntimeBlock()]}
      />
    );

    const loadingShell = screen.getByTestId('block-ui-loading-shell');

    expect(loadingShell.parentElement).toHaveStyle({
      position: 'relative',
      zIndex: '0',
      isolation: 'isolate',
      contain: 'layout paint',
      overflow: 'clip',
      minHeight: '160px'
    });
  });
});
