import type { FlowNodeType } from '@1flowbase/flow-schema';
import type { NodePickerOption } from '../../../plugin-node-definitions';
export function canAddContainerBodyNode(type: FlowNodeType) {
  return ![
    'start',
    'workflow_start',
    'workflow_end',
    'answer',
    'container_start'
  ].includes(type);
}
export function containerBodyPickerOptions(
  options: NodePickerOption[],
  containerId: string | null
) {
  return containerId
    ? options.filter(
        (option) =>
          option.kind !== 'builtin' || canAddContainerBodyNode(option.type)
      )
    : options;
}
