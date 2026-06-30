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
    expect(screen.getByRole('button', { name: '创建应用' })).toBeInTheDocument();
  }, 10_000);

  test('creates workflow applications with a schedule trigger before entering the editor', async () => {
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
    expect(publicApi.saveWorkflowScheduleTrigger).toHaveBeenCalledWith(
      'app-workflow',
      {
        enabled: true,
        cron: '0 9 * * *',
        timezone: 'UTC',
        input_payload: {}
      },
      'csrf-123'
    );
    expect(onCreated).toHaveBeenCalledWith('app-workflow');
    expect(screen.queryByText('未开放')).not.toBeInTheDocument();
  });

  test('submits workflow extension trigger method and parameter mappings', async () => {
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

    fireEvent.click(screen.getByRole('radio', { name: /Workflow/i }));
    fireEvent.change(screen.getByRole('textbox', { name: '名称' }), {
      target: { value: 'Webhook workflow' }
    });
    fireEvent.change(screen.getByRole('textbox', { name: '接口 slug' }), {
      target: { value: 'ticket_webhook' }
    });
    fireEvent.click(screen.getByRole('radio', { name: 'PATCH' }));

    for (const method of ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS']) {
      expect(screen.getByRole('radio', { name: method })).toBeInTheDocument();
    }

    const names = screen.getAllByRole('textbox', { name: '参数名' });
    const targets = screen.getAllByRole('textbox', { name: '目标 selector' });
    fireEvent.change(names[0], { target: { value: 'ticket_id' } });
    fireEvent.change(targets[0], {
      target: { value: 'node-workflow-start.path.ticket_id' }
    });
    fireEvent.change(names[1], { target: { value: 'include_history' } });
    fireEvent.change(targets[1], {
      target: { value: 'node-workflow-start.query.include_history' }
    });
    fireEvent.change(names[2], { target: { value: 'assignee' } });
    fireEvent.change(targets[2], {
      target: { value: 'node-workflow-start.form.assignee' }
    });
    fireEvent.change(names[3], { target: { value: 'summary' } });
    fireEvent.change(targets[3], {
      target: { value: 'node-workflow-start.body.summary' }
    });

    fireEvent.click(screen.getByRole('button', { name: '创建应用' }));

    await waitFor(() => {
      expect(publicApi.saveApplicationApiMapping).toHaveBeenCalledWith(
        'app-workflow',
        expect.objectContaining({
          extension: {
            slug: 'ticket_webhook',
            method: 'PATCH',
            response_mode: 'sync',
            parameters: [
              {
                source: 'path',
                name: 'ticket_id',
                target: 'node-workflow-start.path.ticket_id'
              },
              {
                source: 'query',
                name: 'include_history',
                target: 'node-workflow-start.query.include_history'
              },
              {
                source: 'form',
                name: 'assignee',
                target: 'node-workflow-start.form.assignee'
              },
              {
                source: 'body',
                name: 'summary',
                target: 'node-workflow-start.body.summary'
              }
            ]
          }
        }),
        'csrf-123'
      );
    });
  });
});
