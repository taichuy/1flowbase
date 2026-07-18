import {
  COMPACT_SOURCE_HANDLE_ID,
  getStartCompactDispatch,
  type FlowNodeDocument
} from '@1flowbase/flow-schema';

type CompactDispatchNode = Pick<FlowNodeDocument, 'type' | 'config'>;

export function isStartCompactHandle(
  node: CompactDispatchNode | null | undefined,
  sourceHandle: string | null | undefined
) {
  return node?.type === 'start' && sourceHandle === COMPACT_SOURCE_HANDLE_ID;
}

export function isApplicationFlowStart(
  node: CompactDispatchNode | null | undefined
) {
  return (
    node?.type === 'start' &&
    getStartCompactDispatch(node.config) === 'application_flow'
  );
}

export function isApplicationFlowCompactSource(
  node: CompactDispatchNode | null | undefined,
  sourceHandle: string | null | undefined
) {
  return (
    isStartCompactHandle(node, sourceHandle) &&
    isApplicationFlowStart(node)
  );
}

export function isFlowTerminalNode(
  node: Pick<FlowNodeDocument, 'type'> | null | undefined
) {
  return node?.type === 'answer' || node?.type === 'compact_response';
}
