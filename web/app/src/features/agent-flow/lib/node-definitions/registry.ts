import type {
  FlowNodeDocument,
  FlowNodeOutputDocument,
  FlowNodeType,
  NodeRuntimeUiContract
} from '@1flowbase/flow-schema';

import type { SchemaViewRenderer } from '../../../../shared/schema-ui/registry/create-renderer-registry';
import type { NodeDefinitionMeta } from './types';

export interface RegisteredNodeDefinition {
  contract: NodeRuntimeUiContract;
  meta: NodeDefinitionMeta;
  variableOutputs?: (
    node: Pick<FlowNodeDocument, 'config' | 'outputs'>
  ) => FlowNodeOutputDocument[];
  cardDescription?: SchemaViewRenderer;
  editableOutputContract?: boolean;
  suppressGeneratedOutputVariables?: boolean;
}

const registeredNodeDefinitions = new Map<
  FlowNodeType,
  RegisteredNodeDefinition
>();

export function registerNodeDefinition(
  nodeType: FlowNodeType,
  definition: RegisteredNodeDefinition
) {
  registeredNodeDefinitions.set(nodeType, definition);
}

export function getRegisteredNodeDefinition(
  nodeType: FlowNodeType
): RegisteredNodeDefinition | null {
  return registeredNodeDefinitions.get(nodeType) ?? null;
}
