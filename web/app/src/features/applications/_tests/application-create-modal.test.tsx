import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const applicationsApi = vi.hoisted(() => ({
  applicationsQueryKey: ['applications'],
  createApplication: vi.fn()
}));

vi.mock('../api/applications', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../api/applications')>()),
  ...applicationsApi
}));

const publicApi = vi.hoisted(() => ({
  saveApplicationApiMapping: vi.fn(),
  saveWorkflowScheduleTrigger: vi.fn()
}));

vi.mock('../api/public-api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../api/public-api')>()),
  ...publicApi
}));

import { AppProviders } from '../../../app/AppProviders';

import { ApplicationFormModal } from '../components/ApplicationFormModal';

describe('ApplicationFormModal create intent', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    applicationsApi.createApplication.mockResolvedValue({ id: 'app-workflow' });
    publicApi.saveApplicationApiMapping.mockResolvedValue({});
    publicApi.saveWorkflowScheduleTrigger.mockResolvedValue({});
  });

  test('keeps form semantics after migrating to the shared modal shell', () => {
    render(
      <AppProviders>
        <ApplicationFormModal
          open
          csrfToken="csrf-123"
          onClose={vi.fn()}
          intent={{ kind: 'create', onCreated: vi.fn() }}
        />
      </AppProviders>
    );

    expect(screen.getByText('新建应用')).toBeInTheDocument();
    expect(screen.getByRole('textbox', { name: '名称' })).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: '创建应用' })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('textbox', { name: '图标' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('textbox', { name: '图标类型' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('textbox', { name: '图标背景' })
    ).not.toBeInTheDocument();
  }, 10_000);

  test('creates extension workflows with their initial trigger configuration', async () => {
    const onCreated = vi.fn();

    render(
      <AppProviders>
        <ApplicationFormModal
          open
          csrfToken="csrf-123"
          onClose={vi.fn()}
          intent={{ kind: 'create', onCreated }}
        />
      </AppProviders>
    );

    fireEvent.click(screen.getByRole('radio', { name: /Workflow/i }));
    expect(screen.getByText('/api/ex/')).toBeInTheDocument();
    fireEvent.change(screen.getByRole('textbox', { name: '接口子路径' }), {
      target: { value: 'orders/create' }
    });
    fireEvent.change(screen.getByRole('textbox', { name: '名称' }), {
      target: { value: 'Order workflow' }
    });
    fireEvent.click(screen.getByRole('button', { name: '创建应用' }));

    await waitFor(() => {
      expect(applicationsApi.createApplication).toHaveBeenCalledWith(
        expect.objectContaining({
          application_type: 'workflow',
          workflow_trigger_type: 'extension',
          workflow_trigger_config: {
            subpath: 'orders/create',
            http_method: 'POST',
            response_mode: 'sync'
          },
          name: 'Order workflow'
        }),
        'csrf-123'
      );
    });
    await waitFor(() => {
      expect(onCreated).toHaveBeenCalledWith('app-workflow');
    });
  });

  test('creates schedule workflows disabled with cron configuration', async () => {
    render(
      <AppProviders>
        <ApplicationFormModal
          open
          csrfToken="csrf-123"
          onClose={vi.fn()}
          intent={{ kind: 'create', onCreated: vi.fn() }}
        />
      </AppProviders>
    );

    fireEvent.click(screen.getByRole('radio', { name: /Workflow/i }));
    const triggerTypeSelect = screen.getByRole('combobox', {
      name: /workflow_trigger_type/
    });
    fireEvent.mouseDown(triggerTypeSelect);
    const triggerOptions = document.querySelectorAll<HTMLElement>(
      '.ant-select-item-option'
    );
    expect(triggerOptions).toHaveLength(2);
    fireEvent.click(triggerOptions[1]);
    const cronInput = await screen.findByRole('textbox', {
      name: 'Cron 表达式'
    });
    fireEvent.change(cronInput, {
      target: { value: '0 9 * * 1-5' }
    });
    fireEvent.change(screen.getByRole('textbox', { name: '时区' }), {
      target: { value: 'Asia/Shanghai' }
    });
    fireEvent.change(screen.getByRole('textbox', { name: '名称' }), {
      target: { value: 'Daily schedule' }
    });
    fireEvent.click(screen.getByRole('button', { name: '创建应用' }));

    await waitFor(() => {
      expect(applicationsApi.createApplication).toHaveBeenCalledWith(
        expect.objectContaining({
          application_type: 'workflow',
          workflow_trigger_type: 'schedule',
          workflow_trigger_config: {
            cron: '0 9 * * 1-5',
            timezone: 'Asia/Shanghai',
            input_payload: {}
          }
        }),
        'csrf-123'
      );
    });
  });
});
