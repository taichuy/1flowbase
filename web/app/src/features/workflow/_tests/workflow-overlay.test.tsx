import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

import { AppProviders } from '../../../app/AppProviders';
import { appI18n } from '../../../shared/i18n/app-i18n';
import { WorkflowOverlay } from '../components/WorkflowOverlay';

async function renderWithProviders(ui: ReactNode) {
  return await renderSettled(<AppProviders>{ui}</AppProviders>);
}

function baseProps() {
  return {
    applicationName: 'Ticket Workflow',
    autosaveLabel: '每 30 秒自动保存',
    autosaveStatus: 'idle' as const,
    issueErrorCount: 0,
    saveDisabled: false,
    saveLoading: false,
    testRunAction: null,
    published: false,
    publishDisabled: false,
    publishLoading: false,
    revertLoading: false,
    onOpenEnvironmentVariables: vi.fn(),
    onOpenHistory: vi.fn(),
    onOpenIssues: vi.fn(),
    onSaveDraft: vi.fn(),
    onPublish: vi.fn(),
    onRevertToDraft: vi.fn()
  };
}

describe('WorkflowOverlay publish entry (AC-005)', () => {
  beforeEach(async () => {
    await appI18n.changeLanguage('zh_Hans');
  });

  afterEach(async () => {
    cleanup();
    await act(async () => { await appI18n.changeLanguage('en_US'); });
    vi.clearAllMocks();
  });

  test('exposes a publish action inside the workflow action bar', async () => {
    await renderWithProviders(<WorkflowOverlay {...baseProps()} />);

    const actionBar = screen.getByRole('region', {
      name: 'Workflow 操作栏'
    });
    expect(
      within(actionBar).getByRole('button', { name: '发布' })
    ).toBeInTheDocument();
  });

  test('publish button triggers the publish handler', async () => {
    const props = baseProps();
    await renderWithProviders(<WorkflowOverlay {...props} />);

    await domEvents.click(screen.getByRole('button', { name: '发布' }));

    expect(props.onPublish).toHaveBeenCalledTimes(1);
    expect(props.onRevertToDraft).not.toHaveBeenCalled();
  });

  test('revert to draft is unavailable until the workflow is published', async () => {
    const props = baseProps();
    await renderWithProviders(<WorkflowOverlay {...props} published={false} />);

    await domEvents.click(screen.getByRole('button', { name: '更多发布操作' }));

    const revertItem = screen.getByRole('menuitem', { name: '退回草稿' });
    expect(revertItem).toHaveAttribute('aria-disabled', 'true');
  });

  test('published workflow can revert to draft from the publish menu', async () => {
    const props = baseProps();
    await renderWithProviders(<WorkflowOverlay {...props} published />);

    await domEvents.click(screen.getByRole('button', { name: '更多发布操作' }));
    await domEvents.click(screen.getByRole('menuitem', { name: '退回草稿' }));

    expect(props.onRevertToDraft).toHaveBeenCalledTimes(1);
    expect(props.onPublish).not.toHaveBeenCalled();
  });
});

async function dispatch<T>(event: () => T) {
  let result!: T;
  await act(async () => { result = event(); });
  return result;
}

async function renderSettled(...args: Parameters<typeof render>) {
  let view!: ReturnType<typeof render>;
  await act(async () => { view = render(...args); });
  return view;
}

const domEvents = {
  click: (...args: Parameters<typeof fireEvent.click>) => dispatch(() => fireEvent.click(...args))
};
