import { render, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { expect, test, vi } from 'vitest';

import { ApplicationRunLogViewer } from '../../ApplicationRunLogViewer';
import { AllAgentFlowLogs, ApplicationLogs } from '../ApplicationLogs';
import { NativeBlockSurfaceProvider } from '../../../../../frontstage/lib/native-modules/native-block-surface-context';
import { createNativeOverlayHost } from '../../../../../frontstage/lib/native-modules/native-overlay-host';
import { createNativeBlockSurfaceRuntime } from '../../../../../frontstage/lib/native-modules/surface/native-block-surface-runtime';
import trajectoryVirtualListStyles from '../../../../../agent-flow/components/debug-console/trajectory/virtual/trajectory-virtual-list.css?inline';

import readOnlyConversationInputStyles from '../../../../../agent-flow/components/debug-console/conversation/read-only/read-only-conversation-input.css?inline';

vi.mock(
  '../../../../../agent-flow/components/debug-console/conversation/read-only/read-only-conversation-input.css?inline',
  async () => {
    const { readFile } = await import('node:fs/promises');
    return {
      default: await readFile(
        `${process.cwd()}/src/features/agent-flow/components/debug-console/conversation/read-only/read-only-conversation-input.css`,
        'utf8'
      )
    };
  }
);

// Vitest stubs CSS imports; retain the actual stylesheet for this injection contract.
vi.mock(
  '../../../../../agent-flow/components/debug-console/trajectory/virtual/trajectory-virtual-list.css?inline',
  async () => {
    const { readFile } = await import('node:fs/promises');
    return {
      default: await readFile(
        `${process.cwd()}/src/features/agent-flow/components/debug-console/trajectory/virtual/trajectory-virtual-list.css`,
        'utf8'
      )
    };
  }
);

vi.mock(
  '../../../../../../shared/ui/data-table/data-table-column-settings.css?inline',
  async () => {
    const { readFile } = await import('node:fs/promises');
    return {
      default: await readFile(
        `${process.cwd()}/src/shared/ui/data-table/data-table-column-settings.css`,
        'utf8'
      )
    };
  }
);

vi.mock('../../../../api/applications', () => ({
  applicationsQueryKey: ['applications'],
  fetchApplications: vi.fn().mockResolvedValue([])
}));
vi.mock('../ApplicationLogsWorkspace', () => ({
  ApplicationLogsWorkspace: () => <div>Logs workspace</div>
}));
vi.mock('../../ApplicationRunDetailPanel', () => ({
  ApplicationRunDetailPanel: () => <div>Run details</div>
}));

test.each([
  ['single application', <ApplicationLogs applicationId="app-1" />],
  ['all applications', <AllAgentFlowLogs />],
  [
    'run viewer',
    <ApplicationRunLogViewer
      applicationId="app-1"
      runId="run-1"
      view="detail"
      onClose={() => {}}
    />
  ]
])(
  'injects virtual-list and shared composer styles into the %s overlay ShadowRoot',
  async (name, component) => {
    const host = document.createElement('div');
    document.body.append(host);
    const targetRoot = host.attachShadow({ mode: 'open' });
    const overlayHost = createNativeOverlayHost({
      blockId: 'logs',
      targetRoot
    });
    const scope = createNativeBlockSurfaceRuntime({
      layoutEpoch: 'test',
      overlayHost,
      scrollOwner: window,
      targetRoot
    });
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } }
    });
    const view = render(
      <QueryClientProvider client={client}>
        <NativeBlockSurfaceProvider scope={scope}>
          {component}
        </NativeBlockSurfaceProvider>
      </QueryClientProvider>
    );
    try {
      // A document-head import cannot style virtualized rows in a block overlay.
      // Layout itself is covered by the real-browser embedded trajectory regression.
      expect(trajectoryVirtualListStyles).toContain('.trajectory-virtual-list');
      expect(readOnlyConversationInputStyles).toContain(
        '.read-only-conversation-input__metadata'
      );
      const style = await waitFor(() => {
        const injectedStyle = overlayHost.container.querySelector(
          'style[data-application-logs-overlay-styles], style[data-application-run-log-viewer-styles]'
        );
        expect(injectedStyle?.textContent).toContain(readOnlyConversationInputStyles);
        return injectedStyle;
      });
      expect(style?.getRootNode()).toBe(targetRoot);
      expect(style?.textContent).toContain(trajectoryVirtualListStyles);
      if (name !== 'run viewer') {
        expect(style?.textContent).toContain('.data-table-columns__list');
        expect(style?.textContent).toContain('overscroll-behavior-y: contain');
        expect(
          view.container.querySelector('style[data-application-logs-styles]')
            ?.textContent
        ).toContain('.data-table-columns__list');
      }
    } finally {
      view.unmount();
      client.clear();
      scope.dispose();
      overlayHost.dispose();
      host.remove();
    }
  }
);
