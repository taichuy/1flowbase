import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { Button, Form } from 'antd';
import { beforeEach, expect, test, vi } from 'vitest';
import { AppProviders } from '../../../../app/AppProviders';
import { WorkflowEventTriggerFields } from '../WorkflowEventTriggerFields';
import { ApplicationFormModal } from '../ApplicationFormModal';

const api = vi.hoisted(() => ({
  fetchWorkflowEventCatalog: vi.fn(),
  fetchWorkflowEventTrigger: vi.fn(),
  fetchApplicationCatalog: vi.fn(),
  fetchApplicationDetail: vi.fn(),
  updateApplication: vi.fn(),
  saveWorkflowEventTrigger: vi.fn()
}));
const orchestration = vi.hoisted(() => ({ fetchOrchestrationState: vi.fn() }));
vi.mock('../../../agent-flow/api/orchestration', async (original) => ({
  ...(await original<typeof import('../../../agent-flow/api/orchestration')>()),
  ...orchestration
}));
vi.mock('../../api/applications', async (original) => ({
  ...(await original<typeof import('../../api/applications')>()),
  ...api
}));

const stored = {
  enabled: true,
  contract_id: 'custom.invoice',
  trigger_phase: 'consumed',
  input_mapping: { invoice: '/payload/invoice' }
};

beforeEach(() => vi.clearAllMocks());

test.each(['disabled', 'missing', 'unsupported phase'])(
  'an unavailable %s source can be stopped without changing the stored target',
  async (reason) => {
    api.fetchWorkflowEventCatalog.mockResolvedValue(
      reason === 'missing'
        ? []
        : [
            {
              contract_id: stored.contract_id,
              name: 'Invoice',
              origin: 'custom',
              enabled: reason !== 'disabled',
              supported_phases:
                reason === 'unsupported phase' ? ['published'] : ['consumed']
            }
          ]
    );
    const saved = vi.fn();
    const rejected = vi.fn();
    function Editor() {
      const [form] = Form.useForm();
      return (
        <Form
          form={form}
          initialValues={{ event: stored }}
          onFinish={saved}
          onFinishFailed={rejected}
        >
          <WorkflowEventTriggerFields
            form={form}
            fields={[
              {
                key: 'invoice',
                label: 'Invoice input',
                valueType: 'string',
                required: true
              },
              {
                key: 'new_input',
                label: 'New input',
                valueType: 'string',
                required: true
              }
            ]}
          />
          <Button htmlType="submit">Save subscription</Button>
        </Form>
      );
    }
    render(
      <AppProviders>
        <Editor />
      </AppProviders>
    );
    await waitFor(() =>
      expect(api.fetchWorkflowEventCatalog).toHaveBeenCalled()
    );
    const enabled = screen.getByRole('switch', { name: '启用订阅' });
    expect(enabled).toBeChecked();
    // Provide the newly required input so activation fails on source availability.
    fireEvent.change(
      screen.getByRole('textbox', { name: 'New input · string' }),
      {
        target: { value: '/payload/new_input' }
      }
    );
    fireEvent.click(screen.getByRole('button', { name: 'Save subscription' }));
    await waitFor(() => expect(rejected).toHaveBeenCalled());
    expect(saved).not.toHaveBeenCalled();
    expect(
      rejected.mock.calls[0][0].errorFields.some(
        ({ name }: { name: string[] }) =>
          name.join('.') ===
          (reason === 'unsupported phase'
            ? 'event.trigger_phase'
            : 'event.contract_id')
      )
    ).toBe(true);

    fireEvent.change(
      screen.getByRole('textbox', { name: 'New input · string' }),
      {
        target: { value: '' }
      }
    );
    fireEvent.click(enabled);
    await waitFor(() => expect(enabled).not.toBeChecked());
    fireEvent.click(screen.getByRole('button', { name: 'Save subscription' }));
    await waitFor(() =>
      expect(saved).toHaveBeenCalledWith({
        event: {
          ...stored,
          enabled: false,
          input_mapping: { ...stored.input_mapping, new_input: '' }
        }
      })
    );
  }
);

