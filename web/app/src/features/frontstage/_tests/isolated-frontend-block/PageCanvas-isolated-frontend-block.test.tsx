import { render, screen, within } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

import type { PreparedFrontstageIsolatedContribution } from '../../lib/isolated-frontend-block-contribution';
import type { FrontstageBlockInstance } from '../../lib/page-document';
import type { IsolatedFrontendBlockCapabilityHandlers } from '@1flowbase/page-runtime';

vi.mock('../../lib/isolated-frontend-block-react-adapter', () => ({
  FrontstageIsolatedFrontendBlockHost: ({
    preparation,
    capabilityHandlers
  }: {
    preparation: PreparedFrontstageIsolatedContribution;
    capabilityHandlers?: IsolatedFrontendBlockCapabilityHandlers;
  }) => (
    <div
      data-testid={`mounted-isolated-contribution-${preparation.blockInstanceId}`}
      data-publish-capability={
        capabilityHandlers?.['block.output.publish'] ? 'available' : 'denied'
      }
    />
  )
}));

import { PageCanvas } from '../../components/PageCanvas';
import { createFrontstagePageContentFixture } from '../frontstage-page-content-fixtures';

describe('PageCanvas isolated frontend contributions', () => {
  test('D5-P3 mounts a prepared contribution only in its matching block slot', () => {
    const runtimeBlocks = [
      isolatedBlock('isolated-a'),
      isolatedBlock('isolated-b')
    ];

    render(
      <PageCanvas
        content={createFrontstagePageContentFixture()}
        runtimeBlocks={runtimeBlocks}
        renderBlockIds={runtimeBlocks.map(({ id }) => id)}
        isolatedRuntimePreparations={[preparation('isolated-b')]}
        isolatedCapabilityHandlersByBlockId={{
          'isolated-b': { 'block.output.publish': vi.fn() }
        }}
      />
    );

    expect(
      within(screen.getByTestId('block-slot-isolated-a')).getByTestId(
        'block-ui-loading-shell'
      )
    ).toBeInTheDocument();
    expect(
      within(screen.getByTestId('block-slot-isolated-b')).getByTestId(
        'mounted-isolated-contribution-isolated-b'
      )
    ).toHaveAttribute('data-publish-capability', 'available');
    expect(
      screen.queryByTestId('mounted-isolated-contribution-isolated-a')
    ).not.toBeInTheDocument();
  });
});

function isolatedBlock(id: string): FrontstageBlockInstance {
  return {
    id,
    rendererVersion: 'v1',
    sourceId: id,
    codeRef: '',
    sourceCodeRef: null,
    catalog: {
      providerCode: 'official',
      installationId: 'installation-1'
    },
    contribution: {
      pluginId: 'official.blocks',
      pluginVersion: '1.0.0',
      code: 'isolated-chart'
    },
    props: {},
    ports: { inputs: [], outputs: [] },
    presentation: { heightMode: 'auto', height: null },
    layout: { order: id === 'isolated-a' ? 0 : 1 },
    order: id === 'isolated-a' ? 0 : 1,
    runtime: {
      kind: 'isolated_iframe',
      entry: '@1flowbase/isolated-chart',
      hint: 'isolated_iframe'
    }
  };
}

function preparation(
  blockInstanceId: string
): PreparedFrontstageIsolatedContribution {
  return {
    state: 'prepared',
    blockInstanceId,
    contributionId: 'frontend-block.installation-1.isolated-chart',
    blockId: 'installation-1:isolated-chart',
    blockVersion: '1.0.0',
    graphFingerprint: 'graph-fingerprint',
    runtimeKind: 'isolated',
    executionKind: 'ui_mount',
    isolationRequirement: 'independent_realm',
    lifecycleKind: 'workspace_assignment',
    grantedPermissions: ['frontend-block.ui-mount.isolated-realm'],
    assetIntegrity: 'verified_sha256',
    program: {
      source: 'globalThis.__oneflowbaseIsolatedBlock = {};',
      props: {}
    }
  };
}
