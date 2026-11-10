import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const applicationsApi = vi.hoisted(() => ({
  applicationsQueryKey: ['applications'],
  applicationCatalogQueryKey: ['applications', 'catalog'],
  saveWorkflowEventTrigger: vi.fn(),
  fetchWorkflowEventCatalog: vi.fn(),
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

import { ApplicationFormDrawer } from '../components/ApplicationFormDrawer';

async function selectApplicationType(label: string) {
  fireEvent.mouseDown(await screen.findByRole('combobox', { name: '类型' }));
  const option = await screen.findByText(label, {
    selector: '.ant-select-item-option-content'
  });
  fireEvent.click(option);
}

describe('ApplicationFormDrawer create intent', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    applicationsApi.saveWorkflowEventTrigger.mockResolvedValue({});
    applicationsApi.fetchWorkflowEventCatalog.mockResolvedValue([
      {
        contract_id: 'custom.invoice',
        name: 'Invoice',
        enabled: true,
        origin: 'custom',
        owner_id: null,
        scope: { kind: 'workspace', workspace_id: 'workspace-1' },
        supported_phases: ['published', 'consumed']
      }
    ]);
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
        },
        { value: 'agent_logs', label: 'Agent Logs' }
      ],
      workflow_triggers: [
        {
          value: 'extension',
          label: '扩展接口'
        },
        {
          value: 'schedule',
          label: '定时调度'
        },
        { value: 'event', label: '事件触发' }
      ],
      tags: []
    });
    publicApi.saveApplicationApiMapping.mockResolvedValue({});
    publicApi.saveWorkflowScheduleTrigger.mockResolvedValue({});
  });

  test('AC-001 renders Application type labels without MCP guidance', async () => {
    render(
      <AppProviders>
        <ApplicationFormDrawer
          open
          csrfToken="csrf-123"
          onClose={vi.fn()}
          intent={{ kind: 'create', onCreated: vi.fn() }}
        />
      </AppProviders>
    );

    expect(await screen.findByText('新建应用')).toBeInTheDocument();
    expect(await screen.findByText('Agent Flow')).toBeInTheDocument();
    expect(screen.getByRole('combobox', { name: '类型' })).toBeInTheDocument();
    expect(screen.queryByRole('radio')).not.toBeInTheDocument();
    expect(screen.queryByText('后端 Agent Flow 描述')).not.toBeInTheDocument();
    expect(screen.queryByText('后端 Workflow 描述')).not.toBeInTheDocument();
    expect(screen.getByRole('textbox', { name: '名称' })).toBeInTheDocument();
    const submitButton = screen.getByRole('button', { name: '创建应用' });
    const drawer = screen.getByRole('dialog', { name: '新建应用' });
    const scrollBody = drawer.querySelector('.ant-drawer-body');
    expect(scrollBody).toContainElement(
      screen.getByRole('textbox', { name: '名称' })
    );
    expect(scrollBody).not.toContainElement(submitButton);
    expect(submitButton.closest('.ant-drawer-footer')).toBeInTheDocument();
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

  test('creates Agent Logs without orchestration or a workflow trigger', async () => {
    render(
      <AppProviders>
        <ApplicationFormDrawer
          open
          csrfToken="csrf-123"
          onClose={vi.fn()}
          intent={{ kind: 'create', onCreated: vi.fn() }}
        />
      </AppProviders>
    );
    await selectApplicationType('Agent Logs');
    fireEvent.change(screen.getByRole('textbox', { name: '名称' }), {
      target: { value: 'Collected logs' }
    });
    fireEvent.click(screen.getByRole('button', { name: '创建应用' }));
    await waitFor(() =>
      expect(applicationsApi.createApplication).toHaveBeenCalledWith(
        expect.objectContaining({
          application_type: 'agent_logs',
          workflow_trigger_type: null,
          name: 'Collected logs'
        }),
        'csrf-123'
      )
    );
    expect(publicApi.saveApplicationApiMapping).not.toHaveBeenCalled();
    expect(publicApi.saveWorkflowScheduleTrigger).not.toHaveBeenCalled();
  });

  test('creates extension workflows with their initial trigger configuration', async () => {
    const onCreated = vi.fn();

    render(
      <AppProviders>
        <ApplicationFormDrawer
          open
          csrfToken="csrf-123"
          onClose={vi.fn()}
          intent={{ kind: 'create', onCreated }}
        />
      </AppProviders>
    );

    await selectApplicationType('Workflow');
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
        <ApplicationFormDrawer
          open
          csrfToken="csrf-123"
          onClose={vi.fn()}
          intent={{ kind: 'create', onCreated: vi.fn() }}
        />
      </AppProviders>
    );

    await selectApplicationType('Workflow');
    const triggerTypeSelect = await screen.findByRole('combobox', {
      name: '触发方式'
    });
    fireEvent.mouseDown(triggerTypeSelect);
    fireEvent.click(
      await screen.findByText('定时调度', {
        selector: '.ant-select-item-option-content'
      })
    );
    fireEvent.mouseDown(await screen.findByRole('combobox', { name: '重复频率' }));
    fireEvent.click(await screen.findByText('每周', { selector: '.ant-select-item-option-content' }));
    const weekdays = screen.getByRole('combobox', { name: '执行星期' });
    fireEvent.mouseDown(weekdays);
    for (const day of ['星期二', '星期三', '星期四', '星期五']) {
      fireEvent.click(await screen.findByText(day, { selector: '.ant-select-item-option-content' }));
    }
    const timezone = screen.getByRole('combobox', { name: '时区' });
    fireEvent.mouseDown(timezone);
    fireEvent.change(timezone, { target: { value: 'Shanghai' } });
    fireEvent.click(await screen.findByText('Asia/Shanghai', { selector: '.ant-select-item-option-content' }));
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
            cron: '0 9 * * 1,2,3,4,5',
            timezone: 'Asia/Shanghai',
            input_payload: {}
          }
        }),
        'csrf-123'
      );
    });
  });

  test('creates a daily schedule with an empty timezone to follow the system', async () => {
    render(
      <AppProviders>
        <ApplicationFormDrawer open csrfToken="csrf-123" onClose={vi.fn()} intent={{ kind: 'create', onCreated: vi.fn() }} />
      </AppProviders>
    );
    await selectApplicationType('Workflow');
    fireEvent.mouseDown(await screen.findByRole('combobox', { name: '触发方式' }));
    fireEvent.click(await screen.findByText('定时调度', { selector: '.ant-select-item-option-content' }));
    expect((await screen.findByRole('combobox', { name: '时区' })).closest('.ant-select')).toHaveTextContent('跟随系统时区');
    fireEvent.change(screen.getByRole('textbox', { name: '名称' }), { target: { value: 'System schedule' } });
    fireEvent.click(screen.getByRole('button', { name: '创建应用' }));
    await waitFor(() => expect(applicationsApi.createApplication).toHaveBeenCalledWith(
      expect.objectContaining({ workflow_trigger_config: { cron: '0 9 * * *', timezone: '', input_payload: {} } }),
      'csrf-123'
    ));
  });

  test('E7 creates event workflow then saves subscription without extension config', async () => {
    const onCreated = vi.fn();
    render(
      <AppProviders>
        <ApplicationFormDrawer
          open
          csrfToken="csrf-123"
          onClose={vi.fn()}
          intent={{ kind: 'create', onCreated }}
        />
      </AppProviders>
    );
    await selectApplicationType('Workflow');
    fireEvent.mouseDown(
      await screen.findByRole('combobox', { name: '触发方式' })
    );
    fireEvent.click(
      await screen.findByText('事件触发', {
        selector: '.ant-select-item-option-content'
      })
    );
    fireEvent.mouseDown(
      await screen.findByRole('combobox', { name: '监听事件' })
    );
    fireEvent.click(
      await screen.findByText('Invoice', {
        selector: '.ant-select-item-option-content'
      })
    );
    fireEvent.mouseDown(
      await screen.findByRole('combobox', { name: '监听时机' })
    );
    fireEvent.click(
      await screen.findByText('消费', {
        selector: '.ant-select-item-option-content'
      })
    );
    fireEvent.change(screen.getByRole('textbox', { name: '名称' }), {
      target: { value: 'Invoice handler' }
    });
    fireEvent.click(screen.getByRole('button', { name: '创建应用' }));
    await waitFor(() =>
      expect(applicationsApi.saveWorkflowEventTrigger).toHaveBeenCalledWith(
        'app-workflow',
        expect.objectContaining({
          contract_id: 'custom.invoice',
          trigger_phase: 'consumed',
          input_mapping: {}
        }),
        'csrf-123'
      )
    );
    expect(applicationsApi.createApplication).toHaveBeenCalledWith(
      expect.objectContaining({
        workflow_trigger_type: 'event',
        workflow_trigger_config: null
      }),
      'csrf-123'
    );
    expect(
      applicationsApi.saveWorkflowEventTrigger.mock.calls[0][1]
    ).not.toHaveProperty('contract_version');
    await waitFor(() => expect(onCreated).toHaveBeenCalledWith('app-workflow'));
  });
});
