import type {
  FlowNodeDocument,
  FlowNodeOutputDocument
} from '@1flowbase/flow-schema';

export interface LoopVariableDefinition {
  key: string;
  value_type: string;
}
export function loopVariableDefinitions(
  node: Pick<FlowNodeDocument, 'config'>
): LoopVariableDefinition[] {
  const value = node.config.loop_variables;
  return Array.isArray(value)
    ? value.filter(
        (item): item is LoopVariableDefinition =>
          typeof item === 'object' &&
          item !== null &&
          typeof item.key === 'string' &&
          typeof item.value_type === 'string'
      )
    : [];
}
export function loopVariableOutputs(
  node: Pick<FlowNodeDocument, 'config'>
): FlowNodeOutputDocument[] {
  return loopVariableDefinitions(node).map((variable) => ({
    key: `variables.${variable.key}`,
    title: variable.key,
    valueType: variable.value_type,
    selector: ['variables', variable.key]
  }));
}
export function containerLocalOutputs(
  node: FlowNodeDocument
): FlowNodeOutputDocument[] {
  if (node.type !== 'loop' && node.type !== 'iteration') return [];
  return [
    { key: 'index', title: 'index', valueType: 'number' },
    ...(node.type === 'iteration'
      ? [{ key: 'item', title: 'item', valueType: 'json' }]
      : [
          { key: 'variables', title: 'variables', valueType: 'object' },
          ...loopVariableOutputs(node)
        ])
  ];
}

export function defaultLoopVariableValue(type: string): unknown {
  if (type === 'number') return 0;
  if (type === 'boolean') return false;
  if (type === 'array') return [];
  if (type === 'object') return {};
  return '';
}
