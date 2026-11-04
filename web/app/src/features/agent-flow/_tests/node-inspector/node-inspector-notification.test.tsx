import { act, fireEvent, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';
import type {
  NotificationInstance,
  NotificationType
} from '@1flowbase/api-client';
import {
  listNotificationInstances,
  listNotificationTypes
} from '@1flowbase/api-client';
import type { FlowBinding } from '@1flowbase/flow-schema';
import { appI18n } from '../../../../shared/i18n/app-i18n';
import { NodeConfigTab } from '../../components/detail/tabs/NodeConfigTab';
import { createNodeDocument } from '../../lib/document/node-factory';
import { createEdgeDocument } from '../../lib/document/edge-factory';
import { listVisibleSelectorOptions } from '../../lib/selector-options';
import { AgentFlowEditorStoreProvider } from './fixtures/analysis-provider';
import {
  DocumentObserver,
  SelectionSeed,
  createInitialState,
  openSelect,
  renderWithProviders,
  selectOption,
  setupNodeInspectorTest
} from './support';

vi.mock('@1flowbase/api-client', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@1flowbase/api-client')>()),
  listNotificationInstances: vi.fn(),
  listNotificationTypes: vi.fn()
}));
// Keep the platform editor primitive deterministic; the real template field
// still owns mode selection, debounced commit and the named expression wire.
vi.mock(
  '../../components/bindings/template-editor/LexicalTemplatedTextEditor',
  () => ({
    LexicalTemplatedTextEditor: ({
      ariaLabel,
      value,
      onChange
    }: {
      ariaLabel: string;
      value: string;
      onChange: (value: string) => void;
    }) => (
      <textarea
        aria-label={ariaLabel}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
    )
  })
);
const systemType: NotificationType = {
  type_code: 'system',
  display_name: '系统通知',
  description: null,
  contribution_id: 'builtin.system',
  installation_id: null,
  available: true,
  unavailable_reason: null,
  config_schema: { type: 'object' },
  config_form: { schema_version: '1.0.0', fields: [] },
  input_schema: {
    type: 'object',
    required: ['user_ids', 'title', 'content'],
    properties: {
      user_ids: { type: 'array', items: { type: 'string' } },
      title: { type: 'string' },
      content: { type: 'string' }
    }
  },
  input_form: {
    schema_version: '1.0.0',
    fields: [
      {
        key: 'user_ids',
        label: '收件人',
        type: 'select',
        control: 'multi-select',
        required: true,
        options: [
          { label: 'Alice', value: 'user-alice' },
          { label: 'Bob', value: 'user-bob' }
        ],
        visible_when: [],
        disabled_when: []
      },
      {
        key: 'title',
        label: '消息标题',
        type: 'string',
        required: true,
        options: [],
        visible_when: [],
        disabled_when: []
      },
      {
        key: 'content',
        label: '消息内容',
        type: 'string',
        control: 'textarea',
        required: true,
        options: [],
        visible_when: [],
        disabled_when: []
      }
    ]
  },
  output_schema: {
    type: 'object',
    properties: {
      notification_ids: { type: 'array', items: { type: 'string' } },
      recipient_count: { type: 'integer' }
    }
  }
};
const pluginType: NotificationType = {
  ...systemType,
  type_code: 'fixture',
  display_name: 'Fixture notification',
  installation_id: 'plugin-installation',
  input_schema: {
    type: 'object',
    properties: { amount: { type: 'number' }, enabled: { type: 'boolean' } }
  },
  input_form: {
    schema_version: '1.0.0',
    fields: [
      {
        key: 'amount',
        label: 'Amount',
        type: 'number',
        default_value: 1,
        options: [],
        visible_when: [],
        disabled_when: []
      },
      {
        key: 'enabled',
        label: 'Enabled',
        type: 'boolean',
        default_value: true,
        options: [],
        visible_when: [],
        disabled_when: []
      }
    ]
  },
  output_schema: {
    type: 'object',
    properties: {
      delivery: { type: 'object', properties: { id: { type: 'string' } } }
    }
  }
};
const systemInstance: NotificationInstance = {
  id: 'instance-system',
  workspace_id: 'workspace',
  type_code: 'system',
  installation_id: null,
  display_name: '团队站内通知',
  status: 'enabled',
  authorization: null,
  config_json: {},
  configured_secret_fields: [],
  available: true,
  unavailable_reason: null,
  created_at: '2026-10-13T00:00:00Z',
  updated_at: '2026-10-13T00:00:00Z'
};
const pluginInstance: NotificationInstance = {
  ...systemInstance,
  id: 'instance-plugin',
  type_code: 'fixture',
  installation_id: 'plugin-installation',
  display_name: 'External fixture',
  config_json: { non_secret: 'instance-only' }
};
let latestDocument: ReturnType<typeof createInitialState>['draft']['document'];
function node() {
  return latestDocument.graph.nodes.find(
    (item) => item.id === 'node-notification'
  )!;
}
function input() {
  const value = node().bindings.input;
  return value?.kind === 'named_bindings' ? value.value : [];
}
async function renderNotification(
  config: Record<string, unknown> = {},
  bindings: Record<string, FlowBinding> = {}
) {
  const state = createInitialState();
  const notification = createNodeDocument('notification', 'node-notification');
  notification.config = { ...notification.config, ...config };
  notification.bindings = bindings;
  state.draft.document.graph.nodes.push(notification);
  state.draft.document.graph.edges.push(
    createEdgeDocument({
      id: 'edge-start-notification',
      source: 'node-start',
      target: 'node-notification'
    })
  );
  state.draft.document.graph.edges.push(
    createEdgeDocument({
      id: 'edge-notification-llm',
      source: 'node-notification',
      target: 'node-llm'
    })
  );
  latestDocument = state.draft.document;
  return renderWithProviders(
    <AgentFlowEditorStoreProvider initialState={state}>
      <SelectionSeed nodeId="node-notification" />
      <DocumentObserver
        onChange={(document) => {
          latestDocument = document;
        }}
      />
      <NodeConfigTab />
    </AgentFlowEditorStoreProvider>
  );
}
beforeEach(async () => {
  await setupNodeInspectorTest();
  vi.mocked(listNotificationInstances).mockResolvedValue([
    systemInstance,
    pluginInstance
  ]);
  vi.mocked(listNotificationTypes).mockResolvedValue([systemType, pluginType]);
});

