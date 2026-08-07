import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const applicationsApi = vi.hoisted(() => ({
  applicationsQueryKey: ['applications'],
  applicationCatalogQueryKey: ['applications', 'catalog'],
  createApplication: vi.fn(),
  fetchApplicationCatalog: vi.fn()
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
    applicationsApi.fetchApplicationCatalog.mockResolvedValue({
      types: [
        {
          value: 'agent_flow',
          label: 'Agent Flow'
        },
        {
          value: 'workflow',
          label: 'Workflow'
        }
      ],
      workflow_triggers: [
        {
          value: 'extension',
          label: '扩展接口'
        },
        {
          value: 'schedule',
          label: '定时调度'
        }
      ],
      tags: []
    });
    publicApi.saveApplicationApiMapping.mockResolvedValue({});
    publicApi.saveWorkflowScheduleTrigger.mockResolvedValue({});
  });

  test('AC-001 renders Application type labels without MCP guidance', async () => {
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

    expect(await screen.findByText('新建应用')).toBeInTheDocument();
    expect(await screen.findByText('Agent Flow')).toBeInTheDocument();
    expect(screen.getByText('Workflow')).toBeInTheDocument();
    expect(screen.queryByText('后端 Agent Flow 描述')).not.toBeInTheDocument();
    expect(screen.queryByText('后端 Workflow 描述')).not.toBeInTheDocument();
    expect(screen.getByRole('textbox', { name: '名称' })).toBeInTheDocument();
    const submitButton = screen.getByRole('button', { name: '创建应用' });
    const scrollBody = screen.getByTestId('fixed-height-modal-scroll-body');
    expect(scrollBody).toContainElement(
      screen.getByRole('textbox', { name: '名称' })
    );
    expect(scrollBody).not.toContainElement(submitButton);
    expect(submitButton.closest('.ant-modal-footer')).toBeInTheDocument();
    expect(submitButton).toHaveAttribute('form', 'application-form');
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

    fireEvent.click(await screen.findByRole('radio', { name: /Workflow/i }));
    expect(screen.queryByText('后端扩展触发描述')).not.toBeInTheDocument();
    expect(
      await screen.findByRole('textbox', { name: '/api/ex/' })
    ).toHaveValue('/api/ex/');
    expect(screen.queryByText('访问策略')).not.toBeInTheDocument();
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

    fireEvent.click(await screen.findByRole('radio', { name: /Workflow/i }));
    const triggerTypeSelect = await screen.findByRole('combobox', {
      name: '触发方式'
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