test('same-contract source scopes remain distinct and save canonical source with only advertised phases', async () => {
  const common = {
    id: null,
    owner_id: 'runtime',
    enabled: null,
    origin: 'core',
    contract_id: 'runtime.event',
    description: '',
    payload_schema: null,
    supported_phases: ['published']
  };
  const system = { source_id: 'runtime.required', scope: { kind: 'system' } };
  api.fetchWorkflowEventCatalog.mockResolvedValue([
    {
      ...common,
      name: 'Workspace source',
      scope: { kind: 'workspace', workspace_id: 'workspace-a' },
      supported_phases: ['consumed'],
      source: {
        selection: {
          source_id: 'runtime.required',
          scope: { kind: 'workspace', workspace_id: 'workspace-a' }
        },
        point_id: 'runtime.required'
      }
    },
    {
      ...common,
      name: 'System source',
      scope: { kind: 'system' },
      source: {
        selection: system,
        point_id: 'runtime.required'
      }
    }
  ]);
  const saved = vi.fn();
  function Editor() {
    const [form] = Form.useForm();
    return (
      <Form
        form={form}
        initialValues={{ event: { enabled: true, input_mapping: {} } }}
        onFinish={saved}
      >
        <WorkflowEventTriggerFields form={form} fields={[]} />
        <Button htmlType="submit">Save source</Button>
      </Form>
    );
  }
  render(
    <AppProviders>
      <Editor />
    </AppProviders>
  );
  await waitFor(() => expect(api.fetchWorkflowEventCatalog).toHaveBeenCalled());
  fireEvent.mouseDown(screen.getByRole('combobox', { name: '事件来源' }));
  fireEvent.click(await screen.findByText(/System source/));
  const selects = screen.getAllByRole('combobox');
  fireEvent.mouseDown(selects[1]);
  // Only the source adapter's published phase is offered.
  expect(screen.queryByText('消费时（出队）')).not.toBeInTheDocument();
  fireEvent.click(await screen.findByText('发布时'));
  fireEvent.click(screen.getByRole('button', { name: 'Save source' }));
  await waitFor(() =>
    expect(saved).toHaveBeenCalledWith({
      event: {
        enabled: true,
        contract_id: 'runtime.event',
        source: system,
        trigger_phase: 'published',
        input_mapping: {}
      }
    })
  );
});

test('an unavailable stored system registration is explained and can be disabled without losing its source identity', async () => {
  const source = { source_id: 'native.source', scope: { kind: 'system' } };
  api.fetchWorkflowEventCatalog.mockResolvedValue([
    {
      contract_id: stored.contract_id,
      name: 'Native source',
      enabled: null,
      origin: 'core',
      scope: { kind: 'system' },
      supported_phases: [],
      source: {
        selection: source,
        point_id: 'native.point'
      }
    }
  ]);
  const saved = vi.fn();
  const rejected = vi.fn();
  function Editor() {
    const [form] = Form.useForm();
    return (
      <Form
        form={form}
        initialValues={{ event: { ...stored, source } }}
        onFinish={saved}
        onFinishFailed={rejected}
      >
        <WorkflowEventTriggerFields form={form} fields={[]} />
        <Button htmlType="submit">Save system source</Button>
      </Form>
    );
  }
  render(
    <AppProviders>
      <Editor />
    </AppProviders>
  );
  await screen.findByText(/Native source/);
  fireEvent.click(screen.getByRole('button', { name: 'Save system source' }));
  await waitFor(() => expect(rejected).toHaveBeenCalled());
  fireEvent.click(screen.getByRole('switch', { name: '启用订阅' }));
  fireEvent.click(screen.getByRole('button', { name: 'Save system source' }));
  await waitFor(() =>
    expect(saved).toHaveBeenCalledWith({
      event: { ...stored, source, enabled: false }
    })
  );
});

test.each([
  { label: 'empty', input_mapping: {} },
  { label: 'historical', input_mapping: stored.input_mapping }
])(
  'the application modal preserves $label mapping when disabling an unavailable source without Start inputs',
  async ({ input_mapping }) => {
    const source = { source_id: 'native.source', scope: { kind: 'system' } };
    const event = { ...stored, source, input_mapping };
    api.fetchWorkflowEventCatalog.mockResolvedValue([]);
    api.fetchWorkflowEventTrigger.mockResolvedValue(event);
    api.fetchApplicationDetail.mockResolvedValue({
      id: 'event-application',
      application_type: 'workflow',
      workflow_trigger_type: 'event',
      name: 'Invoice workflow',
      description: '',
      tags: []
    });
    api.fetchApplicationCatalog.mockResolvedValue({
      types: [{ value: 'workflow', label: 'Workflow' }],
      workflow_triggers: [{ value: 'event', label: 'Event' }],
      tags: []
    });
    orchestration.fetchOrchestrationState.mockResolvedValue({
      draft: { document: { graph: { nodes: [] } } }
    });
    api.updateApplication.mockResolvedValue({ id: 'event-application' });
    api.saveWorkflowEventTrigger.mockResolvedValue({
      ...event,
      enabled: false
    });
    const onClose = vi.fn();
    render(
      <AppProviders>
        <ApplicationFormModal
          open
          csrfToken="test-csrf"
          intent={{ kind: 'edit', applicationId: 'event-application' }}
          onClose={onClose}
        />
      </AppProviders>
    );
    const enabled = await screen.findByRole('switch', { name: '启用订阅' });
    await waitFor(() => expect(enabled).toBeChecked());
    const save = screen.getByRole('button', { name: '保存修改' });
    await waitFor(() => expect(save).toBeEnabled());
    fireEvent.click(save);
    await screen.findAllByText('请选择可用的事件来源及其支持的阶段。');
    expect(api.saveWorkflowEventTrigger).not.toHaveBeenCalled();

    fireEvent.click(enabled);
    await waitFor(() => expect(enabled).not.toBeChecked());
    fireEvent.click(save);
    await waitFor(() =>
      expect(api.saveWorkflowEventTrigger).toHaveBeenCalledWith(
        'event-application',
        { ...event, enabled: false },
        'test-csrf'
      )
    );
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  }
);
