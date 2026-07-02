import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const applicationsApi = vi.hoisted(() => ({
  applicationsQueryKey: ['applications'],
  createApplication: vi.fn()
}));

vi.mock('../api/applications', () => applicationsApi);

const publicApi = vi.hoisted(() => ({
  saveApplicationApiMapping: vi.fn(),
  saveWorkflowScheduleTrigger: vi.fn()
}));

vi.mock('../api/public-api', () => publicApi);

import { AppProviders } from '../../../app/AppProviders';

import { ApplicationCreateModal } from '../components/ApplicationCreateModal';

describe('ApplicationCreateModal', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    applicationsApi.createApplication.mockResolvedValue({ id: 'app-workflow' });
    publicApi.saveApplicationApiMapping.mockResolvedValue({});
    publicApi.saveWorkflowScheduleTrigger.mockResolvedValue({});
  });

  test('keeps form semantics after migrating to the shared modal shell', () => {
    render(
      <AppProviders>
        <ApplicationCreateModal
          open
          csrfToken="csrf-123"
          onClose={vi.fn()}
          onCreated={vi.fn()}
        />
      </AppProviders>
    );

    expect(screen.getByText('新建应用')).toBeInTheDocument();
    expect(screen.getByRole('textbox', { name: '名称' })).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: '创建应用' })
    ).toBeInTheDocument();
  }, 10_000);

  test('creates workflow applications with only the trigger type before entering the editor', async () => {
    const onCreated = vi.fn();

    render(
      <AppProviders>
        <ApplicationCreateModal
          open
          csrfToken="csrf-123"
          onClose={vi.fn()}
          onCreated={onCreated}
        />
      </AppProviders>
    );

    expect(screen.getByRole('radio', { name: /AgentFlow/i })).toBeEnabled();
    expect(screen.getByRole('radio', { name: /Workflow/i })).toBeEnabled();

    fireEvent.click(screen.getByRole('radio', { name: /Workflow/i }));
    fireEvent.click(screen.getByRole('radio', { name: '定时触发' }));
    expect(
      screen.queryByRole('textbox', { name: '接口 slug' })
    ).not.toBeInTheDocument();
    expect(screen.queryByText('HTTP method')).not.toBeInTheDocument();
    expect(screen.queryByText('响应模式')).not.toBeInTheDocument();
    expect(screen.queryByText('参数名')).not.toBeInTheDocument();
    expect(screen.queryByText('目标 selector')).not.toBeInTheDocument();
    expect(
      screen.queryByRole('textbox', { name: '定时表达式' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('textbox', { name: '时区' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('textbox', { name: '输入 payload' })
    ).not.toBeInTheDocument();

    fireEvent.change(screen.getByRole('textbox', { name: '名称' }), {
      target: { value: 'Daily workflow' }
    });

    fireEvent.click(screen.getByRole('button', { name: '创建应用' }));

    await waitFor(() => {
      expect(applicationsApi.createApplication).toHaveBeenCalledWith(
        expect.objectContaining({
          application_type: 'workflow',
          name: 'Daily workflow'
        }),
        'csrf-123'
      );
    });
    expect(publicApi.saveWorkflowScheduleTrigger).not.toHaveBeenCalled();
    expect(publicApi.saveApplicationApiMapping).not.toHaveBeenCalled();
    expect(onCreated).toHaveBeenCalledWith('app-workflow');
    expect(screen.queryByText('未开放')).not.toBeInTheDocument();
  });
});
