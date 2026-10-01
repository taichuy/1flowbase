import { fireEvent, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, test } from 'vitest';

import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';
import { NodeConfigTab } from '../../components/detail/tabs/NodeConfigTab';
import { AgentFlowEditorStoreProvider } from '../../store/editor/AgentFlowEditorStoreProvider';
import {
  DocumentObserver,
  SelectionSeed,
  createInitialStateWithDataModelNode,
  fetchDataModelOptionsSpy,
  getDataModelNode,
  openSelect,
  renderWithProviders,
  selectDataModelOption,
  selectOption,
  setupNodeInspectorTest
} from './support';

beforeEach(setupNodeInspectorTest);

describe('NodeInspector data model', () => {
  test('loads Data Model options from the feature API and disables unavailable models', async () => {
    await renderWithProviders(
      <AgentFlowEditorStoreProvider
        initialState={createInitialStateWithDataModelNode()}
      >
        <SelectionSeed nodeId="node-data-model" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    await openSelect('Data Model');

    expect(fetchDataModelOptionsSpy).toHaveBeenCalledTimes(1);
    expect(
      await screen.findByTestId('data-model-option-orders')
    ).not.toHaveAttribute('aria-disabled', 'true');
    expect(
      screen.getByTestId('data-model-option-draft_orders')
    ).toHaveAttribute('aria-disabled', 'true');
    expect(
      screen.getByTestId('data-model-option-disabled_orders')
    ).toHaveAttribute('aria-disabled', 'true');
    expect(
      screen.getByTestId('data-model-option-broken_orders')
    ).toHaveAttribute('aria-disabled', 'true');
  });

  test('updates selected Data Model metadata when the selected model changes', async () => {
    let latestDocument = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    await renderWithProviders(
      <AgentFlowEditorStoreProvider
        initialState={createInitialStateWithDataModelNode()}
      >
        <SelectionSeed nodeId="node-data-model" />
        <DocumentObserver
          onChange={(document) => {
            latestDocument = document;
          }}
        />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    await openSelect('Data Model');
    await selectDataModelOption('orders');

    await waitFor(() => {
      expect(getDataModelNode(latestDocument).config).toMatchObject({
        data_model_code: 'orders',
        data_model_id: 'model-orders',
        data_model_label: 'Orders',
        data_model_fields: [
          { code: 'name', title: 'Name', valueType: 'string', required: true },
          {
            code: 'amount',
            title: 'Amount',
            valueType: 'number',
            required: false
          },
          {
            code: 'status',
            title: 'Status',
            valueType: 'enum',
            required: false
          },
          {
            code: 'customer',
            title: 'Customer',
            valueType: 'many_to_one',
            required: false
          },
          {
            code: 'lines',
            title: 'Lines',
            valueType: 'one_to_many',
            required: false
          },
          {
            code: 'approved',
            title: 'Approved',
            valueType: 'boolean',
            required: false
          }
        ]
      });
    });
  });

  test('edits Data Model list query binding', async () => {
    let latestDocument = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    await renderWithProviders(
      <AgentFlowEditorStoreProvider
        initialState={createInitialStateWithDataModelNode()}
      >
        <SelectionSeed nodeId="node-data-model" />
        <DocumentObserver
          onChange={(document) => {
            latestDocument = document;
          }}
        />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    expect(await screen.findByText('请先选择 Data Model')).toBeInTheDocument();

    await openSelect('Data Model');
    await selectDataModelOption('orders');
    fireEvent.click(
      await screen.findByRole('button', { name: '新增过滤条件' })
    );
    await openSelect('过滤字段 1');
    await selectOption('Status');
    await openSelect('过滤操作符 1');
    await selectOption('eq');
    await openSelect('过滤值来源 1');
    await selectOption('变量');
    await openSelect('过滤变量 1');
    await selectOption('Start');
    await selectOption('query');
    fireEvent.click(
      await screen.findByRole('button', { name: '新增过滤条件' })
    );
    await openSelect('过滤字段 2');
    await selectOption('Amount');
    await waitFor(() => {
      expect(getDataModelNode(latestDocument).bindings.query).toMatchObject({
        value: {
          filters: [
            expect.anything(),
            {
              field_code: 'amount',
              value: { kind: 'constant', value: 0 }
            }
          ]
        }
      });
    });
    fireEvent.change(screen.getByLabelText('过滤值 2'), {
      target: { value: '123' }
    });
    fireEvent.click(
      await screen.findByRole('button', { name: '新增过滤条件' })
    );
    await openSelect('过滤字段 3');
    await selectOption('Approved');
    await waitFor(() => {
      expect(getDataModelNode(latestDocument).bindings.query).toMatchObject({
        value: {
          filters: [
            expect.anything(),
            expect.anything(),
            {
              field_code: 'approved',
              value: { kind: 'constant', value: false }
            }
          ]
        }
      });
    });
    await openSelect('过滤值来源 3');
    await selectOption('变量');
    await openSelect('过滤值来源 3');
    await selectOption('常量');
    await waitFor(() => {
      expect(getDataModelNode(latestDocument).bindings.query).toMatchObject({
        value: {
          filters: [
            expect.anything(),
            expect.anything(),
            {
              field_code: 'approved',
              value: { kind: 'constant', value: false }
            }
          ]
        }
      });
    });
    await openSelect('过滤值 3');
    await selectOption('true');
    fireEvent.click(screen.getByRole('button', { name: '新增排序规则' }));
    await openSelect('排序字段 1');
    await selectOption('Amount');
    await openSelect('排序方向 1');
    await selectOption('desc');
    await openSelect('展开关联');
    await selectOption('Customer');
    fireEvent.change(screen.getByLabelText('页码'), { target: { value: '2' } });
    fireEvent.change(screen.getByLabelText('每页数量'), {
      target: { value: '50' }
    });

    await waitFor(() => {
      expect(getDataModelNode(latestDocument).bindings.query).toMatchObject({
        kind: 'data_model_query',
        value: {
          filters: [
            {
              field_code: 'status',
              operator: 'eq',
              value: {
                kind: 'selector',
                selector: ['node-start', 'query']
              }
            },
            {
              field_code: 'amount',
              operator: 'eq',
              value: { kind: 'constant', value: 123 }
            },
            {
              field_code: 'approved',
              operator: 'eq',
              value: { kind: 'constant', value: true }
            }
          ],
          sorts: [{ field_code: 'amount', direction: 'desc' }],
          expand_relations: ['customer'],
          page: { kind: 'constant', value: 2 },
          page_size: { kind: 'constant', value: 50 }
        }
      });
    });
  }, 60_000);

  test('keeps Data Model query pagination editable when selected model has no fields', async () => {
    let latestDocument = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    await renderWithProviders(
      <AgentFlowEditorStoreProvider
        initialState={createInitialStateWithDataModelNode()}
      >
        <SelectionSeed nodeId="node-data-model" />
        <DocumentObserver
          onChange={(document) => {
            latestDocument = document;
          }}
        />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    expect(await screen.findByText('请先选择 Data Model')).toBeInTheDocument();

    await openSelect('Data Model');
    await selectDataModelOption('empty_orders');

    await waitFor(() => {
      expect(screen.queryByText('请先选择 Data Model')).not.toBeInTheDocument();
      expect(screen.getByLabelText('页码')).toBeInTheDocument();
      expect(screen.getByLabelText('每页数量')).toBeInTheDocument();
    });

    expect(screen.getByRole('button', { name: '新增过滤条件' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '新增排序规则' })).toBeDisabled();

    fireEvent.change(screen.getByLabelText('页码'), { target: { value: '3' } });
    fireEvent.change(screen.getByLabelText('每页数量'), {
      target: { value: '10' }
    });

    await waitFor(() => {
      expect(getDataModelNode(latestDocument).bindings.query).toMatchObject({
        kind: 'data_model_query',
        value: {
          filters: [],
          sorts: [],
          page: { kind: 'constant', value: 3 },
          page_size: { kind: 'constant', value: 10 }
        }
      });
    });
  }, 10000);

  test('renders Data Model create, update, and delete editors from fixed node types', async () => {
    const { unmount: unmountCreate } = await renderWithProviders(
      <AgentFlowEditorStoreProvider
        initialState={createInitialStateWithDataModelNode('data_model_create')}
      >
        <SelectionSeed nodeId="node-data-model" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    expect(screen.queryByLabelText('Action')).not.toBeInTheDocument();
    expect(
      screen.queryByTestId('inspector-field-bindings.record_id')
    ).not.toBeInTheDocument();
    expect(
      screen.queryByTestId('inspector-field-bindings.query')
    ).not.toBeInTheDocument();
    expect(
      screen.getByTestId('inspector-field-bindings.payload')
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '新增字段赋值' }));
    expect(screen.getAllByLabelText('Payload-0-field').length).toBeGreaterThan(
      0
    );
    expect(
      screen.getAllByLabelText('Payload-0-variable').length
    ).toBeGreaterThan(0);
    unmountCreate();

    const { unmount: unmountUpdate } = await renderWithProviders(
      <AgentFlowEditorStoreProvider
        initialState={createInitialStateWithDataModelNode('data_model_update')}
      >
        <SelectionSeed nodeId="node-data-model" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    expect(
      screen.getByTestId('inspector-field-bindings.record_id')
    ).toBeInTheDocument();
    expect(
      screen.queryByTestId('inspector-field-bindings.query')
    ).not.toBeInTheDocument();
    expect(
      screen.getByTestId('inspector-field-bindings.payload')
    ).toBeInTheDocument();
    unmountUpdate();

    await renderWithProviders(
      <AgentFlowEditorStoreProvider
        initialState={createInitialStateWithDataModelNode('data_model_delete')}
      >
        <SelectionSeed nodeId="node-data-model" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    expect(
      screen.getByTestId('inspector-field-bindings.record_id')
    ).toBeInTheDocument();
    expect(
      screen.queryByTestId('inspector-field-bindings.query')
    ).not.toBeInTheDocument();
    expect(
      screen.queryByTestId('inspector-field-bindings.payload')
    ).not.toBeInTheDocument();
  }, 45000);

  test('keeps pagination unique to Data Model list query editors', async () => {
    const { unmount } = await renderWithProviders(
      <AgentFlowEditorStoreProvider
        initialState={createInitialStateWithDataModelNode('data_model_get')}
      >
        <SelectionSeed nodeId="node-data-model" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    await openSelect('Data Model');
    await selectDataModelOption('orders');

    await waitFor(() => {
      expect(
        screen.getByTestId('inspector-field-bindings.record_id')
      ).toBeInTheDocument();
      expect(screen.queryByLabelText('Query')).not.toBeInTheDocument();
      expect(screen.queryByLabelText('页码')).not.toBeInTheDocument();
      expect(screen.queryByLabelText('每页数量')).not.toBeInTheDocument();
      expect(
        screen.queryByRole('button', { name: '新增过滤条件' })
      ).not.toBeInTheDocument();
    });

    unmount();

    await renderWithProviders(
      <AgentFlowEditorStoreProvider
        initialState={createInitialStateWithDataModelNode('data_model_list')}
      >
        <SelectionSeed nodeId="node-data-model" />
        <NodeConfigTab />
      </AgentFlowEditorStoreProvider>
    );

    await openSelect('Data Model');
    await selectDataModelOption('orders');

    await waitFor(() => {
      expect(screen.getByLabelText('页码')).toBeInTheDocument();
      expect(screen.getByLabelText('每页数量')).toBeInTheDocument();
    });
  });
});
