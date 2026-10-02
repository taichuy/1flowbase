import './navigation';
import { fireEvent, render, screen, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { beforeEach, expect, test, vi } from 'vitest';

import { ProviderTrajectory } from '../../../components/debug-console/trajectory/ProviderTrajectory';
import type { ConversationLogTraceLoader } from '../../../components/debug-console/conversation-log-trace-model';
import { NativeBlockSurfaceProvider } from '../../../../frontstage/lib/native-modules/native-block-surface-context';
import { createNativeOverlayHost } from '../../../../frontstage/lib/native-modules/native-overlay-host';
import { createNativeBlockSurfaceRuntime } from '../../../../frontstage/lib/native-modules/surface/native-block-surface-runtime';
import { WindowWorkspaceWindow } from '../../../../../shared/ui/window-workspace/WindowWorkspaceWindow';
import { appI18n } from '../../../../../shared/i18n/app-i18n';

beforeEach(async () => {
  await appI18n.changeLanguage('zh_Hans');
});

function fixture() {
  const loader: ConversationLogTraceLoader = {
    loadTree: vi.fn(),
    loadChildren: vi.fn(),
    loadContent: vi.fn(),
    loadClientTrajectory: vi.fn().mockResolvedValue({
      items: [],
      next_cursor: null,
      integrity: 'complete'
    }),
    loadWorkflowTrajectory: vi.fn().mockResolvedValue({
      items: [],
      next_cursor: null
    })
  };
  return (
    <QueryClientProvider
      client={
        new QueryClient({
          defaultOptions: { queries: { retry: false } }
        })
      }
    >
      <WindowWorkspaceWindow
        active
        zIndex={1100}
        title="运行详情"
        testId="parent-window"
        dragHandleSelector=".parent-header"
        initialRect={() => ({ left: 16, top: 16, width: 600, height: 500 })}
        onActivate={() => {}}
        resizeLabel={(edge) => edge}
      >
        <ProviderTrajectory runId="run-1" loader={loader} />
      </WindowWorkspaceWindow>
    </QueryClientProvider>
  );
}

test('keeps a block trajectory in the styled overlay above its parent and removes it on close', async () => {
  const host = document.createElement('div');
  document.body.append(host);
  const targetRoot = host.attachShadow({ mode: 'open' });
  const overlayHost = createNativeOverlayHost({
    blockId: 'logs-block',
    targetRoot
  });
  const scope = createNativeBlockSurfaceRuntime({
    layoutEpoch: 'test',
    overlayHost,
    scrollOwner: window,
    targetRoot
  });
  const view = render(
    <NativeBlockSurfaceProvider scope={scope}>
      {fixture()}
    </NativeBlockSurfaceProvider>
  );
  try {
    fireEvent.click(screen.getByRole('button', { name: '总轨迹' }));
    const trajectory = overlayHost.container.querySelector<HTMLElement>(
      '[data-testid="trajectory-window"]'
    );
    expect(trajectory).not.toBeNull();
    expect(trajectory!.getRootNode()).toBe(targetRoot);
    expect(Number(trajectory!.style.zIndex)).toBeGreaterThan(
      Number(screen.getByTestId('parent-window').style.zIndex)
    );
    const dialog = within(trajectory!);
    fireEvent.click(dialog.getByRole('radio', { name: '工作流内部事件' }));
    await dialog.findByRole('toolbar', { name: '概览' });
    fireEvent.click(dialog.getByRole('button', { name: '关闭轨迹窗口' }));
    expect(
      overlayHost.container.querySelector('[data-testid="trajectory-window"]')
    ).toBeNull();
    expect(screen.getByTestId('parent-window')).toBeInTheDocument();
  } finally {
    view.unmount();
    scope.dispose();
    overlayHost.dispose();
    host.remove();
  }
});

test('retains document mounting and parent-relative stacking outside a block', () => {
  render(fixture());
  fireEvent.click(screen.getByRole('button', { name: '总轨迹' }));
  const trajectory = screen.getByTestId('trajectory-window');
  expect(trajectory.parentElement).toBe(document.body);
  expect(Number(trajectory.style.zIndex)).toBeGreaterThan(
    Number(screen.getByTestId('parent-window').style.zIndex)
  );
});
