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

test.each(['disabled', 'missing', 'unsupported phase', 'interface capability'])(
  'an unavailable %s source cannot be bypassed by a legacy enabled value',
  async (reason) => {
    const capability = reason === 'interface capability';
    const target = capability
      ? {
          ...stored,
          contract_id: 'interface.orders.create.before',
          trigger_phase: 'published'
        }
      : stored;
    api.fetchWorkflowEventCatalog.mockResolvedValue(
      reason === 'missing'
        ? []
        : [
            {
              contract_id: target.contract_id,
              name: capability ? 'Interface capability' : 'Invoice',
              origin: capability ? 'core' : 'custom',
              enabled: capability ? null : reason !== 'disabled',
              ...(capability
                ? {
                    interface_event: {
                      interface_id: 'orders.create',
                      phase: 'before'
                    },
                    payload_schema: { type: 'object' }
                  }
                : {}),
              supported_phases:
                capability || reason === 'unsupported phase'
                  ? ['published']
                  : ['consumed']
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
          initialValues={{ event: target }}
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
    if (capability) await screen.findAllByText(/Interface capability/);
    expect(screen.queryByRole('switch')).not.toBeInTheDocument();
    // Provide the newly required input so activation fails on source availability.
    fireEvent.change(
      screen.getByRole('combobox', { name: 'New input · string' }),
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

    expect(saved).not.toHaveBeenCalled();
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
  fireEvent.mouseDown(screen.getByRole('combobox', { name: '监听事件' }));
  fireEvent.click(await screen.findByText(/System source/));
  const selects = screen.getAllByRole('combobox');
  fireEvent.mouseDown(selects[1]);
  // Only the source adapter's published phase is offered.
  expect(screen.queryByText('消费')).not.toBeInTheDocument();
  fireEvent.click(
    await screen.findByText('发布', {
      selector: '.ant-select-item-option-content'
    })
  );
  fireEvent.click(screen.getByRole('button', { name: 'Save source' }));
  await waitFor(() =>
    expect(saved).toHaveBeenCalledWith({
      event: {
        contract_id: 'runtime.event',
        source: system,
        trigger_phase: 'published',
        input_mapping: {}
      }
    })
  );
});

test('an unavailable stored system registration keeps its identity and cannot create a listener', async () => {
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
  expect(screen.queryByRole('switch')).not.toBeInTheDocument();
  expect(saved).not.toHaveBeenCalled();
});

test.each([
  { label: 'empty', input_mapping: {} },
  { label: 'historical', input_mapping: stored.input_mapping }
])(
  'the application modal rejects an unavailable source without erasing $label mapping',
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
    await waitFor(() =>
      expect(api.fetchWorkflowEventTrigger).toHaveBeenCalled()
    );
    expect(screen.queryByRole('switch')).not.toBeInTheDocument();
    const save = screen.getByRole('button', { name: '保存修改' });
    await waitFor(() => expect(save).toBeEnabled());
    fireEvent.click(save);
    await screen.findAllByText('请选择可用的监听事件及其支持的阶段。');
    expect(api.saveWorkflowEventTrigger).not.toHaveBeenCalled();

    expect(api.saveWorkflowEventTrigger).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
  }
);

test('same-contract subtype leaves save complete backend filters and discover escaped payload fields', async () => {
  const source = { source_id: 'business.facts', scope: { kind: 'system' } };
  const catalog = ['reserved', 'settled'].map((kind) => ({
    id: null,
    enabled: null,
    owner_id: 'billing',
    origin: 'plugin',
    contract_id: 'business.fact',
    name: kind,
    description: '',
    group_path: ['billing', 'credits'],
    scope: source.scope,
    source: {
      point_id: 'business.facts',
      selection: { ...source, filters: { kind, version: 'v1' } }
    },
    supported_phases: ['published'],
    payload_schema: {
      type: 'object',
      properties: { 'amount/total~value': { type: 'number' } }
    }
  }));
  api.fetchWorkflowEventCatalog.mockResolvedValue(catalog);
  const saved = vi.fn();
  function Editor() {
    const [form] = Form.useForm();
    return (
      <Form
        form={form}
        initialValues={{ event: { enabled: true, input_mapping: {} } }}
        onFinish={saved}
      >
        <WorkflowEventTriggerFields
          form={form}
          fields={[
            {
              key: 'amount',
              label: 'Amount',
              valueType: 'number',
              required: true
            }
          ]}
        />
        <Button htmlType="submit">Save subtype</Button>
      </Form>
    );
  }
  render(
    <AppProviders>
      <Editor />
    </AppProviders>
  );
  await waitFor(() => expect(api.fetchWorkflowEventCatalog).toHaveBeenCalled());
  const sourceSelect = screen.getByRole('combobox', { name: '监听事件' });
  fireEvent.mouseDown(sourceSelect);
  expect(
    await screen.findByText(/billing \/ credits \/ reserved/)
  ).toBeInTheDocument();
  fireEvent.click(await screen.findByText(/billing \/ credits \/ settled/));
  const phaseSelect = screen.getByRole('combobox', { name: '监听时机' });
  fireEvent.mouseDown(phaseSelect);
  fireEvent.click(
    await screen.findByText('发布', {
      selector: '.ant-select-item-option-content'
    })
  );
  const mapping = screen.getByRole('combobox', { name: 'Amount · number' });
  fireEvent.focus(mapping);
  fireEvent.change(mapping, { target: { value: '/payload/amount' } });
  fireEvent.click(
    await screen.findByText('/payload/amount~1total~0value', {
      selector: '.ant-select-item-option-content'
    })
  );
  fireEvent.click(screen.getByRole('button', { name: 'Save subtype' }));
  await waitFor(() =>
    expect(saved).toHaveBeenCalledWith({
      event: {
        contract_id: 'business.fact',
        source: catalog[1].source.selection,
        trigger_phase: 'published',
        input_mapping: { amount: '/payload/amount~1total~0value' }
      }
    })
  );
});

test('new listeners default to publication and retain every supported phase without advanced controls', async () => {
  api.fetchWorkflowEventCatalog.mockResolvedValue([
    {
      contract_id: 'custom.invoice',
      name: 'Invoice',
      origin: 'custom',
      enabled: true,
      supported_phases: ['subscribed', 'published', 'consumed'],
      payload_schema: { type: 'object' }
    }
  ]);
  const saved = vi.fn();
  function Editor() {
    const [form] = Form.useForm();
    return (
      <Form
        form={form}
        initialValues={{ event: { input_mapping: {} } }}
        onFinish={saved}
      >
        <WorkflowEventTriggerFields form={form} fields={[]} />
        <Button htmlType="submit">Save listener</Button>
      </Form>
    );
  }
  render(
    <AppProviders>
      <Editor />
    </AppProviders>
  );
  fireEvent.mouseDown(screen.getByRole('combobox', { name: '监听事件' }));
  fireEvent.click(
    await screen.findByText('Invoice', {
      selector: '.ant-select-item-option-content'
    })
  );
  expect(screen.queryByRole('switch')).not.toBeInTheDocument();
  expect(screen.queryByText('高级设置')).not.toBeInTheDocument();
  await waitFor(() =>
    expect(
      screen.getByRole('combobox', { name: '监听时机' }).closest('.ant-select')
    ).toHaveTextContent('发布')
  );
  fireEvent.click(screen.getByRole('button', { name: 'Save listener' }));
  await waitFor(() => expect(saved).toHaveBeenCalled());
  expect(saved.mock.calls[0][0].event).toMatchObject({
    contract_id: 'custom.invoice',
    trigger_phase: 'published',
    input_mapping: {}
  });
  expect(saved.mock.calls[0][0].event).not.toHaveProperty('enabled');
  fireEvent.mouseDown(screen.getByRole('combobox', { name: '监听时机' }));
  for (const label of ['订阅', '发布', '消费']) {
    expect(
      screen.getByText(label, { selector: '.ant-select-item-option-content' })
    ).toBeInTheDocument();
  }
  fireEvent.click(
    screen.getByText('消费', {
      selector: '.ant-select-item-option-content'
    })
  );
  fireEvent.click(screen.getByRole('button', { name: 'Save listener' }));
  await waitFor(() =>
    expect(saved.mock.calls.at(-1)?.[0].event.trigger_phase).toBe('consumed')
  );
});

test('editing preserves a consumed phase even with a legacy disabled configuration flag', async () => {
  api.fetchWorkflowEventCatalog.mockResolvedValue([
    {
      contract_id: stored.contract_id,
      name: 'Invoice',
      origin: 'custom',
      enabled: true,
      supported_phases: ['subscribed', 'published', 'consumed'],
      payload_schema: { type: 'object' }
    }
  ]);
  const saved = vi.fn();
  function Editor() {
    const [form] = Form.useForm();
    return (
      <Form
        form={form}
        initialValues={{ event: { ...stored, enabled: false } }}
        onFinish={saved}
      >
        <WorkflowEventTriggerFields form={form} fields={[]} />
        <Button htmlType="submit">Save existing listener</Button>
      </Form>
    );
  }
  render(
    <AppProviders>
      <Editor />
    </AppProviders>
  );
  await screen.findByText('Invoice', { exact: true });
  fireEvent.click(
    screen.getByRole('button', { name: 'Save existing listener' })
  );
  await waitFor(() => expect(saved).toHaveBeenCalled());
  expect(saved.mock.calls[0][0].event).toMatchObject({
    contract_id: stored.contract_id,
    trigger_phase: 'consumed',
    input_mapping: stored.input_mapping
  });
});