describe('notification inspector AC-002/005', () => {
  test('switches plugin field text without changing existing notification bindings or defaults', async () => {
    vi.mocked(listNotificationTypes).mockResolvedValue([
      {
        ...pluginType,
        i18n: {
          zh_Hans: {
            description: '插件通知说明',
            input_form: {
              fields: {
                amount: { label: '金额' },
                enabled: { label: '启用通知' }
              }
            }
          },
          en_US: {
            description: 'Plugin notification help',
            input_form: {
              fields: {
                amount: { label: 'Amount' },
                enabled: { label: 'Enabled' }
              }
            }
          }
        }
      }
    ]);
    await renderNotification(
      { instance_id: pluginInstance.id },
      {
        input: {
          kind: 'named_bindings',
          value: [
            {
              name: 'amount',
              valueType: 'number',
              value: { kind: 'constant', value: 7 }
            }
          ]
        }
      }
    );
    await screen.findByRole('spinbutton', { name: '金额' });
    const before = structuredClone(node().bindings);
    try {
      await act(async () => {
        await appI18n.changeLanguage('en_US');
      });
      expect(screen.getByRole('spinbutton', { name: 'Amount' })).toHaveValue(
        '7'
      );
      expect(screen.getByText('Plugin notification help')).toBeInTheDocument();
      expect(node().bindings).toEqual(before);
      expect(pluginType.input_form.fields[0].default_value).toBe(1);
    } finally {
      await act(async () => {
        await appI18n.changeLanguage('zh_Hans');
      });
    }
  });
  test('selects a persistent instance, writes recipient UUID array and input fields as named expressions', async () => {
    await renderNotification();
    await openSelect('通知实例');
    await selectOption('团队站内通知');
    await screen.findByRole('combobox', { name: '收件人' });
    await openSelect('收件人');
    await selectOption('Alice');
    await selectOption('Bob');
    fireEvent.change(screen.getByRole('textbox', { name: '消息标题' }), {
      target: { value: '运行完成' }
    });
    fireEvent.change(screen.getByRole('textbox', { name: '消息内容' }), {
      target: { value: '请检查结果' }
    });
    await waitFor(() =>
      expect(input()).toEqual(
        expect.arrayContaining([
          {
            name: 'user_ids',
            valueType: 'array',
            value: { kind: 'constant', value: ['user-alice', 'user-bob'] }
          },
          {
            name: 'title',
            valueType: 'string',
            value: { kind: 'constant', value: '运行完成' }
          },
          {
            name: 'content',
            valueType: 'string',
            value: { kind: 'constant', value: '请检查结果' }
          }
        ])
      )
    );
    expect(node().config).toEqual({ instance_id: 'instance-system' });
    expect(node().outputs[0]).toMatchObject({
      key: 'result',
      valueType: 'object',
      selector: ['result'],
      jsonSchema: systemType.output_schema
    });
    const selectors = listVisibleSelectorOptions(latestDocument, 'node-llm');
    expect(selectors).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          value: ['node-notification', 'result', 'recipient_count'],
          valueType: 'number'
        })
      ])
    );
  });
  test('switches type, removes prior inputs and instance configuration, updates output schema and keeps booleans and numbers typed', async () => {
    await renderNotification(
      { instance_id: 'instance-system' },
      {
        input: {
          kind: 'named_bindings',
          value: [
            {
              name: 'title',
              valueType: 'string',
              value: { kind: 'constant', value: 'obsolete' }
            }
          ]
        }
      }
    );
    await openSelect('通知实例');
    await selectOption('External fixture');
    await screen.findByRole('spinbutton', { name: 'Amount' });
    expect(input().map((entry) => entry.name)).toEqual(['amount', 'enabled']);
    expect(node().config).toEqual({ instance_id: 'instance-plugin' });
    expect(JSON.stringify(node())).not.toContain('instance-only');
    fireEvent.change(screen.getByRole('spinbutton', { name: 'Amount' }), {
      target: { value: '5' }
    });
    fireEvent.click(screen.getByRole('switch', { name: 'Enabled' }));
    await waitFor(() =>
      expect(input()).toEqual(
        expect.arrayContaining([
          {
            name: 'amount',
            valueType: 'number',
            value: { kind: 'constant', value: 5 }
          },
          {
            name: 'enabled',
            valueType: 'boolean',
            value: { kind: 'constant', value: false }
          }
        ])
      )
    );
    expect(node().outputs[0].jsonSchema).toEqual(pluginType.output_schema);
    expect(listVisibleSelectorOptions(latestDocument, 'node-llm')).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          value: ['node-notification', 'result', 'delivery', 'id']
        })
      ])
    );
    expect(
      listVisibleSelectorOptions(latestDocument, 'node-llm').some((option) =>
        option.value.includes('recipient_count')
      )
    ).toBe(false);
  });
  test('retains invalid instance identity and shows a formal missing/disabled state', async () => {
    const view = await renderNotification({ instance_id: 'removed-instance' });
    await screen.findByText('通知实例不存在，请重新选择。');
    expect(node().config.instance_id).toBe('removed-instance');
    view.unmount();
    vi.mocked(listNotificationInstances).mockResolvedValue([
      {
        ...systemInstance,
        status: 'disabled',
        available: false,
        unavailable_reason: 'instance_disabled'
      }
    ]);
    await renderNotification({ instance_id: systemInstance.id });
    await screen.findByText('通知实例已停用', { exact: true });
    expect(node().config.instance_id).toBe(systemInstance.id);
    expect(node().bindings).toEqual({});
  });
  test('the real input editor switches to upstream selector binding without flattening its value', async () => {
    await renderNotification({ instance_id: 'instance-system' });
    await screen.findByRole('textbox', { name: '消息标题' });
    await openSelect('消息标题输入方式');
    await selectOption('引用变量');
    await waitFor(() =>
      expect(input().find((entry) => entry.name === 'title')?.value).toEqual({
        kind: 'selector',
        selector: []
      })
    );
    // An empty canonical selector is an editable authoring draft, so the
    // chosen mode must remain visible until the user picks its actual path.
    const selector = await screen.findByRole('combobox', { name: '消息标题' });
    expect(selector.closest('.ant-cascader')).not.toBeNull();
    expect(
      screen
        .getByRole('combobox', { name: '消息标题输入方式' })
        .closest('.ant-select')
    ).toHaveTextContent('引用变量');
    fireEvent.mouseDown(selector);
    fireEvent.click(await screen.findByText('系统变量'));
    fireEvent.click(await screen.findByText('sys.conversation_id'));
    await waitFor(() =>
      expect(input().find((entry) => entry.name === 'title')?.value).toEqual({
        kind: 'selector',
        selector: ['sys', 'conversation_id']
      })
    );
  });
  test('commits numeric template expressions and edits the actual result schema for downstream selectors', async () => {
    await renderNotification();
    await openSelect('通知实例');
    await selectOption('External fixture');
    await screen.findByRole('spinbutton', { name: 'Amount' });
    await openSelect('Amount输入方式');
    await selectOption('数值表达式');
    const expression = await screen.findByRole('textbox', { name: 'Amount' });
    fireEvent.change(expression, {
      target: { value: '{{sys.dialog_count}} + 1' }
    });
    fireEvent.blur(expression);
    await waitFor(() =>
      expect(input().find((entry) => entry.name === 'amount')).toEqual({
        name: 'amount',
        valueType: 'number',
        value: { kind: 'templated_text', value: '{{sys.dialog_count}} + 1' }
      })
    );
    fireEvent.click(screen.getByRole('button', { name: '编辑 JSON Schema' }));
    const fieldName = await screen.findByLabelText('Schema 字段名 1');
    fireEvent.change(fieldName, { target: { value: 'receipt' } });
    fireEvent.click(screen.getByRole('button', { name: /^保\s*存$/u }));
    await waitFor(() =>
      expect(node().outputs[0].jsonSchema).toMatchObject({
        properties: {
          receipt: { type: 'object', properties: { id: { type: 'string' } } }
        }
      })
    );
    expect(node().outputs[0].selector).toEqual(['result']);
    expect(listVisibleSelectorOptions(latestDocument, 'node-llm')).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          value: ['node-notification', 'result', 'receipt', 'id']
        })
      ])
    );
  });
  test('keeps plugin instance parameters out of the flow and explains unavailability', async () => {
    vi.mocked(listNotificationInstances).mockResolvedValue([
      {
        ...pluginInstance,
        available: false,
        unavailable_reason: 'plugin_unavailable'
      }
    ]);
    vi.mocked(listNotificationTypes).mockResolvedValue([
      {
        ...pluginType,
        available: false,
        unavailable_reason: 'plugin_unavailable'
      }
    ]);
    await renderNotification(
      { instance_id: pluginInstance.id },
      {
        input: {
          kind: 'named_bindings',
          value: [
            {
              name: 'amount',
              valueType: 'number',
              value: { kind: 'constant', value: 2 }
            }
          ]
        }
      }
    );
    await screen.findByText('通知实例不可用', { exact: true });
    expect(node().config).toEqual({ instance_id: pluginInstance.id });
    expect(input()).toEqual([
      {
        name: 'amount',
        valueType: 'number',
        value: { kind: 'constant', value: 2 }
      }
    ]);
    expect(JSON.stringify(node())).not.toContain('instance-only');
  });
});
