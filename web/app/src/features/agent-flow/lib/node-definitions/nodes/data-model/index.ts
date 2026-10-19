import type { FlowNodeDocument } from '@1flowbase/flow-schema';

import { basicFields } from '../../base';
import type { NodeDefinition } from '../../types';

export type DataModelNodeAction = 'list' | 'get' | 'create' | 'update' | 'delete';
export type DataModelFlowNodeType =
  | 'data_model_list'
  | 'data_model_get'
  | 'data_model_create'
  | 'data_model_update'
  | 'data_model_delete';

const DATA_MODEL_ACTION_BY_NODE_TYPE = {
  data_model_list: 'list',
  data_model_get: 'get',
  data_model_create: 'create',
  data_model_update: 'update',
  data_model_delete: 'delete'
} satisfies Record<DataModelFlowNodeType, DataModelNodeAction>;

const DATA_MODEL_NODE_LABELS = {
  data_model_list: 'Data Model List',
  data_model_get: 'Data Model Get',
  data_model_create: 'Data Model Create',
  data_model_update: 'Data Model Update',
  data_model_delete: 'Data Model Delete'
} satisfies Record<DataModelFlowNodeType, string>;

const defaultDataModelNodeConfig = { data_model_code: '' } as const;

const DATA_MODEL_WRITE_ACTIONS = new Set<DataModelNodeAction>([
  'create',
  'update',
  'delete'
]);

export function getDataModelNodeDefaultConfig(
  nodeType: DataModelFlowNodeType
): Record<string, unknown> {
  const action = getDataModelActionForNodeType(nodeType);

  return action && DATA_MODEL_WRITE_ACTIONS.has(action)
    ? { ...defaultDataModelNodeConfig, side_effect_policy: 'disabled' }
    : { ...defaultDataModelNodeConfig };
}

const dataModelActionOutputs = {
  list: [
    { key: 'records', title: 'Records', valueType: 'array' },
    { key: 'total', title: 'Total', valueType: 'number' }
  ],
  get: [{ key: 'record', title: 'Record', valueType: 'json' }],
  create: [{ key: 'record', title: 'Record', valueType: 'json' }],
  update: [{ key: 'record', title: 'Record', valueType: 'json' }],
  delete: [
    { key: 'deleted_id', title: 'Deleted ID', valueType: 'string' },
    { key: 'affected_count', title: 'Affected Count', valueType: 'number' }
  ]
} satisfies Record<DataModelNodeAction, FlowNodeDocument['outputs']>;

function isDataModelFlowNodeType(
  nodeType: string
): nodeType is DataModelFlowNodeType {
  return Object.prototype.hasOwnProperty.call(
    DATA_MODEL_ACTION_BY_NODE_TYPE,
    nodeType
  );
}

export function getDataModelActionForNodeType(
  nodeType: string
): DataModelNodeAction | null {
  return isDataModelFlowNodeType(nodeType)
    ? DATA_MODEL_ACTION_BY_NODE_TYPE[nodeType]
    : null;
}

export function getDataModelNodeOutputs(
  action: DataModelNodeAction
): FlowNodeDocument['outputs'] {
  return dataModelActionOutputs[action].map((output) => ({ ...output }));
}

const dataModelField = {
  key: 'config.data_model_code',
  label: 'Data Model',
  editor: 'data_model',
  required: true
} as const;

const dataModelQueryField = {
  key: 'bindings.query',
  label: 'Query',
  editor: 'data_model_query'
} as const;

const dataModelRecordIdField = {
  key: 'bindings.record_id',
  label: 'Record ID',
  editor: 'selector',
  required: true
} as const;

const dataModelPayloadField = {
  key: 'bindings.payload',
  label: 'Payload',
  editor: 'named_bindings',
  required: true
} as const;

const dataModelSideEffectPolicyField = {
  key: 'config.side_effect_policy',
  label: 'Side Effect Policy',
  editor: 'static_select',
  required: true,
  options: [
    { label: 'Disabled', value: 'disabled' },
    { label: 'Confirm Each Run', value: 'confirm_each_run' },
    { label: 'Allow With Idempotency', value: 'allow_with_idempotency' }
  ]
} satisfies NodeDefinition['sections'][number]['fields'][number];

function createDataModelNodeDefinition({
  label,
  fields
}: {
  label: string;
  fields: NodeDefinition['sections'][number]['fields'];
}): NodeDefinition {
  return {
    label,
    sections: [
      {
        key: 'basics',
        title: 'Basics',
        fields: basicFields
      },
      {
        key: 'inputs',
        title: 'Inputs',
        fields: [dataModelField, ...fields]
      },
      {
        key: 'outputs',
        title: 'Outputs',
        fields: []
      }
    ]
  };
}

const dataModelListNodeDefinition = createDataModelNodeDefinition({
  label: DATA_MODEL_NODE_LABELS.data_model_list,
  fields: [dataModelQueryField]
});

const dataModelGetNodeDefinition = createDataModelNodeDefinition({
  label: DATA_MODEL_NODE_LABELS.data_model_get,
  fields: [dataModelRecordIdField]
});

const dataModelCreateNodeDefinition = createDataModelNodeDefinition({
  label: DATA_MODEL_NODE_LABELS.data_model_create,
  fields: [dataModelPayloadField, dataModelSideEffectPolicyField]
});

const dataModelUpdateNodeDefinition = createDataModelNodeDefinition({
  label: DATA_MODEL_NODE_LABELS.data_model_update,
  fields: [dataModelRecordIdField, dataModelPayloadField, dataModelSideEffectPolicyField]
});

const dataModelDeleteNodeDefinition = createDataModelNodeDefinition({
  label: DATA_MODEL_NODE_LABELS.data_model_delete,
  fields: [dataModelRecordIdField, dataModelSideEffectPolicyField]
});

export const dataModelNodeDefinitions = {
  data_model_list: dataModelListNodeDefinition,
  data_model_get: dataModelGetNodeDefinition,
  data_model_create: dataModelCreateNodeDefinition,
  data_model_update: dataModelUpdateNodeDefinition,
  data_model_delete: dataModelDeleteNodeDefinition
} satisfies Record<DataModelFlowNodeType, NodeDefinition>;

export const dataModelNodeMeta = {
  data_model_list: {
    summary: 'List records from a Data Model runtime.',
    helpHref: '/docs/agentflow/nodes/data-model-list'
  },
  data_model_get: {
    summary: 'Get one record from a Data Model runtime.',
    helpHref: '/docs/agentflow/nodes/data-model-get'
  },
  data_model_create: {
    summary: 'Create a record through a Data Model runtime.',
    helpHref: '/docs/agentflow/nodes/data-model-create'
  },
  data_model_update: {
    summary: 'Update a record through a Data Model runtime.',
    helpHref: '/docs/agentflow/nodes/data-model-update'
  },
  data_model_delete: {
    summary: 'Delete a record through a Data Model runtime.',
    helpHref: '/docs/agentflow/nodes/data-model-delete'
  }
} as const;
